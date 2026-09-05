#![cfg(unix)]

use std::path::Path;

use commandagent::mode::ExecutionMode;
use commandagent::tools::bash::path_confinement_rejection;
use commandagent::tools::registry::{ToolContext, ToolRegistry, tool_error_kind};
use commandagent::tools::workspace_policy::WorkspacePolicy;
use serde_json::{Value, json};

const FIXTURE: &str = "tests/corpus/apps/issue428-bash-dynamic-route";

fn context(root: &Path, events: &Path) -> ToolContext {
    ToolContext {
        root: root.to_path_buf(),
        mode: ExecutionMode::Act,
        auto_approve: true,
        interactive_approval: false,
        offline: true,
        workspace_policy: WorkspacePolicy::NormalTask,
        eval_events_path: Some(events.to_path_buf()),
        expected_paths: Vec::new(),
        protected_paths: Vec::new(),
    }
}

fn copy_tree(source: &Path, target: &Path) {
    std::fs::create_dir_all(target).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let destination = target.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &destination);
        } else {
            std::fs::copy(entry.path(), destination).unwrap();
        }
    }
}

#[test]
fn e1_corpus_reads_execute_in_a_treatment_workspace_without_confinement_events() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("recovery treatment");
    copy_tree(Path::new(FIXTURE), &root);
    let events = temp.path().join("events.jsonl");
    let reads: Vec<Value> = serde_json::from_str(
        &std::fs::read_to_string(Path::new(FIXTURE).join("fixtures/reads.json")).unwrap(),
    )
    .unwrap();
    for read in reads {
        let command = read["command"].as_str().unwrap();
        let output = ToolRegistry::default()
            .execute(
                "Bash",
                &json!({"command": command}),
                &context(&root, &events),
            )
            .unwrap_or_else(|error| panic!("{command}: {error}"));
        assert!(output.contains("outcome: Success"), "{command}: {output}");
        assert!(
            output.contains(read["contains"].as_str().unwrap()),
            "{output}"
        );
    }
    assert!(
        !events.exists(),
        "successful reads must not emit rejections"
    );
}

#[test]
fn absolute_candidates_remain_confined_across_quotes_and_operators() {
    let root = tempfile::tempdir().unwrap();
    for command in [
        "cat /outside/route.ts",
        r#"cat "/outside/[id]/route.ts""#,
        r"cat /outside/\[id\]/route.ts",
        r#"cat /out"side"/route.ts"#,
        "cat /out\\side/route.ts",
        "cat </outside/route.ts",
        "ls 2>/dev/null;cat /outside/route.ts",
        "ls 2>/dev/null&&cat /outside/route.ts",
        "ls 2>/dev/null|cat /outside/route.ts",
        r#"sh -c 'cat /outside/route.ts'"#,
        r#"python -c "paths=['/outside/route.ts']""#,
        "cat ]/outside/route.ts",
        "cat /dev/zero",
        "cat '/dev/null;'",
        "cat '/dev/null>'",
        "cat '/dev/null[other]'",
        r"cat /dev/null\;",
    ] {
        let rejection = path_confinement_rejection(command, root.path())
            .unwrap_or_else(|| panic!("allowed {command}"));
        assert_eq!(rejection.operation, "path reference", "{command}");
    }
    let route = root.path().join("src/app/api/expenses/[id]/route.ts");
    assert!(
        path_confinement_rejection(&format!("cat '{}'", route.display()), root.path()).is_none()
    );
}

#[test]
fn traversal_symlink_and_outside_writes_still_reject_before_execution() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("workspace");
    let outside = temp.path().join("outside");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("[id]")).unwrap();
    let events = temp.path().join("events.jsonl");
    for command in [
        "printf forbidden > '../outside/written.txt'",
        "printf forbidden > '[id]/written.txt'",
        r"printf forbidden > \[id\]/written.txt",
        "ls 2>/dev/null;printf forbidden > ../outside/written.txt",
        "printf forbidden > /dev/zero",
        "printf forbidden > '/dev/null;'",
        "rm /dev/null",
    ] {
        let error = ToolRegistry::default()
            .execute(
                "Bash",
                &json!({"command": command}),
                &context(&root, &events),
            )
            .unwrap_err();
        assert_eq!(
            tool_error_kind(&error),
            "bash_path_confinement_error",
            "{command}"
        );
        assert!(!outside.join("written.txt").exists());
    }
    let records: Vec<Value> = std::fs::read_to_string(events)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(records.len(), 7);
    for record in records {
        assert_eq!(record["event"], "bash_path_confinement_rejected");
        assert_eq!(record["blocked"], true);
        assert!(
            record["reason"]
                .as_str()
                .unwrap()
                .contains("Gate 1 workspace boundary")
        );
    }
}

#[test]
fn whole_relative_paths_cannot_hide_read_traversal_or_symlink_escape() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("workspace");
    let outside = temp.path().join("outside");
    std::fs::create_dir_all(root.join("src/[id]")).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("route.ts"), "outside sentinel").unwrap();
    std::os::unix::fs::symlink(&outside, root.join("src/[link]")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("src/i")).unwrap();
    let events = temp.path().join("events.jsonl");
    for command in [
        "cat ../outside/route.ts".to_string(),
        "cat 'src/[id]/../../../outside/route.ts'".to_string(),
        "cat 'src/[link]/route.ts'".to_string(),
        "cat src/[id]/route.ts".to_string(),
        "sh -c 'cat src/[id]/route.ts'".to_string(),
        r"cat src/\[link\]/route.ts".to_string(),
        "sh -c 'cat src/[link]/route.ts'".to_string(),
        format!("cat '{}'", root.join("src/[link]/route.ts").display()),
        format!("cat '{}'", root.join("../outside/route.ts").display()),
    ] {
        let error = ToolRegistry::default()
            .execute(
                "Bash",
                &json!({"command": command}),
                &context(&root, &events),
            )
            .unwrap_err();
        assert_eq!(
            tool_error_kind(&error),
            "bash_path_confinement_error",
            "{command}"
        );
        assert!(!error.to_string().contains("outside sentinel"));
    }
}

#[test]
fn path_boundary_fix_does_not_grant_write_approval() {
    let root = tempfile::tempdir().unwrap();
    let mut context = context(root.path(), &root.path().join("events.jsonl"));
    context.auto_approve = false;
    let error = ToolRegistry::default()
        .execute(
            "Bash",
            &json!({"command": "printf forbidden > '[id].txt'"}),
            &context,
        )
        .unwrap_err();
    assert!(error.to_string().contains("approval"), "{error}");
    assert!(!root.path().join("[id].txt").exists());
}
