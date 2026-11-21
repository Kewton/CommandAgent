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

mod issue817 {
    use super::super::super::{WorkspaceEvidence, verify_command_kind};
    use super::super::package_script_check::{formed_predicate_command, predicate, weak_predicate};
    use crate::minimal_loop::evidence::VerifyCommandKind;

    fn weak_command(flag: &str, method: &str, value: &str) -> String {
        format!("node {flag} \"require('./package.json').scripts.dev.{method}('{value}')\"")
    }

    fn kind(command: &str) -> VerifyCommandKind {
        verify_command_kind(command, &WorkspaceEvidence::default())
    }

    #[test]
    fn issue817_grammar_accepts_three_predicates_with_both_print_flags() {
        for flag in ["-p", "--print"] {
            for method in ["includes", "startsWith", "endsWith"] {
                let command = weak_command(flag, method, "next dev -p 3011");
                assert_eq!(
                    weak_predicate(&command),
                    Some(("dev".into(), method.into(), "next dev -p 3011".into())),
                    "{command}"
                );
            }
        }
    }

    #[test]
    fn issue817_grammar_rejects_everything_outside_the_closed_form() {
        let rejected = [
            "node -p \"require('./package.json').scripts[k].includes('3011')\"",
            "node -p \"require('./package.json').scripts.constructor.includes('3011')\"",
            "node -p \"require('./package.json').scripts.__proto__.includes('3011')\"",
            "node -p \"require('./package.json').scripts.dev.includes('30;11')\"",
            "node -p \"require('./package.json').scripts.dev.includes('3011\\'')\"",
            "node -p \"require('./package.json').scripts.dev.includes('$HOME')\"",
            "node -p \"require('./package.json').scripts.dev.includes('`id`')\"",
            "node -p \"require('./package.json').scripts.dev.includes('')\"",
            "node -p \"require('./package.json').scripts.dev.includes(' 3011')\"",
            "node -p \"require('./package.json').scripts.dev.includes('3011');1\"",
            "node -p \"require('./package.json').scripts.dev.includes('3011')\" || true",
            "node -p \"require('./package.json').scripts.dev.includes('3011')\"; echo ok",
            "node -p \"require('./package.json').scripts.dev.includes('3011')\" | cat",
            "node -e \"require('./package.json').scripts.dev.includes('3011')\"",
            "node -p \"require('./package.json').scripts.dev.includes('3011')&&require('./package.json').scripts.start.includes('3011')\"",
            "node -p \"require('./package.json').scripts.dev.includes(port)\"",
            "node -p \"require('./package.json').scripts.dev.includes(3011)\"",
            "node -p \"require('./other.json').scripts.dev.includes('3011')\"",
            "node -p \"require('./package.json').scripts.dev.match('3011')\"",
            "node -p \"require('./package.json').scripts.dev.trim().includes('3011')\"",
            "node -p \"require('./package.json').scripts.dev\"",
            "npm -p \"require('./package.json').scripts.dev.includes('3011')\"",
        ];
        for command in rejected {
            assert_eq!(weak_predicate(command), None, "{command}");
        }
    }

    #[test]
    fn issue817_weak_form_stays_weak_and_formed_form_is_static_syntax() {
        for flag in ["-p", "--print"] {
            for method in ["includes", "startsWith", "endsWith"] {
                let weak = weak_command(flag, method, "3011");
                assert_eq!(
                    kind(&weak),
                    VerifyCommandKind::Weak("node_smoke_without_assertion".into()),
                    "{weak}"
                );
                let formed = formed_predicate_command("dev", method, "3011");
                assert_eq!(
                    predicate(&formed),
                    Some(("dev".into(), method.into(), "3011".into()))
                );
                assert_eq!(kind(&formed), VerifyCommandKind::StaticSyntax, "{formed}");
            }
        }
    }

    #[test]
    fn issue817_hand_weakened_formed_forms_are_weak() {
        let formed = formed_predicate_command("dev", "includes", "3011");
        let weakened = [
            formed.replace(
                "ok(actual.includes(\\\"3011\\\"))",
                "ok(actual.includes(\\\"3011\\\")||process.exit(0))",
            ),
            formed.replace(
                "require('node:assert/strict').ok(actual.includes(\\\"3011\\\"))",
                "process.exit(0)",
            ),
            formed.replace(
                "require('node:assert/strict').strictEqual(typeof actual,'string');",
                "",
            ),
            formed.replace("require('node:assert/strict').ok(", "console.log("),
            formed.replace(";console.log(true)", ";console.log(false)"),
            formed.replace("includes", "indexOf"),
            formed.replace("includes(\\\"3011\\\")", "includes(\\\"30'11\\\")"),
            formed.replace("./package.json", "./other.json"),
            format!("{formed} || true"),
        ];
        for command in weakened {
            assert_ne!(command, formed);
            assert!(predicate(&command).is_none(), "{command}");
            assert!(
                matches!(kind(&command), VerifyCommandKind::Weak(_)),
                "{command}: {:?}",
                kind(&command)
            );
        }
    }

    fn run(dir: &std::path::Path, command: &str) -> (bool, String) {
        let output = crate::bounded_process::run_with_timeout(
            std::process::Command::new("sh")
                .args(["-c", command])
                .current_dir(dir)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped()),
            std::time::Duration::from_secs(10),
        )
        .unwrap();
        (
            output.success(),
            String::from_utf8_lossy(&output.stdout).into_owned(),
        )
    }

    #[test]
    fn issue817_formed_form_passes_only_when_the_original_prints_true() {
        let cases: [(&str, Option<&str>, bool); 9] = [
            (
                "true",
                Some(r#"{"scripts":{"dev":"next dev -p 3011"}}"#),
                true,
            ),
            (
                "false",
                Some(r#"{"scripts":{"dev":"vite --port 4000"}}"#),
                false,
            ),
            (
                "undefined",
                Some(r#"{"scripts":{"start":"next start"}}"#),
                false,
            ),
            ("number", Some(r#"{"scripts":{"dev":3011}}"#), false),
            (
                "array",
                Some(r#"{"scripts":{"dev":["next dev -p 3011"]}}"#),
                false,
            ),
            (
                "object",
                Some(r#"{"scripts":{"dev":{"includes":"3011"}}}"#),
                false,
            ),
            ("no_scripts", Some(r#"{}"#), false),
            ("invalid_json", Some("{"), false),
            ("no_package_json", None, false),
        ];
        for (name, package, formed_passes) in cases {
            for method in ["includes", "startsWith", "endsWith"] {
                let lit = match method {
                    "includes" => "-p 3011",
                    "startsWith" => "next dev",
                    _ => "3011",
                };
                let dir = tempfile::tempdir().unwrap();
                if let Some(package) = package {
                    std::fs::write(dir.path().join("package.json"), package).unwrap();
                }
                let weak = run(dir.path(), &weak_command("-p", method, lit));
                let formed = run(dir.path(), &formed_predicate_command("dev", method, lit));
                assert_eq!(formed.0, formed_passes, "{name}/{method}: {formed:?}");
                if formed.0 {
                    assert_eq!(weak, (true, "true\n".into()), "{name}/{method}");
                    assert_eq!(formed.1, weak.1, "{name}/{method}");
                } else if name == "false" {
                    // The weak original exits zero for a false predicate.
                    assert_eq!(weak, (true, "false\n".into()), "{name}/{method}");
                }
            }
        }
    }
}
