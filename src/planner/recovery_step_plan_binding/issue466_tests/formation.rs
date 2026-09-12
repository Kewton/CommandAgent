use super::*;

#[test]
fn issue466_generated_formation_retains_mixed_requirements_and_finite_budget() {
    let root = tempfile::tempdir().unwrap();
    let c = config(root.path());
    let mut mixed = step("mixed", "implement", &[SCRIPT, "package.json", "app.js"]);
    mixed.instruction = format!(
        "{INSTRUCTION} Also create package.json and app.js without dropping application behavior."
    );
    mixed.verify = vec![format!("node {SCRIPT}"), "test -f app.js".into()];
    let before = plan(vec![mixed.clone()]);
    let mut producer = step("verifier", "implement", &[SCRIPT]);
    producer.instruction = format!(
        "Only create the verifier in this step. Original requirement context: {}",
        mixed.instruction
    );
    let mut application = step("app-config", "implement", &["package.json", "app.js"]);
    application.instruction = format!(
        "Implement the application/configuration outputs in this step. Original requirement context: {}",
        mixed.instruction
    );
    let mut confirmation = mixed.clone();
    confirmation.id = "confirm-original-group".into();
    confirmation.kind = "verify".into();
    let after = plan(vec![producer, application, confirmation]);
    let mut planner = Replay::new(vec![proposal(&before), proposal(&after)]);
    let p = crate::planner::runner::generate_step_plan(&mut planner, &before.goal, &c)
        .unwrap_or_else(|e| panic!("{e}; events={:?}", events(&c)));
    assert_eq!(planner.requests.lock().unwrap().len(), 2);
    assert_eq!(
        p.steps
            .iter()
            .flat_map(|s| s.expected_paths.iter())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        3
    );
    assert!(
        events(&c)
            .iter()
            .any(|e| e["event"] == "recovery_verifier_obligations_formed")
    );
    let mut dropped = after.clone();
    dropped.steps[1].expected_paths.clear();
    let mut planner = Replay::new(vec![
        proposal(&before),
        proposal(&dropped),
        proposal(&dropped),
    ]);
    assert!(crate::planner::runner::generate_step_plan(&mut planner, &before.goal, &c).is_err());
    assert_eq!(planner.requests.lock().unwrap().len(), 3);
    // Putting the original text on an unrelated owner cannot discharge app work.
    let mut misplaced = after.clone();
    misplaced.steps[1].instruction = "Create files without the original requirements".into();
    misplaced
        .steps
        .push(step("unrelated", "report", &["notes.txt"]));
    misplaced.steps[3].instruction = mixed.instruction;
    let mut planner = Replay::new(vec![
        proposal(&before),
        proposal(&misplaced),
        proposal(&misplaced),
    ]);
    assert!(crate::planner::runner::generate_step_plan(&mut planner, &before.goal, &c).is_err());
    assert!(
        serde_json::to_string(&planner.requests.lock().unwrap()[2])
            .unwrap()
            .contains("lost original requirements")
    );
}

#[test]
fn issue466_host_augmentation_is_measured_before_obligation_closure() {
    let root = tempfile::tempdir().unwrap();
    let mut c = config(root.path());
    c.profile = crate::planner::profile_descriptor::NEXTJS_PROFILE_ID.into();
    let mut producer = step("verifier", "implement", &[SCRIPT]);
    producer.verify = vec![format!("node {SCRIPT}")];
    let mut raw = plan(vec![producer]);
    raw.goal = "scaffold verifier".into();
    let mut host = raw.clone();
    crate::planner::runner::strengthen_step_plan_for_profile(&mut host, &c);
    assert!(
        host.steps[0]
            .expected_paths
            .contains(&"package.json".into())
    );
    let mut admission = admission::Admission::default();
    assert!(matches!(
        admission.check(&c, None, &raw, &mut host, 1).unwrap(),
        admission::Decision::Retry(_)
    ));
    let log = events(&c);
    let e = log
        .iter()
        .find(|e| e["event"] == "recovery_verifier_plan_admission")
        .unwrap();
    assert_eq!(
        e["model_proposal"]["steps"][0]["expected_paths"],
        json!([SCRIPT])
    );
    assert!(
        e["host_augmented_proposal"]["steps"][0]["expected_paths"]
            .as_array()
            .unwrap()
            .contains(&json!("package.json"))
    );
    let mut corrected = host.clone();
    corrected.steps[0].expected_paths = vec![SCRIPT.into()];
    corrected.steps[0].verify.clear();
    let mut configuration = step("configuration", "implement", &["package.json"]);
    configuration.instruction = host.steps[0].instruction.clone();
    corrected.steps.push(configuration);
    let mut confirmation = host.steps[0].clone();
    confirmation.id = "confirm-group".into();
    confirmation.kind = "verify".into();
    corrected.steps.push(confirmation);
    let model = corrected.clone();
    crate::planner::runner::strengthen_step_plan_for_profile(&mut corrected, &c);
    assert!(matches!(
        admission
            .check(&c, None, &model, &mut corrected, 2)
            .unwrap(),
        admission::Decision::Ready(false)
    ));
}

#[test]
fn issue466_compound_commands_share_registration_admission_and_nonexecuting_steps_cannot_register()
{
    for kind in ["implement", "verify", "inspect", "report", "setup"] {
        let root = tempfile::tempdir().unwrap();
        let mut c = config(root.path());
        let _guard = authority::begin_run(&c);
        let contract_path = generated_contract(&c);
        let mut empty: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&contract_path).unwrap()).unwrap();
        empty["verify_commands"] = json!([]);
        std::fs::write(&contract_path, serde_json::to_vec(&empty).unwrap()).unwrap();
        let producer = step("producer", "implement", &[SCRIPT]);
        let mut check = step("command-owner", kind, &[]);
        check.verify = vec!["test -f app.js && node smoke-check.js".into()];
        let mut source = plan(vec![
            step("application", "implement", &["app.js"]),
            producer,
            check,
        ]);
        if kind == "implement" {
            source.steps[1].verify = source.steps.pop().unwrap().verify;
        }
        if matches!(kind, "implement" | "verify") {
            // The actual Runner normalizes the same raw compound expression.
            let mut planner = Replay::new(vec![proposal(&source)]);
            let generated =
                crate::planner::runner::generate_step_plan(&mut planner, &source.goal, &c)
                    .unwrap_or_else(|e| panic!("{kind}: {e}; {:?}", events(&c)));
            scope::register(&c, &generated).unwrap();
            let producers = scope::generated(&c);
            assert_eq!(producers.len(), 1);
            assert_eq!(producers[0].expected_paths, [SCRIPT]);
            c.completion_contract_path = Some(contract_path);
            let contract = CompletionContract::load_for_config(&c).unwrap().unwrap();
            assert_eq!(contract.verify_commands, ["node smoke-check.js"]);
            assert!(contract.required_paths.contains(&"app.js".into()));
            bind_context(&c);
            let mut recovery = plan(vec![step("recovery-producer", "implement", &[SCRIPT])]);
            bind_generated(&c, Some("repair"), &mut recovery).unwrap();
            assert!(
                recovery
                    .steps
                    .last()
                    .unwrap()
                    .verify
                    .contains(&"node smoke-check.js".into())
            );
        } else {
            // Calling the registration boundary directly must not broaden the
            // prior Implement/Verify-only contract, even without a lint caller.
            scope::register(&c, &source).unwrap();
            assert!(scope::generated(&c).is_empty());
            c.completion_contract_path = Some(contract_path);
            assert!(
                CompletionContract::load_for_config(&c)
                    .unwrap()
                    .unwrap()
                    .verify_commands
                    .is_empty()
            );
        }
    }
}
