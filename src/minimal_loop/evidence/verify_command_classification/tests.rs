use super::super::{SourceFile, collect_workspace_evidence, verify_command_kind};
use super::*;

fn workspace(script: &str) -> WorkspaceEvidence {
    WorkspaceEvidence {
        source_files: vec![SourceFile::new("Smoke Check.cjs".into(), script.into())],
        ..Default::default()
    }
}

#[test]
fn issue439_real_r0_script_is_command_local_test_evidence() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus/apps/issue439-node-evidence");
    let workspace = collect_workspace_evidence(&root);
    for command in ["node smoke-check.js", "node ./smoke-check.js"] {
        assert_eq!(
            verify_command_kind(command, &workspace),
            VerifyCommandKind::Test
        );
    }
    let source = std::fs::read_to_string(root.join("smoke-check.js")).unwrap();
    assert!(source.contains("if (failures.length > 0)"));
    assert!(source.contains("process.exit(1)"));
    assert!(source.contains("process.exit(0)"));
}

#[test]
fn issue439_checks_preserve_case_quotes_and_reject_unrelated_assertions() {
    let mut workspace = workspace("if (!result.ok) { process.exit(1); } else { process.exit(0); }");
    assert_eq!(
        verify_command_kind("node './Smoke Check.cjs'", &workspace),
        VerifyCommandKind::Test
    );
    workspace.test_files.push(SourceFile::new(
        "tests/unrelated.test.js".into(),
        "const assert = require('assert'); assert(true);".into(),
    ));
    for command in [
        "node 'smoke check.cjs'",
        "node absent.js",
        "node -e \"console.log('ok')\"",
        "node '../Smoke Check.cjs'",
        "node 'Smoke Check.cjs' || true",
        "node 'Smoke Check.cjs' | cat",
    ] {
        assert!(
            matches!(
                verify_command_kind(command, &workspace),
                VerifyCommandKind::Weak(_)
            ),
            "{command}"
        );
    }
    workspace.source_files[0] =
        SourceFile::new("Smoke Check.cjs".into(), "console.log('ok');".into());
    assert!(matches!(
        verify_command_kind("node 'Smoke Check.cjs'", &workspace),
        VerifyCommandKind::Weak(_)
    ));
}

#[test]
fn issue439_opaque_or_swallowed_checks_remain_weak() {
    for source in [
        "console.log('ok');",
        "const msg = 'if (!ok) process.exit(1);'; console.log(msg);",
        "// if (!ok) process.exit(1);\nconsole.log('ok');",
        "try { const assert = require('assert'); assert(false); } catch (e) {} process.exit(0);",
        "try { if (!ok) throw Error('failed'); } catch (e) { console.log(e); } process.exit(0);",
        "function unused() { if (!ok) process.exit(1); } console.log('ok');",
        "if (false) { process.exit(1); }",
        "process.exit(0); if (!ok) process.exit(1);",
        "process.exit = () => {}; if (!ok) process.exit(1);",
        "const process = { exit() {} }; if (!ok) process.exit(1);",
    ] {
        assert!(
            matches!(
                verify_command_kind("node 'Smoke Check.cjs'", &workspace(source)),
                VerifyCommandKind::Weak(_)
            ),
            "{source}"
        );
    }
}

#[test]
fn issue439_direct_assertions_and_future_test_artifacts() {
    let workspace = workspace("const assert = require('assert'); assert.equal(1, 1);");
    assert_eq!(
        verify_command_kind("node 'Smoke Check.cjs'", &workspace),
        VerifyCommandKind::Test
    );
    for command in [
        "npm test",
        "npm test -- --runInBand",
        "npm run test",
        "node --test",
        "node --test tests/app.test.js",
        "pnpm test",
        "yarn test",
    ] {
        let mut workspace = WorkspaceEvidence::default();
        assert_eq!(
            verify_command_kind(command, &workspace),
            VerifyCommandKind::Weak("node_test_without_test_artifact".into()),
            "{command}"
        );
        workspace.test_files.push(SourceFile::new(
            "tests/app.test.js".into(),
            "const assert = require('assert'); assert(true);".into(),
        ));
        assert_eq!(
            verify_command_kind(command, &workspace),
            VerifyCommandKind::Test,
            "{command}"
        );
    }
}

#[test]
fn issue439_weak_sources_preserve_commands_and_non_command_origins() {
    let workspace = workspace("console.log('ok');");
    let commands = vec![
        "node 'Smoke Check.cjs'".into(),
        "node -e \"console.log('ok')\"".into(),
    ];
    let reasons = vec![
        "node_smoke_without_assertion".into(),
        "route_unbound:src/unbound.tsx".into(),
        "non_implementation_obligation_only:scaffold".into(),
    ];
    let sources = super::super::weak_sources::collect(&reasons, &commands, &workspace);
    let json = serde_json::to_value(&sources).unwrap();
    assert_eq!(sources.len(), 4);
    assert_eq!(json[0]["source"], "contract_verify_command");
    assert_eq!(json[0]["command"], commands[0]);
    assert_eq!(json[1]["command"], commands[1]);
    assert_eq!(json[2]["source"], "route_unbound");
    assert_eq!(json[3]["source"], "obligation");
}

#[test]
fn issue439_negative_corpus_remains_weak() {
    let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../../tests/corpus/apps/issue439-node-evidence/fixtures/node-negatives.json"
    ))
    .unwrap();
    for case in cases {
        let mut workspace = WorkspaceEvidence::default();
        workspace.source_files.push(SourceFile::new(
            "smoke-check.js".into(),
            case["source"].as_str().unwrap().into(),
        ));
        if case["unrelated_assertion"] == true {
            workspace.test_files.push(SourceFile::new(
                "tests/unrelated.test.js".into(),
                "const assert = require('assert'); assert(true);".into(),
            ));
        }
        let command = case["command"].as_str().unwrap();
        assert!(
            matches!(
                verify_command_kind(command, &workspace),
                VerifyCommandKind::Weak(_)
            ),
            "{case}"
        );
    }
}

#[test]
fn issue439_reviewed_noop_scripts_must_not_supply_test_evidence() {
    let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../../tests/corpus/apps/issue439-node-evidence/fixtures/node-negatives.json"
    ))
    .unwrap();
    let mut promoted = Vec::new();
    for case in cases
        .iter()
        .filter(|case| case["name"].as_str().unwrap().starts_with("review-"))
    {
        let source = case["source"].as_str().unwrap();
        let output = crate::bounded_process::run_with_timeout(
            std::process::Command::new("node").args(["-e", source]),
            std::time::Duration::from_secs(5),
        )
        .unwrap();
        assert!(output.success(), "{case}: {output:?}");
        let kind = verify_command_kind("node 'Smoke Check.cjs'", &workspace(source));
        if !matches!(kind, VerifyCommandKind::Weak(_)) {
            promoted.push(format!("{}: {kind:?}", case["name"]));
        }
    }
    assert!(promoted.is_empty(), "No-op scripts promoted: {promoted:?}");
}
