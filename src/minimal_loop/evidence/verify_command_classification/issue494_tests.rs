use super::super::{collect_workspace_evidence, verify_command_kind};
use super::*;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const FIXTURE: &str = "tests/corpus/apps/issue494-require-formation";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE)
}

fn cases() -> Value {
    serde_json::from_str(&std::fs::read_to_string(root().join("cases.json")).unwrap()).unwrap()
}

fn command(case: &str) -> String {
    let cases = cases();
    let command = cases[case]["command"].as_str().unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(command.as_bytes())),
        cases[case]["sha256"],
        "fixture command hash drifted for {case}"
    );
    command.into()
}

fn run(cwd: &Path, command: &str) -> crate::bounded_process::BoundedProcessOutput {
    crate::bounded_process::run_with_timeout(
        std::process::Command::new("sh")
            .args(["-c", command])
            .current_dir(cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped()),
        std::time::Duration::from_secs(5),
    )
    .unwrap()
}

#[test]
fn issue494_exact_require_print_is_recognized_but_remains_weak() {
    let original = command("original");
    assert_eq!(
        import_check::pure_require_target(&original).as_deref(),
        Some("./src/lib/types.ts")
    );
    assert_eq!(
        verify_command_kind(&original, &WorkspaceEvidence::default()),
        VerifyCommandKind::Weak("node_smoke_without_assertion".into())
    );
    assert_eq!(
        import_check::require_export_set_target(&command("positive")).as_deref(),
        Some("./src/lib/types.ts")
    );
    assert_eq!(
        verify_command_kind(&command("positive"), &WorkspaceEvidence::default()),
        VerifyCommandKind::StaticSyntax
    );
}

#[test]
fn issue494_require_grammar_rejects_every_frozen_negative() {
    let cases = cases();
    for case in cases["negative_originals"].as_array().unwrap() {
        let command = case["command"].as_str().unwrap();
        assert_eq!(
            import_check::pure_require_target(command),
            None,
            "{}",
            case["name"]
        );
    }
    for case in cases["negative_replacements"].as_array().unwrap() {
        let command = case["command"].as_str().unwrap();
        if case["name"] == "wrong_target" {
            assert_eq!(
                import_check::require_export_set_target(command).as_deref(),
                Some("./src/lib/store.ts")
            );
        } else {
            assert_eq!(
                import_check::require_export_set_target(command),
                None,
                "{}",
                case["name"]
            );
        }
    }
}

#[test]
fn issue494_runtime_preserves_require_failures_and_printed_value() {
    let fixture = root();
    assert_eq!(
        String::from_utf8(run(&fixture, "node --version").stdout)
            .unwrap()
            .trim(),
        cases()["runtime"]["node_version"].as_str().unwrap()
    );
    let original = "node -p \"require('./module.cjs')\"";
    let strengthened = "node -p \"const actual=require('./module.cjs');require('node:assert/strict').deepStrictEqual(Object.keys(actual).sort(),[\\\"value\\\"]);actual\"";
    let original_output = run(&fixture, original);
    let strengthened_output = run(&fixture, strengthened);
    assert!(original_output.success(), "{original_output:?}");
    assert!(strengthened_output.success(), "{strengthened_output:?}");
    assert_eq!(strengthened_output.stdout, original_output.stdout);

    let missing_original = run(&fixture, "node -p \"require('./missing.cjs')\"");
    let missing_strengthened = run(
        &fixture,
        "node -p \"const actual=require('./missing.cjs');require('node:assert/strict').deepStrictEqual(Object.keys(actual).sort(),[]);actual\"",
    );
    assert!(!missing_original.success());
    assert!(!missing_strengthened.success());

    let wrong_export = run(
        &fixture,
        "node -p \"const actual=require('./module.cjs');require('node:assert/strict').deepStrictEqual(Object.keys(actual).sort(),[]);actual\"",
    );
    assert!(!wrong_export.success());

    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("package.json"),
        "{\"type\":\"commonjs\"}\n",
    )
    .unwrap();
    std::fs::write(
        temp.path().join("broken.cjs"),
        "throw new Error('evaluation failed');\n",
    )
    .unwrap();
    for command in [
        "node -p \"require('./broken.cjs')\"",
        "node -p \"const actual=require('./broken.cjs');require('node:assert/strict').deepStrictEqual(Object.keys(actual).sort(),[]);actual\"",
    ] {
        let output = run(temp.path(), command);
        assert!(!output.success(), "{command}: {output:?}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("evaluation failed"));
    }
}

#[test]
fn issue494_top_level_await_keeps_cross_loader_control_outside_correspondence() {
    let fixture = root();
    let original = run(&fixture, "node -p \"require('./esm/entry.mjs')\"");
    let strengthened = run(
        &fixture,
        "node -p \"const actual=require('./esm/entry.mjs');require('node:assert/strict').deepStrictEqual(Object.keys(actual).sort(),[\\\"ready\\\"]);actual\"",
    );
    let dynamic = run(
        &fixture,
        "node -e \"import('./esm/entry.mjs').then(actual=>require('node:assert/strict').deepStrictEqual(Object.keys(actual).sort(),['ready']))\"",
    );
    assert!(!original.success(), "{original:?}");
    assert!(!strengthened.success(), "{strengthened:?}");
    assert!(dynamic.success(), "{dynamic:?}");
    assert!(String::from_utf8_lossy(&original.stderr).contains("ERR_REQUIRE_ASYNC_MODULE"));

    let workspace = collect_workspace_evidence(&fixture);
    assert!(matches!(
        verify_command_kind("node -p \"require('./esm/entry.mjs')\"", &workspace),
        VerifyCommandKind::Weak(_)
    ));
}
