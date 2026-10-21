use super::*;

fn inline_plan() -> StepPlan {
    let mut producer = step(
        "application",
        "implement",
        &["src/lib/types.ts", "src/lib/store.ts"],
    );
    producer.instruction = "Implement the shared domain interfaces and storage API.".into();
    let mut check = step("check-modules", "verify", &[]);
    check.instruction = "Check both domain modules and retain their import failures.".into();
    check.verify = vec![
        "node -e \"import('./src/lib/types.ts')\"".into(),
        "node -e \"import('./src/lib/store.ts')\"".into(),
    ];
    plan(vec![producer, check])
}

#[test]
fn issue479_pure_import_requires_preclosure_formation() {
    let root = tempfile::tempdir().unwrap();
    let c = config(root.path());
    let original = inline_plan();
    let mut proposed = original.clone();
    let decision = admission::Admission::default()
        .check(&c, None, &original, &mut proposed, 1)
        .unwrap();
    assert!(
        matches!(decision, admission::Decision::Retry(_)),
        "pure imports must not close unchanged"
    );
}

fn formed_commands() -> Vec<String> {
    serde_json::from_str(include_str!(
        "../../../tests/corpus/apps/issue479-verifier-formation/formed-commands.json"
    ))
    .unwrap()
}

fn refined_plan() -> StepPlan {
    let mut p = inline_plan();
    p.steps[1].verify = formed_commands()[1..3].to_vec();
    p
}

fn replay_inline(c: &Config, after: &StepPlan) -> (anyhow::Result<StepPlan>, Replay) {
    std::fs::write(
        c.workspace_root.join("package.json"),
        include_str!("../../../tests/corpus/apps/issue479-verifier-formation/package.json"),
    )
    .unwrap();
    std::fs::create_dir_all(c.workspace_root.join("node_modules")).unwrap();
    let before = inline_plan();
    let mut client = Replay::new(vec![proposal(&before), proposal(after), proposal(after)]);
    let result = crate::planner::runner::generate_step_plan(&mut client, &before.goal, c);
    (result, client)
}

#[test]
fn issue479_runner_forms_explicit_export_checks_and_registers_only_the_refinements() {
    let root = tempfile::tempdir().unwrap();
    let mut c = config(root.path());
    let _guard = authority::begin_run(&c);
    let path = generated_contract(&c);
    std::fs::write(
        &path,
        json!({"profile":"generic","required_paths":[],"verify_commands":[]}).to_string(),
    )
    .unwrap();
    let (result, client) = replay_inline(&c, &refined_plan());
    let formed = result.unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(client.requests.lock().unwrap().len(), 2);
    assert_eq!(formed, refined_plan());
    scope::register(&c, &formed).unwrap();
    c.completion_contract_path = Some(path.clone());
    let contract = CompletionContract::load_for_config(&c).unwrap().unwrap();
    assert_eq!(contract.verify_commands, formed_commands()[1..3]);
    assert!(
        !contract
            .verify_commands
            .iter()
            .any(|c| inline_plan().steps[1].verify.contains(c))
    );
    let record = events(&c)
        .into_iter()
        .find(|e| e["event"] == "preclosure_verifier_replacements_validated")
        .unwrap();
    assert_eq!(record["replacements"].as_array().unwrap().len(), 2);
    assert_eq!(
        record["replacements"][0]["import_target"],
        "./src/lib/types.ts"
    );
    assert_eq!(record["replacements"][0]["expected_result"], "pass");
    // Closure cannot be reopened for another proposal spelling or expectation.
    let bytes = std::fs::read(&path).unwrap();
    let mut proposed = inline_plan();
    assert!(matches!(
        admission::Admission::default()
            .check(&c, None, &inline_plan(), &mut proposed, 1)
            .unwrap(),
        admission::Decision::Ready(false)
    ));
    scope::register(&c, &proposed).unwrap();
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn issue479_runner_rejects_unproved_replacements_and_keeps_the_three_attempt_budget() {
    let cases: Vec<String> = serde_json::from_str(include_str!(
        "../../../tests/corpus/apps/issue479-verifier-formation/refusals.json"
    ))
    .unwrap();
    for case in cases
        .iter()
        .filter(|c| c.as_str() != "changed_expected_set")
    {
        let root = tempfile::tempdir().unwrap();
        let c = config(root.path());
        let mut after = refined_plan();
        match case.as_str() {
            "delete_check" => {
                after.steps[1].verify.remove(0);
            }
            "wrong_target" => {
                after.steps[1].verify[0] =
                    after.steps[1].verify[0].replace("types.ts", "unrelated.ts")
            }
            "changed_result" => after.steps[1].expected_result = "fail".into(),
            "unrelated_assertion" => {
                after.steps[1].verify[0] =
                    "node -e \"const assert=require('assert');assert.equal(1,1)\"".into()
            }
            "swallowed_import" => {
                after.steps[1].verify[0] =
                    after.steps[1].verify[0].replace(".then(actual", ".catch(()=>{}).then(actual")
            }
            "extra_statement" => {
                after.steps[1].verify[0] =
                    after.steps[1].verify[0].replace("})\"", "});process.exit(0)\"")
            }
            "append_beside_weak" => after.steps[1]
                .verify
                .push(inline_plan().steps[1].verify[0].clone()),
            "wrong_owner" => {
                let mut owner = after.steps[0].clone();
                owner.id = "notes".into();
                owner.expected_paths = vec!["notes.txt".into()];
                after.steps[0].instruction = "Drop the source contract".into();
                after.steps.push(owner);
            }
            "premature_check" => after.steps.swap(0, 1),
            "missing_output" => after.steps[0].expected_paths.clear(),
            "unregistered_replacement" => {
                after.steps[1].verify[0] = "node checks/model-equivalence.js".into()
            }
            _ => panic!("{case}"),
        }
        let (result, client) = replay_inline(&c, &after);
        assert!(result.is_err(), "{case}");
        assert_eq!(client.requests.lock().unwrap().len(), 3, "{case}");
    }
}

#[test]
fn issue479_validated_expectation_cannot_change_on_retry_or_fallback() {
    let root = tempfile::tempdir().unwrap();
    let c = config(root.path());
    let mut admission = admission::Admission::default();
    let original = inline_plan();
    let mut first = original.clone();
    assert!(matches!(
        admission.check(&c, None, &original, &mut first, 1).unwrap(),
        admission::Decision::Retry(_)
    ));
    // Only the first command is refined; the second still needs its proposal.
    let mut second = original.clone();
    second.steps[1].verify[0] = formed_commands()[1].clone();
    assert!(matches!(
        admission
            .check(&c, None, &second.clone(), &mut second, 2)
            .unwrap(),
        admission::Decision::Retry(_)
    ));
    let mut third = refined_plan();
    third.steps[1].verify[0] = third.steps[1].verify[0].replace("module.exports", "unexpected");
    let error = admission
        .check(&c, None, &third.clone(), &mut third, 3)
        .err()
        .unwrap();
    assert!(
        error
            .to_string()
            .contains("changed its explicit expectation"),
        "{error}"
    );
    assert!(admission.finish(&c, None, third).is_err());
    assert!(admission.finish(&c, None, original).is_err());
}

#[test]
fn issue479_final_inventory_preserves_all_five_checks_and_full_issue478_duties() {
    use super::issue478::{nextjs_config, saved};
    use crate::minimal_loop::evidence::command_diagnosis;
    let root = tempfile::tempdir().unwrap();
    let c = nextjs_config(root.path());
    let mut before = saved(2);
    let original: Vec<String> = serde_json::from_str(include_str!(
        "../../../tests/corpus/apps/issue479-verifier-formation/original-commands.json"
    ))
    .unwrap();
    before
        .steps
        .last_mut()
        .unwrap()
        .verify
        .extend_from_slice(&original[1..]);
    let mut after = saved(3);
    after
        .steps
        .last_mut()
        .unwrap()
        .verify
        .extend_from_slice(&formed_commands()[1..]);
    let mut client = Replay::new(vec![proposal(&before), proposal(&after), proposal(&after)]);
    let formed = crate::planner::runner::generate_step_plan_with_ui_for_phase(
        &mut client,
        &before.goal,
        &c,
        &crate::tui::NOOP_UI,
        Some("contract-wiring"),
        true,
        false,
    )
    .unwrap_or_else(|e| panic!("{e}; {:?}", events(&c)));
    assert_eq!(client.requests.lock().unwrap().len(), 2);
    assert_eq!(formed.steps[2], before.steps[2]);
    let package: Vec<String> = serde_json::from_str(include_str!(
        "../../../tests/corpus/apps/issue478-model-host-obligations/package-checks.json"
    ))
    .unwrap();
    assert_eq!(formed.steps[3].verify, package);
    assert_eq!(formed.steps[3].step_kind(), StepKind::Implement);
    assert!(
        formed.steps[3]
            .instruction
            .contains(&after.steps[3].instruction)
    );
    let retained = events(&c)
        .into_iter()
        .find(|e| e["event"] == "recovery_verifier_plan_admission")
        .unwrap();
    assert!(
        formed.steps[3].instruction.contains(
            retained["original_obligation_sources"]["host"]["steps"][0]["instruction"]
                .as_str()
                .unwrap()
        )
    );
    assert_eq!(formed.steps[4].verify, ["node smoke-check.js"]);
    assert_eq!(formed.steps[5].verify, formed_commands());
    let all = scope::commands(&formed);
    assert_eq!(all.len(), 9);
    let diagnoses = command_diagnosis::collect(root.path(), &all);
    // The uncreated smoke script remains contingent on its registered owner.
    assert_eq!(diagnoses.iter().filter(|d| d.kind == "weak").count(), 1);
    assert_eq!(
        diagnoses[3].repairability,
        "file_backed_verifier_requires_owner"
    );
    assert!(command_diagnosis::immutable_failure(&diagnoses).is_none());
    let contract: CompletionContract =
        serde_json::from_value(json!({"profile":c.profile,"goal":before.goal})).unwrap();
    let registered = scope::admitted_commands(&c, &contract, &formed).unwrap();
    assert_eq!(registered.len(), 6);
    assert!(package.iter().all(|p| !registered.contains(p)));
    assert!(formed_commands().iter().all(|c| registered.contains(c)));
    if let Ok(path) = std::env::var("ISSUE479_FORMATION_EVIDENCE") {
        std::fs::write(path,serde_json::to_vec_pretty(&json!({
            "source":"actual Runner, #478 profile augmentation/preset and admission, and final-command registration filter",
            "planner_requests":2,"model_before":before,"model_refined":after,
            "retained_sources":retained["original_obligation_sources"],"formed":formed,
            "all_step_command_diagnoses":diagnoses,"registered_commands":registered,
            "execution":"formation only; smoke creation and app build remain execution obligations"
        })).unwrap()).unwrap();
    }
}

#[test]
fn issue479_attached_weak_inline_cannot_close_with_app_edits() {
    for command in [
        "node --eval=\"console.log(1)\"",
        "node -e\"console.log(1)\"",
    ] {
        let root = tempfile::tempdir().unwrap();
        let c = config(root.path());
        let mut original = inline_plan();
        original.steps[1].verify = vec![command.into()];
        let contract: CompletionContract = serde_json::from_value(json!({})).unwrap();
        assert_eq!(
            scope::admitted_commands(&c, &contract, &original).unwrap(),
            [command]
        );
        let mut admission = admission::Admission::default();
        for attempt in 1..=3 {
            std::fs::write(
                root.path().join("app.js"),
                format!("export const value={attempt};"),
            )
            .unwrap();
            let result = admission.check(&c, None, &original, &mut original.clone(), attempt);
            if attempt < 3 {
                assert!(matches!(result.unwrap(), admission::Decision::Retry(_)));
            } else {
                assert!(result.err().unwrap().to_string().contains("3-attempt"));
            }
        }
        assert!(admission.finish(&c, None, original).is_err());
    }
}

#[test]
fn issue479_missing_registered_script_owner_can_create_and_confirm() {
    let (root, c, _guard) = registered(&[SCRIPT], &[]);
    let diagnostics = crate::minimal_loop::evidence::command_diagnosis::collect(
        root.path(),
        &[format!("node {SCRIPT}")],
    );
    assert_eq!(
        diagnostics[0].repairability,
        "file_backed_verifier_requires_owner"
    );
    let original = plan(vec![step("candidate-owner", "implement", &[SCRIPT])]);
    let mut planner = Replay::new(vec![proposal(&original)]);
    let formed = generate(&c, &mut planner).unwrap();
    let mut executor = Replay::new(vec![
        AssistantReply {
            content: String::new(),
            tool_calls: vec![ToolCall::new(
                "Write",
                json!({"path":SCRIPT,"content":std::fs::read_to_string(Path::new(FIXTURE).join(SCRIPT)).unwrap()}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        },
        AssistantReply::text("Created verifier"),
    ]);
    assert!(crate::planner::runner::run_step_plan(&mut executor, &formed, &c).is_ok());
    assert!(root.path().join(SCRIPT).is_file());
    assert_eq!(
        crate::minimal_loop::evidence::command_diagnosis::collect(
            root.path(),
            &[format!("node {SCRIPT}")]
        )[0]
        .kind,
        "test"
    );
}
