use super::*;

fn with_inline() -> (tempfile::TempDir, Config) {
    with_inline_command("node -e \"import('./src/types.js')\"")
}

fn with_inline_command(command: &str) -> (tempfile::TempDir, Config) {
    setup_with(|c| {
        let path = c.completion_contract_path.as_ref().unwrap();
        let mut contract: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        contract["required_evidence"] = json!(["implementation_artifact"]);
        contract["verify_commands"]
            .as_array_mut()
            .unwrap()
            .push(json!(command));
        std::fs::write(path, serde_json::to_vec(&contract).unwrap()).unwrap();
    })
}

#[test]
fn issue479_attached_frozen_inline_refuses_app_only_repair() {
    for command in [
        "node --eval=\"console.log(1)\"",
        "node -e\"console.log(1)\"",
    ] {
        let (_root, c) = with_inline_command(command);
        let raw = plan();
        let error = crate::planner::recovery_step_plan_binding::admission::Admission::default()
            .check(&c, Some("repair-api"), &raw, &mut raw.clone(), 1)
            .err()
            .unwrap();
        assert!(
            error.to_string().contains("not repairable by app edits"),
            "{error}"
        );
        let mut group = plan();
        group.steps.push(step("remaining-store", STORE));
        let options = options(&c, &group, 0);
        let api = c.workspace_root.join(API);
        let source = std::fs::read_to_string(&api).unwrap();
        std::fs::write(
            api,
            source.replace("validateShift(input)", "validateShift(input, policy)"),
        )
        .unwrap();
        let obligation = options.recovery_obligation.as_ref().unwrap();
        assert!(obligation.feedback(&c, &options).unwrap().contains(command));
        assert!(!obligation.pending());
    }
}

#[test]
fn issue479_frozen_inline_stops_admission_without_app_worker_or_contract_mutation() {
    let (root, c) = with_inline();
    let before = std::fs::read(c.completion_contract_path.as_ref().unwrap()).unwrap();
    let raw = plan();
    let mut candidate = raw.clone();
    let error = crate::planner::recovery_step_plan_binding::admission::Admission::default()
        .check(&c, Some("repair-api"), &raw, &mut candidate, 1)
        .err()
        .unwrap();
    assert!(
        error.to_string().contains("not repairable by app edits"),
        "{error}"
    );
    assert!(error.to_string().contains("import('./src/types.js')"));
    assert_eq!(candidate, raw);
    let log = events(&c);
    let admission = log
        .iter()
        .find(|e| e["event"] == "recovery_verifier_plan_admission")
        .unwrap();
    assert_eq!(admission["classification"], "original_inconsistent");
    assert_eq!(admission["status"], "stopped");
    assert_eq!(admission["planner_attempt"], 1);
    assert_eq!(
        std::fs::read(c.completion_contract_path.as_ref().unwrap()).unwrap(),
        before
    );
    assert!(root.path().join(API).is_file());
}

#[test]
fn issue479_inline_diagnosis_blocks_pending_and_unrelated_edits_but_keeps_preservation() {
    let (_root, c) = with_inline();
    let mut group = plan();
    group.steps.push(step("remaining-store", STORE));
    let options = options(&c, &group, 0);
    let obligation = options.recovery_obligation.as_ref().unwrap();
    let path = c.workspace_root.join(API);
    let source = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        source.replace("validateShift(input)", "validateShift(input, policy)"),
    )
    .unwrap();
    let feedback = obligation.feedback(&c, &options).unwrap();
    assert!(feedback.contains("immutable_inline_evidence"), "{feedback}");
    assert!(!obligation.pending());
    let log = events(&c);
    let observation = log
        .iter()
        .find(|e| e["event"] == "recovery_repair_obligation_observed")
        .unwrap();
    assert_eq!(observation["status"], "unresolved");
    assert_eq!(
        observation["command_diagnoses"][1]["repairability"],
        "immutable_inline_evidence"
    );
    // Source processing loss remains a preservation failure even with an
    // independently irreparable inline-evidence defect in the same contract.
    std::fs::write(path, "export function submit(input){return {ok:true};}").unwrap();
    let feedback = obligation.feedback(&c, &options).unwrap();
    assert!(feedback.contains("processing removed"), "{feedback}");
}

#[test]
fn issue479_frozen_file_backed_weak_verifier_is_identified_without_granting_write_authority() {
    let (_root, c) = setup_with(|c| {
        std::fs::write(
            c.workspace_root.join("checks/target.js"),
            "console.log('weak');",
        )
        .unwrap();
        let path = c.completion_contract_path.as_ref().unwrap();
        let mut contract: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        contract["required_evidence"] = json!(["implementation_artifact"]);
        std::fs::write(path, serde_json::to_vec(&contract).unwrap()).unwrap();
    });
    let mut group = plan();
    group.steps.push(step("remaining-store", STORE));
    let options = options(&c, &group, 0);
    let api = c.workspace_root.join(API);
    let source = std::fs::read_to_string(&api).unwrap();
    std::fs::write(
        api,
        source.replace("validateShift(input)", "validateShift(input, policy)"),
    )
    .unwrap();
    let feedback = options
        .recovery_obligation
        .as_ref()
        .unwrap()
        .feedback(&c, &options)
        .unwrap();
    assert!(
        feedback.contains("node_smoke_without_assertion"),
        "{feedback}"
    );
    assert!(!options.recovery_obligation.as_ref().unwrap().pending());
    let log = events(&c);
    let observation = log
        .iter()
        .find(|e| e["event"] == "recovery_repair_obligation_observed")
        .unwrap();
    assert_eq!(
        observation["command_diagnoses"][0]["repairability"],
        "frozen_verifier_evidence"
    );
    assert_eq!(observation["status"], "unresolved");
    std::fs::write(
        c.workspace_root.join("checks/target.js"),
        "import assert from 'node:assert';assert(true);",
    )
    .unwrap();
    let feedback = options
        .recovery_obligation
        .as_ref()
        .unwrap()
        .feedback(&c, &options)
        .unwrap();
    assert!(
        feedback.contains("confirmation input changed"),
        "{feedback}"
    );
}
