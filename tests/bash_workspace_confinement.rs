#![cfg(unix)]

use commandagent::mode::ExecutionMode;
use commandagent::tools::registry::{ToolContext, ToolRegistry, tool_error_kind};
use commandagent::tools::workspace_policy::WorkspacePolicy;
use serde_json::{Value, json};

fn context(root: &std::path::Path, events: &std::path::Path) -> ToolContext {
    ToolContext {
        root: root.to_path_buf(),
        mode: ExecutionMode::Act,
        auto_approve: true,
        interactive_approval: false,
        offline: false,
        workspace_policy: WorkspacePolicy::NormalTask,
        eval_events_path: Some(events.to_path_buf()),
        expected_paths: Vec::new(),
        protected_paths: Vec::new(),
    }
}

fn confinement_events(path: &std::path::Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .filter(|event: &Value| event["event"] == "bash_path_confinement_rejected")
        .collect()
}

#[test]
fn bash_blocks_outside_symlink_and_file_write_with_event_reasons() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("workspace");
    let outside = fixture.path().join("outside");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    let events = fixture.path().join("events.jsonl");
    let registry = ToolRegistry::default();

    let outside_link = outside.join("python");
    let symlink_command = format!(
        "ln -s /usr/bin/python3 '{}' 2>/dev/null || ln -s /bin/sh '{}'",
        outside_link.display(),
        outside_link.display()
    );
    let error = registry
        .execute(
            "Bash",
            &json!({"command": symlink_command}),
            &context(&root, &events),
        )
        .unwrap_err();
    assert_eq!(tool_error_kind(&error), "bash_path_confinement_error");
    assert!(!outside_link.exists());

    let outside_file = outside.join("written.txt");
    let error = registry
        .execute(
            "Bash",
            &json!({"command": format!("printf forbidden > '{}'", outside_file.display())}),
            &context(&root, &events),
        )
        .unwrap_err();
    assert_eq!(tool_error_kind(&error), "bash_path_confinement_error");
    assert!(!outside_file.exists());

    let workspace_link = root.join("linked-outside");
    let linked_write = outside.join("through-link.txt");
    let error = registry
        .execute(
            "Bash",
            &json!({
                "command": format!(
                    "ln -s '{}' linked-outside && printf forbidden > linked-outside/through-link.txt",
                    outside.display()
                )
            }),
            &context(&root, &events),
        )
        .unwrap_err();
    assert_eq!(tool_error_kind(&error), "bash_path_confinement_error");
    assert!(!workspace_link.exists());
    assert!(!linked_write.exists());

    let rejections = confinement_events(&events);
    assert_eq!(rejections.len(), 3);
    assert!(rejections.iter().all(|event| event["blocked"] == true));
    assert!(rejections.iter().all(|event| {
        event["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("outside the Gate 1 workspace boundary"))
    }));
    assert_eq!(rejections[0]["operation"], "ln");
    assert_eq!(rejections[1]["operation"], "output redirection");
    assert_eq!(rejections[2]["operation"], "symlink target");
}

#[test]
fn bash_keeps_normal_workspace_commands_and_symlinks_working() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("workspace");
    std::fs::create_dir_all(&root).unwrap();
    let events = fixture.path().join("events.jsonl");

    let output = ToolRegistry::default()
        .execute(
            "Bash",
            &json!({
                "command": "mkdir -p output && printf ok > output/result.txt && ln -s result.txt output/current.txt && test -e /bin/sh"
            }),
            &context(&root, &events),
        )
        .unwrap();

    assert!(output.contains("outcome: Success"), "{output}");
    assert_eq!(
        std::fs::read_to_string(root.join("output/current.txt")).unwrap(),
        "ok"
    );
    assert!(
        !events.exists(),
        "allowed commands must not emit rejections"
    );
}
