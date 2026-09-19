//! Tests inside Admission also exercise arbitrary host lengths without adding a
//! synthetic runtime profile or treating model-written labels as provenance.
use super::*;
use clap::Parser;

fn setup() -> (tempfile::TempDir, Config) {
    let root = tempfile::tempdir().unwrap();
    let mut c =
        Config::from_cli(crate::cli::Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
    c.workspace_root = root.path().to_path_buf();
    c.profile = "generic".into();
    c.eval_events_path = Some(root.path().join("events.jsonl"));
    (root, c)
}

#[cfg(test)]
#[test]
fn issue496_host_alone_over_capacity_is_never_truncated_in_any_augmentation_shape() {
    for shape in ["prefix", "contained", "append"] {
        let (_root, c) = setup();
        let host = "務".repeat(2_501);
        let mut raw = StepPlan {
            goal: "Complete duties".into(),
            steps: vec![PlanStep {
                id: "owner".into(),
                kind: "implement".into(),
                expected_result: "pass".into(),
                instruction: match shape {
                    "prefix" => "務".repeat(10),
                    "contained" => host.clone(),
                    _ => "Model duty".into(),
                },
                expected_paths: vec!["output.txt".into()],
                verify: vec![],
            }],
        };
        let model = raw.steps[0].clone();
        raw.steps[0].instruction = if shape == "append" {
            format!("{}\n\nProfile contract:\n{host}", model.instruction)
        } else {
            host.clone()
        };
        let addition = ProfileAddition::new(model, &raw.steps[0], host.clone());
        let mut admission = Admission {
            profile_addition: Some(addition),
            ..Default::default()
        };
        let report = admission.sanitize(&mut raw, Some(&c.workspace_root));
        assert!(report.instruction_truncations.is_empty());
        let original = raw.clone();
        let Decision::Retry(feedback) = admission.check(&c, None, &original, &mut raw, 1).unwrap()
        else {
            panic!("{shape} accepted")
        };
        assert!(feedback.contains("2500-character capacity"));
        assert!(raw.steps[0].instruction.contains(&host));
        assert!(admission.finish(&c, None, raw).is_err());
    }
}

#[cfg(test)]
#[test]
fn issue496_marker_capture_failure_is_frozen_instead_of_becoming_no_obligations() {
    let (_root, mut c) = setup();
    c.profile = "nextjs".into();
    let raw = StepPlan {
        goal: "Complete the application".into(),
        steps: vec![PlanStep {
            id: "owner".into(),
            kind: "implement".into(),
            expected_result: "pass".into(),
            instruction: "業".repeat(600),
            expected_paths: vec!["page.tsx".into()],
            verify: vec!["node -e \"unterminated".into()],
        }],
    };
    let mut candidate = raw.clone();
    let mut admission = Admission::default();
    admission.strengthen(&c, &mut candidate);
    admission.sanitize(&mut candidate, Some(&c.workspace_root));
    assert!(matches!(
        admission.check(&c, None, &raw, &mut candidate, 1).unwrap(),
        Decision::Retry(_)
    ));
    let error = admission
        .marker_capture_error
        .clone()
        .expect("invalid source must not look empty");
    candidate.steps[0].verify.clear();
    assert!(
        admission
            .preserve(&candidate)
            .unwrap_err()
            .to_string()
            .contains("source acquisition failed")
    );
    admission.retain_current_contract(&c, &candidate).unwrap();
    assert_eq!(
        admission.marker_capture_error.as_deref(),
        Some(error.as_str())
    );
    assert_eq!(
        admission.original.as_ref().unwrap().steps[0].verify,
        raw.steps[0].verify
    );
}

#[cfg(test)]
#[test]
fn issue496_capacity_freezes_marker_formation_before_workspace_changes() {
    let (root, mut c) = setup();
    c.profile = "nextjs".into();
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tests/corpus/apps/issue488-d4-inline/commands.json"
    ))
    .unwrap();
    let original_command = fixture["O"]["command"].as_str().unwrap();
    let contract = crate::planner::completion_contract_path::generated_path(
        &c.workspace_root,
        c.eval_events_path.as_deref(),
        "completion-contract-ultra-plan-run.json",
    );
    std::fs::write(
        &contract,
        include_str!("../../../../tests/corpus/apps/issue488-d4-inline/contract.json"),
    )
    .unwrap();
    let _guard = crate::planner::recovery_contract_authority::begin_run(&c);
    crate::planner::recovery_contract_authority::record_generated_contract(
        &c,
        "ultra-plan-run",
        &contract,
    );
    std::fs::create_dir_all(root.path().join("src/app")).unwrap();
    std::fs::create_dir_all(root.path().join("node_modules")).unwrap();
    std::fs::write(root.path().join("package.json"), "{}").unwrap();
    std::fs::write(root.path().join("src/app/page.tsx"), "// original source").unwrap();
    let mut raw = StepPlan {
        goal: "Complete app duties".into(),
        steps: vec![
            PlanStep {
                id: "app-owner".into(),
                kind: "implement".into(),
                expected_result: "pass".into(),
                instruction: "業".repeat(600),
                expected_paths: vec!["src/app/page.tsx".into()],
                verify: vec![],
            },
            PlanStep {
                id: "check-markers".into(),
                kind: "verify".into(),
                expected_result: "pass".into(),
                instruction: "Check the saved source markers.".into(),
                expected_paths: vec![],
                verify: vec![original_command.into()],
            },
        ],
    };
    let model = raw.clone();
    let mut admission = Admission::default();
    let registered = crate::planner::recovery_contract_authority::load_for_handoff(&c)
        .unwrap()
        .expect("generated contract");
    assert!(
        crate::planner::recovery_contract_authority::inline_admission::eligible(&c, &registered)
            .unwrap(),
        "intent={:?}, profile={:?}",
        c.resolved_run_intent(),
        registered.profile
    );
    admission.strengthen(&c, &mut raw);
    admission.sanitize(&mut raw, Some(root.path()));
    assert!(matches!(
        admission.check(&c, None, &model, &mut raw, 1).unwrap(),
        Decision::Retry(_)
    ));
    assert_eq!(raw.steps[1].verify.len(), 1);
    assert_eq!(admission.marker_checks.len(), 1);
    let frozen = admission.marker_checks.clone();
    assert!(super::super::literal_marker_formation::matches(
        &frozen[0],
        fixture["V2"]["command"].as_str().unwrap()
    ));
    std::fs::write(root.path().join("src/app/page.tsx"), "// changed source").unwrap();
    raw.steps[1].verify = vec!["test -f src/app/page.tsx".into()];
    admission.retain_current_contract(&c, &raw).unwrap();
    assert!(admission.marker_checks == frozen);
    assert!(admission.preserve(&raw).is_err());
}
