use std::path::Path;

use super::{
    Check, PortScript, build_check_command, checked_port, dev_port_check_command_for, evaluate,
    evaluate_in_profile, is_check_command, is_legacy_inline_command, legacy_inline_command,
    legacy_inline_covers, list_has_declared_check, parse, port_check_command, run_step_check,
    same_declared_check,
};
use crate::planner::profile_descriptor::NEXTJS_PROFILE_ID;
use crate::planner::step_plan::PlanStep;
use crate::planner::verify::VerificationReport;

fn former_port_command(script: PortScript, port: u16) -> String {
    legacy_inline_command(Check::PortScript { script, port })
}

fn node_available() -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join("node").is_file()))
}

fn workspace(package_json: Option<&str>) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    if let Some(text) = package_json {
        std::fs::write(dir.path().join("package.json"), text).unwrap();
    }
    dir
}

/// Runs the former inline check through the production verifier, as the host did.
fn node_passes(root: &Path, command: &str) -> bool {
    let step = step(vec![command.to_string()]);
    crate::planner::verify::verify_step(root, &step).is_pass()
}

fn manifest(scripts: &str) -> String {
    format!("{{\"scripts\":{scripts}}}")
}

#[test]
fn tokens_use_the_check_namespace_and_stay_off_the_shell() {
    assert_eq!(
        port_check_command(PortScript::Dev, 3011),
        "anvil-nextjs-check:package_script_port:dev:3011"
    );
    assert_eq!(
        port_check_command(PortScript::Start, 3011),
        "anvil-nextjs-check:package_script_port:start:3011"
    );
    assert_eq!(build_check_command(), "anvil-nextjs-check:build_script");
    for command in [
        port_check_command(PortScript::Dev, 3011),
        port_check_command(PortScript::Start, 4321),
        build_check_command(),
    ] {
        assert!(!command.contains("node"), "{command}");
        assert!(!command.contains(char::is_whitespace), "{command}");
        assert!(is_check_command(&command), "{command}");
        assert!(parse(&command).is_some(), "{command}");
    }
    assert_eq!(
        parse("anvil-nextjs-check:package_script_port:start:4321"),
        Some(Check::PortScript {
            script: PortScript::Start,
            port: 4321
        })
    );
    assert_eq!(
        parse("anvil-nextjs-check:build_script"),
        Some(Check::BuildScript)
    );
}

#[test]
fn an_altered_token_is_owned_by_the_host_but_never_parses() {
    for altered in [
        "anvil-nextjs-check:",
        "anvil-nextjs-check:build",
        "anvil-nextjs-check:unknown_check",
        "anvil-nextjs-check:build_script:extra",
        "anvil-nextjs-check:build_script --flag",
        "anvil-nextjs-check:package_script_port",
        "anvil-nextjs-check:package_script_port:dev",
        "anvil-nextjs-check:package_script_port:build:3011",
        "anvil-nextjs-check:package_script_port:dev:3011:x",
        "anvil-nextjs-check:package_script_port:dev:03011",
        "anvil-nextjs-check:package_script_port:dev:+3011",
        "anvil-nextjs-check:package_script_port:dev:99999",
        "anvil-nextjs-check:package_script_port:dev:",
        "anvil-nextjs-check:package_script_port:dev:3011 && true",
        "anvil-nextjs-check:package_script_port:dev:$(id)",
    ] {
        assert!(is_check_command(altered), "{altered}");
        assert_eq!(parse(altered), None, "{altered}");
        let root = workspace(Some(&manifest(
            r#"{"dev":"next dev -p 3011","build":"next build"}"#,
        )));
        let error = evaluate(root.path(), altered).unwrap_err();
        assert!(error.contains("malformed"), "{altered}: {error}");
    }
    assert!(!is_check_command("node -p 1"));
    assert!(!is_check_command("npm run build"));
    assert!(!is_check_command("echo anvil-nextjs-check:build_script"));
}

#[test]
fn an_unparsable_catalog_port_fails_closed_without_node() {
    let command = dev_port_check_command_for("99999");
    assert!(is_check_command(&command), "{command}");
    assert_eq!(parse(&command), None);
    assert!(!command.contains("node"), "{command}");
    assert_eq!(
        dev_port_check_command_for("3011"),
        port_check_command(PortScript::Dev, 3011)
    );
}

#[test]
fn dev_start_and_build_judgements_follow_the_declared_contract() {
    let root = workspace(Some(&manifest(
        r#"{"dev":"next dev -p 3011","start":"next start --port=3011","build":"next build"}"#,
    )));
    for command in [
        port_check_command(PortScript::Dev, 3011),
        port_check_command(PortScript::Start, 3011),
        build_check_command(),
    ] {
        assert_eq!(evaluate(root.path(), &command), Ok(()), "{command}");
    }
    let other_port = evaluate(root.path(), &port_check_command(PortScript::Dev, 3000));
    assert!(other_port.unwrap_err().contains("3000"));
}

#[test]
fn rust_judgement_is_the_former_node_predicate_or_stricter() {
    // (package.json text, expect the string-only forms to match node exactly)
    let dev_cases = [
        r#"{"dev":"next dev -p 3011"}"#,
        r#"{"dev":"next dev -p3011"}"#,
        r#"{"dev":"next dev --port 3011"}"#,
        r#"{"dev":"next dev --port=3011"}"#,
        r#"{"dev":"next dev  -p 3011"}"#,
        r#"{"dev":"next dev -p  3011"}"#,
        r#"{"dev":"next  dev -p 3011"}"#,
        r#"{"dev":"next dev --turbo -p 3011"}"#,
        r#"{"dev":"next  dev"}"#,
        r#"{"dev":"next dev"}"#,
        r#"{"dev":"next start -p 3011"}"#,
        r#"{"dev":"next dev -p 3000"}"#,
        r#"{"dev":"next dev --port=30110"}"#,
        r#"{"dev":"next dev --port=3011x"}"#,
        r#"{"dev":"next dev -p"}"#,
        r#"{"dev":"nextdev -p 3011"}"#,
        r#"{"dev":"PORT=3011 next dev"}"#,
        r#"{"dev":""}"#,
        r#"{"dev":5}"#,
        r#"{"dev":["next dev -p 3011"]}"#,
        r#"{"dev":null}"#,
        r#"{"build":"next build"}"#,
        r#"{}"#,
        r#"[]"#,
        r#"{"scripts":"next dev -p 3011"}"#,
        r#"{"scripts":{"dev":"next dev -p 3011"}"#,
        "",
    ];
    let start_cases = [
        r#"{"start":"next start -p 3011"}"#,
        r#"{"start":"next start --port 3011"}"#,
        r#"{"start":"next start"}"#,
        r#"{"start":"next dev -p 3011"}"#,
        r#"{"start":""}"#,
        r#"{"start":null}"#,
        r#"{"start":0}"#,
        r#"{"start":false}"#,
        r#"{"start":5}"#,
        r#"{"start":true}"#,
        r#"{"start":["next start -p 3011"]}"#,
        r#"{"start":{"a":1}}"#,
        r#"{"dev":"next dev -p 3011"}"#,
        r#"{}"#,
        r#"{"scripts":"x"}"#,
        r#"{"scripts":{"start":"#,
    ];
    let build_cases = [
        r#"{"build":"next build"}"#,
        r#"{"build":"next build --lint"}"#,
        r#"{"build":"next  build"}"#,
        r#"{"build":" next build"}"#,
        r#"{"build":5}"#,
        r#"{"build":["next build"]}"#,
        r#"{"build":null}"#,
        r#"{"dev":"next dev"}"#,
        r#"{}"#,
        r#"{"scripts":{"build":"next build""#,
    ];
    // Values that are not strings are where the host is deliberately stricter than
    // the JS `String()` coercion; every other form must agree with node.
    let non_string = |text: &str| {
        serde_json::from_str::<serde_json::Value>(text)
            .ok()
            .and_then(|value| value.get("scripts").cloned())
            .and_then(|scripts| scripts.as_object().cloned())
            .is_some_and(|scripts| {
                scripts
                    .values()
                    .any(|value| !value.is_string() && !value.is_null())
            })
    };
    let mut jobs: Vec<(String, String, String)> = Vec::new();
    for case in dev_cases {
        jobs.push((
            case.to_string(),
            port_check_command(PortScript::Dev, 3011),
            former_port_command(PortScript::Dev, 3011),
        ));
    }
    for case in start_cases {
        jobs.push((
            case.to_string(),
            port_check_command(PortScript::Start, 3011),
            former_port_command(PortScript::Start, 3011),
        ));
    }
    for case in build_cases {
        jobs.push((
            case.to_string(),
            build_check_command(),
            legacy_inline_command(Check::BuildScript),
        ));
    }
    let wrapped = |case: &str| {
        // The cases above spell the scripts object; wrap bare script maps.
        match serde_json::from_str::<serde_json::Value>(case) {
            Ok(value) if value.get("scripts").is_none() && value.is_object() => {
                let scripts = value.as_object().unwrap();
                if scripts.is_empty() {
                    case.to_string()
                } else {
                    manifest(case)
                }
            }
            _ => case.to_string(),
        }
    };
    let have_node = node_available();
    if !have_node {
        eprintln!("SKIPPED: node is not available; the Rust-only expectations below still run");
    }
    for (case, token, former) in jobs {
        let text = wrapped(&case);
        let package_json = (!text.is_empty()).then_some(text.as_str());
        let root = workspace(package_json);
        let rust = evaluate(root.path(), &token).is_ok();
        if !have_node {
            continue;
        }
        let node = node_passes(root.path(), &former);
        assert!(!rust || node, "Rust passes but node fails: {text} {token}");
        if !non_string(&text) {
            assert_eq!(rust, node, "{text} {token}");
        }
    }
}

#[test]
fn a_missing_package_json_fails_every_check() {
    let root = workspace(None);
    for command in [
        port_check_command(PortScript::Dev, 3011),
        port_check_command(PortScript::Start, 3011),
        build_check_command(),
    ] {
        let error = evaluate(root.path(), &command).unwrap_err();
        assert!(error.contains("package.json"), "{command}: {error}");
    }
}

#[test]
fn a_byte_order_mark_is_unreadable_and_fails() {
    let root = workspace(Some("\u{feff}{\"scripts\":{\"build\":\"next build\"}}"));
    assert!(evaluate(root.path(), &build_check_command()).is_err());
}

#[test]
fn the_judgement_is_bound_to_the_nextjs_profile() {
    let root = workspace(Some(&manifest(
        r#"{"dev":"next dev -p 3011","build":"next build"}"#,
    )));
    let command = port_check_command(PortScript::Dev, 3011);
    assert_eq!(
        evaluate_in_profile(root.path(), Some(NEXTJS_PROFILE_ID), &command),
        Ok(())
    );
    for profile in [None, Some("python-cli"), Some("data"), Some("ingest")] {
        let error = evaluate_in_profile(root.path(), profile, &command).unwrap_err();
        assert!(
            error.contains("outside the active nextjs profile"),
            "{error}"
        );
    }
}

fn step(verify: Vec<String>) -> PlanStep {
    PlanStep {
        id: "nextjs-profile-verify".to_string(),
        kind: "verify".to_string(),
        expected_result: "pass".to_string(),
        instruction: "Verify package scripts".to_string(),
        expected_paths: Vec::new(),
        verify,
    }
}

#[test]
fn the_step_check_reports_a_failing_token_and_ignores_other_commands() {
    let root = workspace(Some(&manifest(
        r#"{"dev":"next dev -p 3000","build":"next build"}"#,
    )));
    let failing = port_check_command(PortScript::Dev, 3011);
    let step = step(vec![
        build_check_command(),
        failing.clone(),
        "npm run build".to_string(),
    ]);
    let mut report = VerificationReport::pass();
    run_step_check(root.path(), Some(NEXTJS_PROFILE_ID), &step, &mut report);
    assert_eq!(report.command_failures.len(), 1, "{report:?}");
    assert_eq!(report.command_failures[0].command, failing);

    let mut report = VerificationReport::pass();
    run_step_check(root.path(), Some("python-cli"), &step, &mut report);
    assert_eq!(report.command_failures.len(), 2, "{report:?}");
}

#[test]
fn legacy_inline_forms_stay_recognized_for_saved_plans_but_are_never_emitted() {
    // Byte-identical to the dev-required builder recorded by Issue #832.
    const RECORDED_DEV_3011: &str = "node -p \"['dev'].every(function(k){return String(Object(require('./package.json').scripts)[k]).split(' ').some(function(t,i,a){return t=='next' ? a.slice(i+1).find(function(x){return x})==k : false}) ? String(Object(require('./package.json').scripts)[k]).split(' ').some(function(t,i,a){return t=='--port=3011' ? true : t=='-p' ? a.slice(i+1).find(function(x){return x})=='3011' : t=='-p3011' ? true : t=='--port' ? a.slice(i+1).find(function(x){return x})=='3011' : false}) : false}) ? true : process.exit(1)\"";
    assert_eq!(
        legacy_inline_command(Check::PortScript {
            script: PortScript::Dev,
            port: 3011
        }),
        RECORDED_DEV_3011
    );
    let mut legacy = vec![legacy_inline_command(Check::BuildScript)];
    for script in [PortScript::Dev, PortScript::Start] {
        legacy.push(legacy_inline_command(Check::PortScript {
            script,
            port: 4321,
        }));
    }
    for command in &legacy {
        assert!(is_legacy_inline_command(command), "{command}");
        assert!(!is_check_command(command), "{command}");
        assert_eq!(parse(command), None, "{command}");
        assert!(legacy_inline_covers(command, 4321), "{command}");
        assert!(
            !is_legacy_inline_command(&format!("{command} ")),
            "{command}"
        );
    }
    assert!(!legacy_inline_covers(&legacy[1], 3011));
    assert!(!is_legacy_inline_command("node -p \"process.exit(1)\""));
    // The host assembles tokens only.
    assert!(!port_check_command(PortScript::Dev, 4321).starts_with("node"));
    assert!(!build_check_command().starts_with("node"));
}

#[test]
fn a_saved_legacy_text_is_the_same_declared_check_only_when_it_is_byte_identical() {
    let dev = port_check_command(PortScript::Dev, 3011);
    let legacy_dev = legacy_inline_command(Check::PortScript {
        script: PortScript::Dev,
        port: 3011,
    });
    assert!(same_declared_check(&dev, &legacy_dev));
    assert!(same_declared_check(&legacy_dev, &dev));
    assert!(same_declared_check(&dev, &dev));
    assert!(same_declared_check(
        &build_check_command(),
        &legacy_inline_command(Check::BuildScript)
    ));
    // A one-character difference anywhere is a different command.
    for index in (0..legacy_dev.len()).step_by(7) {
        let mut altered = legacy_dev.clone();
        altered.remove(index);
        assert!(!same_declared_check(&dev, &altered), "removed at {index}");
        assert!(!same_declared_check(&altered, &dev), "removed at {index}");
    }
    assert!(!same_declared_check(
        &dev,
        &legacy_dev.replace("3011", "3012")
    ));
    assert!(!same_declared_check(&dev, &format!("{legacy_dev} ")));
    // The other script, the other port and the build check are different declarations.
    assert!(!same_declared_check(
        &dev,
        &legacy_inline_command(Check::PortScript {
            script: PortScript::Start,
            port: 3011
        })
    ));
    assert!(!same_declared_check(
        &dev,
        &legacy_inline_command(Check::PortScript {
            script: PortScript::Dev,
            port: 3012
        })
    ));
    assert!(!same_declared_check(&build_check_command(), &legacy_dev));
    assert!(!same_declared_check("npm run build", &legacy_dev));
    // Two legacy texts are not equivalent to each other: only token and text pair up.
    assert!(!same_declared_check(
        &legacy_dev,
        &legacy_inline_command(Check::BuildScript)
    ));
    let saved = vec![legacy_dev.clone(), "npm run build".to_string()];
    assert!(list_has_declared_check(&saved, &dev));
    assert!(!list_has_declared_check(
        &saved,
        &port_check_command(PortScript::Dev, 3012)
    ));
    assert_eq!(checked_port(&dev), Some(3011));
    assert_eq!(checked_port(&legacy_dev), Some(3011));
    assert_eq!(checked_port(&build_check_command()), None);
}

#[test]
fn the_corpus_table_agrees_with_the_host_judgement() {
    let table: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tests/corpus/apps/nextjs-verify-host-check-repeated/package-json-table.json"
    ))
    .unwrap();
    let declared: Vec<String> = serde_json::from_str(include_str!(
        "../../../../tests/corpus/apps/nextjs-verify-host-check-repeated/declared-verify.json"
    ))
    .unwrap();
    assert_eq!(
        declared,
        [
            port_check_command(PortScript::Dev, 3011),
            port_check_command(PortScript::Start, 3011),
            build_check_command(),
            "npm run build".to_string()
        ]
    );
    let cases = table["cases"].as_array().unwrap();
    assert!(cases.len() >= 10);
    for case in cases {
        let root = workspace(case["package_json"].as_str());
        let name = case["name"].as_str().unwrap();
        for (key, token) in [
            ("dev", &declared[0]),
            ("start", &declared[1]),
            ("build", &declared[2]),
        ] {
            assert_eq!(
                evaluate(root.path(), token).is_ok(),
                case[key].as_bool().unwrap(),
                "{name}: {key}"
            );
        }
    }
}
