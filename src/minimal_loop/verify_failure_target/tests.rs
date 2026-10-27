use std::path::Path;

use tempfile::TempDir;

use super::{append_repair_guidance, resolve_target};
use crate::minimal_loop::build_verifier;
use crate::planner::fix_diagnostics::prompt_has_diagnostic;
use crate::planner::repair::{RecoveryHandoff, render_ultra_recovery_prompt};
use crate::planner::verify::VerificationReport;

const SMOKE_CHECK: &str = "const { readOne } = require('./src/lib/persistence');\n\
if (readOne('missing') !== null) throw new Error('expected null');\n";

fn write(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn workspace(script: &str, files: &[&str]) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "smoke-check.js", script);
    for file in files {
        write(dir.path(), file, "module.exports = {};\n");
    }
    dir
}

fn report(command: &str) -> VerificationReport {
    let mut report = VerificationReport::pass();
    report.push_command_failure(command, "AssertionError: expected null");
    report
}

fn target(dir: &TempDir, command: &str) -> Option<String> {
    resolve_target(dir.path(), &report(command))
}

#[test]
fn required_js_file_wins_over_typescript_sibling() {
    let dir = workspace(
        SMOKE_CHECK,
        &["src/lib/persistence.js", "src/lib/persistence.ts"],
    );
    assert_eq!(
        target(&dir, "node smoke-check.js").as_deref(),
        Some("src/lib/persistence.js")
    );
}

#[test]
fn node_resolution_order_and_import_forms_are_followed() {
    let dir = workspace(
        "import { run } from './lib/run';\n",
        &["lib/run.mjs", "lib/run.json"],
    );
    assert_eq!(
        target(&dir, "node smoke-check.js").as_deref(),
        Some("lib/run.mjs")
    );
    let dir = workspace("require(\"./lib\");\n", &["lib/index.js"]);
    assert_eq!(
        target(&dir, "node --trace-warnings smoke-check.js").as_deref(),
        Some("lib/index.js")
    );
}

#[test]
fn typescript_only_target_is_not_named() {
    let dir = workspace(SMOKE_CHECK, &["src/lib/persistence.ts"]);
    assert_eq!(target(&dir, "node smoke-check.js"), None);
}

#[test]
fn script_itself_package_names_and_missing_script_are_not_named() {
    let dir = workspace("require('./smoke-check');\n", &[]);
    assert_eq!(target(&dir, "node smoke-check.js"), None);
    let dir = workspace("require('react');\nrequire('node:fs');\n", &["react.js"]);
    assert_eq!(target(&dir, "node smoke-check.js"), None);
    let dir = workspace(SMOKE_CHECK, &["src/lib/persistence.js"]);
    assert_eq!(target(&dir, "node missing-check.js"), None);
}

#[test]
fn workspace_escape_and_hidden_runtime_dirs_are_not_named() {
    let outer = tempfile::tempdir().unwrap();
    let root = outer.path().join("app");
    write(&root, "smoke-check.js", "require('../shared.js');\n");
    write(outer.path(), "shared.js", "module.exports = {};\n");
    let mut escaping = VerificationReport::pass();
    escaping.push_command_failure("node smoke-check.js", "failed");
    assert_eq!(resolve_target(&root, &escaping), None);

    let dir = workspace(
        "require('./.commandagent/helper');\n",
        &[".commandagent/helper.js"],
    );
    assert_eq!(target(&dir, "node smoke-check.js"), None);
    let dir = workspace(
        "require('./node_modules/dep/index');\n",
        &["node_modules/dep/index.js"],
    );
    assert_eq!(target(&dir, "node smoke-check.js"), None);
    let dir = workspace(SMOKE_CHECK, &["src/lib/persistence.js"]);
    assert_eq!(target(&dir, "node ../smoke-check.js"), None);
}

#[test]
fn ambiguous_or_partly_unresolved_requires_are_not_named() {
    let dir = workspace("require('./a');\nrequire('./b');\n", &["a.js", "b.js"]);
    assert_eq!(target(&dir, "node smoke-check.js"), None);
    let dir = workspace("require('./a');\nrequire('./b');\n", &["a.js", "b.ts"]);
    assert_eq!(target(&dir, "node smoke-check.js"), None);
    let dir = workspace("// no relative loads\nrequire('node:fs');\n", &[]);
    assert_eq!(target(&dir, "node smoke-check.js"), None);
}

#[test]
fn grep_inline_and_compound_commands_are_not_named() {
    let dir = workspace(
        SMOKE_CHECK,
        &["src/lib/persistence.js", "src/hooks/useTodos.ts"],
    );
    for command in [
        "grep -q \"src/lib/types\" src/hooks/useTodos.ts",
        "grep -qE 'persistence' smoke-check.js",
        "node -e \"require('./src/lib/persistence')\"",
        "node -p smoke-check.js",
        "cd . && node smoke-check.js",
        "npm test && node smoke-check.js",
        "node smoke-check.js | tee out.txt",
        "npx node smoke-check.js",
    ] {
        assert_eq!(target(&dir, command), None, "{command}");
    }
}

#[test]
fn guidance_uses_existing_write_pressure_line_once() {
    let dir = workspace(SMOKE_CHECK, &["src/lib/persistence.js"]);
    let mut prompt = "Repair step `verify`.".to_string();
    append_repair_guidance(
        &mut prompt,
        &report("node smoke-check.js"),
        Some(dir.path()),
    );
    assert!(prompt.ends_with(
        "- write-pressure target: src/lib/persistence.js (selection_reason=diagnosis_mapped)\n"
    ));

    let mut existing =
        "- write-pressure target: src/app/page.tsx (selection_reason=diagnosis_mapped)".to_string();
    let before = existing.clone();
    append_repair_guidance(
        &mut existing,
        &report("node smoke-check.js"),
        Some(dir.path()),
    );
    assert_eq!(existing, before);
}

#[test]
fn unnameable_failure_leaves_prompt_byte_identical() {
    let dir = workspace(SMOKE_CHECK, &["src/lib/persistence.ts"]);
    let mut prompt =
        "Repair step `verify`.\nCommand failures:\n- node smoke-check.js: x".to_string();
    let before = prompt.clone();
    append_repair_guidance(
        &mut prompt,
        &report("node smoke-check.js"),
        Some(dir.path()),
    );
    append_repair_guidance(&mut prompt, &report("node smoke-check.js"), None);
    assert_eq!(prompt, before);
}

#[test]
fn named_target_is_not_a_phase_planner_diagnostic() {
    let dir = workspace(SMOKE_CHECK, &["src/lib/persistence.js"]);
    let verify = report("node smoke-check.js");
    let repair_prompt = crate::planner::repair::build_repair_prompt_with_context(
        "verify-storage",
        &verify,
        &crate::planner::repair::RepairContext {
            workspace_root: Some(dir.path().to_path_buf()),
            ..Default::default()
        },
    );
    assert!(prompt_has_diagnostic(&repair_prompt), "{repair_prompt}");

    let handoff = RecoveryHandoff {
        profile: "generic".to_string(),
        original_goal: "Build the todo app".to_string(),
        failed_phase: Some("persistence".to_string()),
        failed_step: Some("verify-storage".to_string()),
        failure_kind: "implementation".to_string(),
        failure_evidence: verify
            .command_failures
            .iter()
            .map(|failure| format!("{}: {}", failure.command, failure.reason))
            .collect(),
        missing_paths: Vec::new(),
        missing_capabilities: Vec::new(),
        verify_commands: vec!["node smoke-check.js".to_string()],
        changed_paths: vec!["src/lib/persistence.js".to_string()],
        repair_targets: vec!["implementation".to_string()],
    };
    let phase_prompt = render_ultra_recovery_prompt(&handoff);
    assert!(!prompt_has_diagnostic(&phase_prompt), "{phase_prompt}");
}

#[test]
fn corpus_cases_match_expected_targets_and_classification() {
    let case_dir = Path::new("tests/corpus/apps/nextjs-verify-repair-target-from-check");
    let read = |name: &str| -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(case_dir.join(name)).unwrap()).unwrap()
    };
    let workspaces = read("workspaces.json");
    let expected = read("expected.json");
    let cases = workspaces["cases"].as_array().unwrap();
    let expectations = expected["cases"].as_array().unwrap();
    assert_eq!(cases.len(), expectations.len());
    for (case, expectation) in cases.iter().zip(expectations) {
        let name = case["name"].as_str().unwrap();
        let dir = tempfile::tempdir().unwrap();
        for (relative, content) in case["files"].as_object().unwrap() {
            write(dir.path(), relative, content.as_str().unwrap());
        }
        let command = case["command"].as_str().unwrap();
        let stderr = case["stderr"]
            .as_str()
            .unwrap()
            .replace("<workspace>", &dir.path().to_string_lossy());
        assert_eq!(
            target(&dir, command).as_deref(),
            expectation["target"].as_str(),
            "{name}"
        );
        let dependency_missing = build_verifier::is_dependency_missing_output(&stderr)
            && !build_verifier::output_reports_workspace_internal_missing(dir.path(), &stderr);
        assert_eq!(
            dependency_missing,
            expectation["dependency_missing"].as_bool().unwrap(),
            "{name}"
        );
    }
}

#[test]
fn mixed_or_non_command_failures_are_not_named() {
    let dir = workspace(SMOKE_CHECK, &["src/lib/persistence.js"]);
    let mut mixed = report("node smoke-check.js");
    mixed.push_command_failure("grep -q token src/hooks/useTodos.ts", "exit status 1");
    assert_eq!(resolve_target(dir.path(), &mixed), None);

    let mut missing = report("node smoke-check.js");
    missing.missing_paths.push("src/app/page.tsx".to_string());
    assert_eq!(resolve_target(dir.path(), &missing), None);

    let mut prompt = "Repair step `verify`.".to_string();
    let before = prompt.clone();
    append_repair_guidance(&mut prompt, &mixed, Some(dir.path()));
    assert_eq!(prompt, before);
}

#[test]
fn node_failures_loading_different_files_are_not_named() {
    let dir = workspace("require('./a');\n", &["a.js", "b.js"]);
    write(dir.path(), "other-check.js", "require('./b');\n");
    let mut both = report("node smoke-check.js");
    both.push_command_failure("node other-check.js", "AssertionError");
    assert_eq!(resolve_target(dir.path(), &both), None);

    let mut prompt = "Repair step `verify`.".to_string();
    let before = prompt.clone();
    append_repair_guidance(&mut prompt, &both, Some(dir.path()));
    assert_eq!(prompt, before);

    let mut same = report("node smoke-check.js");
    same.push_command_failure("node smoke-check.js", "AssertionError");
    assert_eq!(resolve_target(dir.path(), &same).as_deref(), Some("a.js"));
}

#[cfg(unix)]
#[test]
fn script_symlinked_outside_the_workspace_is_not_named() {
    // The outside script lives next to the workspace and requires a file that
    // resolves back inside it, so only the script location check can refuse.
    let outer = tempfile::tempdir().unwrap();
    let root = outer.path().join("app");
    write(&root, "lib.js", "module.exports = {};\n");
    write(outer.path(), "outside-check.js", "require('./app/lib');\n");
    std::os::unix::fs::symlink(
        outer.path().join("outside-check.js"),
        root.join("link-check.js"),
    )
    .unwrap();
    let mut linked = VerificationReport::pass();
    linked.push_command_failure("node link-check.js", "failed");

    assert_eq!(resolve_target(&root, &linked), None);
}
