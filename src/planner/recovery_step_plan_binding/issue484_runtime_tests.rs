use super::*;
use crate::minimal_loop::evidence::command_diagnosis;

fn execute(root: &Path, command: &str) -> crate::planner::verify::VerificationReport {
    let mut step = saved(true).steps[4].clone();
    step.kind = "verify".into();
    step.expected_paths.clear();
    step.verify = vec![command.into()];
    crate::planner::verify::verify_step(root, &step)
}

fn raw(root: &Path, command: &str) -> crate::bounded_process::BoundedProcessOutput {
    crate::bounded_process::run_with_timeout(
        std::process::Command::new("sh")
            .args(["-c", command])
            .current_dir(root)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped()),
        std::time::Duration::from_secs(5),
    )
    .unwrap()
}

#[test]
fn issue484_real_node_preserves_stdout_and_propagates_every_runtime_failure() {
    let root = tempfile::tempdir().unwrap();
    let original = saved(false).steps[4].verify[0].clone();
    let command = check();
    let package = root.path().join("package.json");
    for (case, data, passes) in [
        ("matching", Some(fixture("package.json")), true),
        ("script_missing", Some("{\"scripts\":{}}".into()), false),
        ("scripts_missing", Some("{}".into()), false),
        (
            "mismatch",
            Some(fixture("package.json").replace("60302", "3000")),
            false,
        ),
        ("file_missing", None, false),
        ("invalid_json", Some("not JSON".into()), false),
    ] {
        if let Some(data) = data {
            std::fs::write(&package, data).unwrap();
        } else {
            std::fs::remove_file(&package).unwrap();
        }
        let result = execute(root.path(), &command);
        assert_eq!(result.is_pass(), passes, "{case}: {result:?}");
        let actual = raw(root.path(), &command);
        assert_eq!(actual.success(), passes, "{case}");
        if passes {
            let before = raw(root.path(), &original);
            assert!(before.success());
            assert_eq!(actual.stdout, before.stdout);
            assert_eq!(actual.stdout, b"next dev -p 60302\n");
            assert!(actual.stderr.is_empty());
        } else {
            assert!(actual.stdout.is_empty(), "{case} emitted success output");
            assert!(!actual.stderr.is_empty());
            if case == "file_missing" {
                // Existing verifier reporting classifies MODULE_NOT_FOUND as
                // DependencyMissing; raw Node still proves the nonzero failure.
                assert!(!result.dependency_missing.is_empty(), "{result:?}");
            } else {
                assert!(
                    result.command_failures.iter().any(|f| f.command == command),
                    "{case}: {result:?}"
                );
            }
            if matches!(case, "file_missing" | "invalid_json" | "scripts_missing") {
                assert!(
                    !raw(root.path(), &original).success(),
                    "original failure must also propagate"
                );
            }
        }
    }
}

#[test]
fn issue484_allowed_literal_characters_survive_shell_and_node_unchanged() {
    let root = tempfile::tempdir().unwrap();
    let c = config(root.path());
    // Exercise every accepted punctuation, including JSON-independent shell
    // boundaries. Characters outside this ASCII grammar are refusal fixtures.
    for value in [
        "next dev --port=60302",
        "A_z09 ./bin/run-js --arg=value:name @scope/pkg +ok",
    ] {
        assert!(grammar::literal(value));
        let before = single_owner(&format!(
            "Update package.json scripts so that dev is '{value}'."
        ));
        let mut after = before.clone();
        let command = grammar::formed_command("dev", value);
        after.steps[0].verify = vec![command.clone()];
        assert!(matches!(
            direct(&c, &before, &after).0,
            admission::Decision::Ready(false)
        ));
        std::fs::write(
            root.path().join("package.json"),
            json!({"scripts":{"dev":value}}).to_string(),
        )
        .unwrap();
        let result = raw(root.path(), &command);
        assert!(result.success(), "{result:?}");
        assert_eq!(result.stdout, format!("{value}\n").as_bytes());
        assert!(execute(root.path(), &command).is_pass());
    }
}

#[test]
fn issue484_structural_check_cannot_supply_test_or_implementation_credit() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("package.json"), fixture("package.json")).unwrap();
    let command = check();
    assert_eq!(
        command_diagnosis::collect(root.path(), std::slice::from_ref(&command))[0].kind,
        "static_syntax"
    );
    assert_eq!(
        command_diagnosis::collect(root.path(), std::slice::from_ref(&command))[0].repairability,
        "runtime_input_dependent"
    );
    assert!(execute(root.path(), &command).is_pass());
    let contract: CompletionContract = serde_json::from_value(json!({
        "required_paths":["package.json"], "verify_commands":[command],
        "required_evidence":["implementation_artifact"]
    }))
    .unwrap();
    let report = contract.verify_with_goal(root.path(), "Implement application behavior");
    assert!(!report.is_pass(), "{report:?}");
    assert!(
        report.primary_reason().contains("implementation_artifact"),
        "{report:?}"
    );
    // An existing genuine target-dependent business check keeps its Test class.
    let business = "node -e \"import('./src/calculate.mjs').then(actual=>{require('node:assert/strict').deepStrictEqual(actual.double(...[21]),42)})\"";
    assert_eq!(
        command_diagnosis::collect(root.path(), &[business.into()])[0].kind,
        "test"
    );
}

#[test]
fn issue484_registered_contract_conflicts_and_frozen_hash_are_preserved() {
    let root = tempfile::tempdir().unwrap();
    let mut c = config(root.path());
    let guard = authority::begin_run(&c);
    let path = generated_contract(&c);
    let before = single_owner(&saved(false).steps[4].instruction);
    let mut after = before.clone();
    after.steps[0].verify = vec![check()];
    std::fs::write(
        &path,
        json!({"profile":"generic","verify_commands":[check().replace("60302", "3000")]})
            .to_string(),
    )
    .unwrap();
    let (decision, admission) = direct(&c, &before, &after);
    assert!(matches!(decision, admission::Decision::Retry(_)));
    assert!(admission.finish(&c, None, after.clone()).is_err());
    assert!(events(&c).iter().any(|e| {
        e["reason"]
            .as_str()
            .is_some_and(|s| s.contains("registered completion contract"))
    }));

    std::fs::write(
        &path,
        json!({"profile":"generic","verify_commands":[]}).to_string(),
    )
    .unwrap();
    let (decision, admission) = direct(&c, &before, &after);
    assert!(matches!(decision, admission::Decision::Ready(false)));
    let formed = admission.finish(&c, None, after).unwrap();
    scope::register(&c, &formed).unwrap();
    c.completion_contract_path = Some(path.clone());
    let bytes = std::fs::read(&path).unwrap();
    let contract = CompletionContract::load_for_config(&c).unwrap().unwrap();
    assert_eq!(contract.verify_commands, [check()]);
    let digest = hash(&bytes);
    let mut changed = formed.clone();
    changed.steps[0].verify[0] = check().replace("60302", "3000");
    scope::register(&c, &changed).unwrap();
    assert_eq!(hash(&std::fs::read(&path).unwrap()), digest);
    std::fs::write(root.path().join("package.json"), fixture("package.json")).unwrap();
    assert!(
        contract
            .verify_with_goal(root.path(), "Check package structure")
            .is_pass()
    );
    std::fs::write(
        root.path().join("package.json"),
        fixture("package.json").replace("60302", "3000"),
    )
    .unwrap();
    let failed = contract.verify_with_goal(root.path(), "Check package structure");
    assert!(!failed.is_pass());
    assert!(failed.command_failures.iter().any(|f| f.command == check()));
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    drop(guard);
}

#[test]
fn issue484_host_port_cannot_supply_literal_or_override_explicit_model_value() {
    let root = tempfile::tempdir().unwrap();
    let c = super::issue478::nextjs_config(root.path());
    for (instruction, accepted) in [
        ("Maintain package.json.", false),
        (
            "Update package.json scripts so that dev is 'next dev -p 3000'.",
            false,
        ),
        (
            "Update package.json scripts so that dev is 'next dev --port 60302'.",
            true,
        ),
    ] {
        let mut before = single_owner(instruction);
        before.goal = "Maintain package scripts on port 60302".into();
        let mut admission = admission::Admission::default();
        admission.strengthen(&c, &mut before);
        assert!(matches!(
            admission
                .check(&c, None, &before.clone(), &mut before.clone(), 1)
                .unwrap(),
            admission::Decision::Retry(_)
        ));
        let mut after = before.clone();
        after.steps[0].verify = vec![grammar::formed_command(
            "dev",
            if accepted {
                "next dev --port 60302"
            } else {
                "next dev -p 60302"
            },
        )];
        let decision = admission
            .check(&c, None, &after.clone(), &mut after, 2)
            .unwrap();
        assert_eq!(
            matches!(decision, admission::Decision::Ready(false)),
            accepted,
            "{instruction}"
        );
    }
}
