use super::*;

fn frozen_json(prompt: &str, label: &str) -> serde_json::Value {
    serde_json::from_str(prompt.split_once(label).unwrap().1.lines().nth(1).unwrap()).unwrap()
}

fn admission_events(c: &Config) -> Vec<serde_json::Value> {
    events(c)
        .into_iter()
        .filter(|event| event["event"] == "recovery_verifier_plan_admission")
        .collect()
}

fn retry(
    c: &Config,
    admission: &mut admission::Admission,
    raw: &StepPlan,
    attempt: usize,
) -> String {
    let (mut candidate, _) = through_policy(c, admission, raw);
    let admission::Decision::Retry(feedback) = admission
        .check(c, None, raw, &mut candidate, attempt)
        .unwrap()
    else {
        panic!("expected a retry")
    };
    feedback
}

fn overlapping(c: &Config) -> (StepPlan, StepPlan) {
    let mut before = bounded_plan(c, 2_500);
    let guidance = crate::planner::profile::resolve_profile_runtime(&c.profile)
        .guidance(&before.goal)
        .unwrap();
    // Host completion can remove duplication without losing a single model word.
    before.steps[0].instruction = guidance.chars().skip(1).collect();
    let mut check = step("check-package", "verify", &[]);
    check.instruction = "Check the package manifest exists.".into();
    check.verify = vec!["test -f package.json".into()];
    before.steps.push(check);
    let mut after = before.clone();
    after.steps[0].instruction = guidance;
    (before, after)
}

#[cfg(test)]
#[test]
fn issue496_same_owner_can_remove_overlap_but_must_keep_all_duties() {
    let cases: Vec<String> = serde_json::from_str(include_str!(
        "../../../../tests/corpus/apps/issue496-profile-guidance-capacity/refusals.json"
    ))
    .unwrap();
    for case in std::iter::once("positive".to_string()).chain(cases) {
        let root = tempfile::tempdir().unwrap();
        let c = super::super::issue478::nextjs_config(root.path());
        let (before, after) = overlapping(&c);
        let mut admission = admission::Admission::default();
        let feedback = retry(&c, &mut admission, &before, 1);
        assert!(feedback.contains("2500-character capacity"));
        let (mut candidate, _) = through_policy(&c, &mut admission, &after);
        match case.as_str() {
            "positive" => {}
            "model_instruction" => {
                candidate.steps[0].instruction = "Keep the host summary only.".into()
            }
            "host_tail" => {
                candidate.steps[0].instruction.pop();
            }
            "expected_path" => candidate.steps[0].expected_paths.clear(),
            "expected_result" => candidate.steps[0].expected_result = "fail".into(),
            "check" => candidate.steps[1].verify.clear(),
            "owner_order" => candidate.steps.swap(0, 1),
            _ => panic!("unknown fixture {case}"),
        }
        let decision = admission
            .check(&c, None, &after, &mut candidate, 2)
            .unwrap();
        if case == "positive" {
            assert!(matches!(decision, admission::Decision::Ready(false)));
            assert!(
                crate::planner::lint::lint_template_contract(&candidate, Some(root.path()))
                    .is_pass()
            );
            assert!(admission.finish(&c, None, candidate).is_ok());
        } else {
            assert!(matches!(decision, admission::Decision::Retry(_)), "{case}");
            assert!(admission.finish(&c, None, candidate).is_err(), "{case}");
        }
    }
}

#[cfg(test)]
#[test]
fn issue496_saved_model_and_host_tail_loss_are_independently_rejected() {
    for model_loss in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let c = super::super::issue478::nextjs_config(root.path());
        let raw = saved();
        let mut admission = admission::Admission::default();
        let (mut candidate, _) = through_policy(&c, &mut admission, &raw);
        let original = candidate.clone();
        let sources = formation_scope::FormationScope::capture(
            &candidate,
            profile_augmentation::strengthen_step_plan_for_profile(&mut raw.clone(), &c).as_ref(),
        );
        let owner = candidate
            .steps
            .iter_mut()
            .find(|s| s.id == "implement-layout-and-config")
            .unwrap();
        if model_loss {
            owner.instruction = owner.instruction.replace(&target(&raw).instruction, "");
        } else {
            let new_len = owner.instruction.len() - 344;
            owner.instruction.truncate(new_len);
        }
        assert!(owner.instruction.chars().count() <= 2_500);
        let error = sources.preserve(&original, &candidate).unwrap_err();
        assert!(
            error.to_string().contains("lost original requirements"),
            "{error}"
        );
    }
}

#[cfg(test)]
#[test]
fn issue496_capacity_freezes_sources_contract_and_expectations_once() {
    let root = tempfile::tempdir().unwrap();
    let mut c = super::super::issue478::nextjs_config(root.path());
    let contract_path = root.path().join("contract.json");
    let contract = json!({"profile":"nextjs", "verify_commands":["test -f package.json"]});
    std::fs::write(&contract_path, contract.to_string()).unwrap();
    c.completion_contract_path = Some(contract_path.clone());
    let mut raw: StepPlan = serde_json::from_str(include_str!(
        "../../../../tests/corpus/apps/issue484-package-script-formation/original-plan.json"
    ))
    .unwrap();
    let mut admission = admission::Admission::default();
    let first_feedback = retry(&c, &mut admission, &raw, 1);
    let first = admission_events(&c).pop().unwrap();
    let formation = &first["package_script_formation"];
    assert_eq!(
        formation["sources"]["acquisition_stage"],
        "profile_augmentation_before_sanitization"
    );
    assert_eq!(
        first["original_obligation_sources"],
        formation["sources"]["plans"]
    );
    assert_eq!(
        formation["obligations"][0]["expected_literal"],
        "next dev -p 60302"
    );
    assert_eq!(
        formation["registered_contract"]["verify_commands"],
        contract["verify_commands"]
    );
    assert_eq!(
        frozen_json(
            &first_feedback,
            "Frozen formation evidence and registered contract:"
        ),
        *formation
    );
    std::fs::write(
        root.path().join("package.json"),
        r#"{"scripts":{"dev":"echo changed"}}"#,
    )
    .unwrap();
    std::fs::write(
        &contract_path,
        r#"{"verify_commands":["test -f changed.txt"]}"#,
    )
    .unwrap();
    raw.steps[4]
        .instruction
        .push_str(" Later proposal must not set the baseline.");
    let second_feedback = retry(&c, &mut admission, &raw, 2);
    let second = admission_events(&c).pop().unwrap();
    for key in [
        "original_scope",
        "original_obligation_sources",
        "package_script_formation",
    ] {
        assert_eq!(first[key], second[key], "{key} changed");
    }
    assert_eq!(
        frozen_json(
            &second_feedback,
            "Frozen formation evidence and registered contract:"
        ),
        *formation
    );
}

#[cfg(test)]
#[test]
fn issue496_mixed_failures_retain_retry_input_and_block_setup_fallback() {
    for invalid in [
        "{invalid",
        "",
        r#"{"goal":"x","steps":[{"id":"x","kind":"unknown","instruction":"x","expected_result":"pass","expected_paths":[],"verify":[]}]}"#,
    ] {
        let root = tempfile::tempdir().unwrap();
        let c = super::super::issue478::nextjs_config(root.path());
        let (raw, _) = overlapping(&c);
        let mut client = Replay::new(vec![
            proposal(&raw),
            AssistantReply::text(invalid),
            AssistantReply::text(invalid),
        ]);
        // Existing setup fallback is selected only after proposal retries.
        let result = crate::planner::runner::generate_step_plan(
            &mut client,
            "Phase id: setup\nPhase task: scaffold the project",
            &c,
        );
        assert!(result.is_err(), "{invalid:?} escaped via fallback");
        let log = events(&c);
        if invalid == "{invalid" || invalid.is_empty() {
            assert!(
                log.iter().any(|e| e["event"] == "planner_fallback_plan"),
                "fallback was not reached: {invalid:?}"
            );
            assert!(
                log.iter()
                    .any(|e| e["event"] == "recovery_verifier_plan_return_rejected"),
                "finish was not reached: {invalid:?}"
            );
            let stage = if invalid.is_empty() {
                "empty_response"
            } else {
                "schema"
            };
            assert_eq!(
                log.iter().filter(|e| e["planner_stage"] == stage).count(),
                2,
                "{invalid:?}"
            );
        } else {
            assert_eq!(
                admission_events(&c).len(),
                3,
                "missing obligations must reach admission"
            );
        }
        let requests = client.requests.lock().unwrap();
        assert_eq!(requests.len(), 3);
        let source = admission_events(&c)[0]["original_obligation_sources"].clone();
        for request in requests.iter().skip(1) {
            let text = serde_json::to_string(request).unwrap();
            assert!(text.contains("Frozen admission baseline"), "{invalid:?}");
            // Compare decoded message content, without assuming serialization escaping.
            let message = request
                .iter()
                .find(|m| m.content.contains("Frozen obligation sources:"))
                .unwrap();
            assert_eq!(
                frozen_json(&message.content, "Frozen obligation sources:"),
                source
            );
        }
        assert!(
            events(&c)
                .iter()
                .all(|e| e["event"] != "tool_call_started" && e["event"] != "step_started")
        );
    }
}

#[cfg(test)]
#[test]
fn issue496_prior_valid_plan_cannot_bypass_later_capacity_failure() {
    let root = tempfile::tempdir().unwrap();
    let c = super::super::issue478::nextjs_config(root.path());
    let raw = bounded_plan(&c, 2_499);
    let mut admission = admission::Admission::default();
    let (mut valid, _) = through_policy(&c, &mut admission, &raw);
    assert!(matches!(
        admission.check(&c, None, &raw, &mut valid, 1).unwrap(),
        admission::Decision::Ready(false)
    ));
    let mut larger = raw.clone();
    larger.steps[0]
        .instruction
        .push_str(" Preserve the additional obligation.");
    retry(&c, &mut admission, &larger, 2);
    assert!(admission.finish(&c, None, valid).is_err());
    assert!(
        events(&c)
            .iter()
            .any(|e| e["event"] == "recovery_verifier_plan_return_rejected")
    );
}

#[cfg(test)]
#[test]
fn issue496_formation_before_capacity_keeps_the_first_baseline() {
    let root = tempfile::tempdir().unwrap();
    let mut c = config(root.path());
    let mut raw = plan(vec![step("owner", "implement", &["module.cjs"])]);
    raw.steps[0].verify = vec!["node -p \"require('./module.cjs')\"".into()];
    let mut admission = admission::Admission::default();
    retry(&c, &mut admission, &raw, 1);
    let first = admission_events(&c).pop().unwrap();
    c.profile = NEXTJS_PROFILE_ID.into();
    raw.steps[0].instruction = "業".repeat(700);
    retry(&c, &mut admission, &raw, 2);
    let second = admission_events(&c).pop().unwrap();
    assert!(
        second["reason"]
            .as_str()
            .unwrap()
            .contains("2500-character capacity")
    );
    for key in [
        "original_scope",
        "original_obligation_sources",
        "package_script_formation",
    ] {
        assert_eq!(first[key], second[key]);
    }
}

#[cfg(test)]
#[test]
fn issue496_recovery_and_configured_contract_cannot_bypass_capacity() {
    for recovery in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let mut c = super::super::issue478::nextjs_config(root.path());
        let path = root.path().join("contract.json");
        let bytes = r#"{"profile":"nextjs","verify_commands":["test -f package.json"],"required_paths":[]}"#;
        std::fs::write(&path, bytes).unwrap();
        c.completion_contract_path = Some(path.clone());
        if recovery {
            bind_context(&c);
        }
        let raw = bounded_plan(&c, 2_501);
        let mut admission = admission::Admission::default();
        assert!(retry(&c, &mut admission, &raw, 1).contains("2500-character capacity"));
        assert_eq!(std::fs::read_to_string(path).unwrap(), bytes);
        let event = admission_events(&c).pop().unwrap();
        assert_eq!(
            event["stage"],
            if recovery {
                "closed_binding"
            } else {
                "preclosure_formation"
            }
        );
    }
}

#[cfg(test)]
#[test]
fn issue496_deterministic_capacity_retry_is_an_explicit_error() {
    let root = tempfile::tempdir().unwrap();
    let c = super::super::issue478::nextjs_config(root.path());
    let long_path = format!("{}.txt", "a".repeat(600));
    let goal = format!(
        "Phase id: build-verification\nPhase task: Verify the Next.js build\nRequired final artifacts:\n- {long_path}"
    );
    let mut client = Replay::default();
    let error = crate::planner::runner::generate_step_plan_with_ui_for_phase(
        &mut client,
        &goal,
        &c,
        &crate::tui::NOOP_UI,
        Some("build-verification"),
        true,
        true,
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("deterministic template requires explicit verifier scope"),
        "{error:#}"
    );
    assert!(
        error.to_string().contains("2500-character capacity"),
        "{error:#}"
    );
    assert!(client.requests.lock().unwrap().is_empty());
    let admissions = admission_events(&c);
    assert_eq!(admissions.len(), 1);
    assert_eq!(admissions[0]["planner_attempt"], 1);
}

#[cfg(test)]
#[test]
fn issue496_capacity_then_actual_lint_then_empty_keeps_frozen_retry_context() {
    let root = tempfile::tempdir().unwrap();
    let c = super::super::issue478::nextjs_config(root.path());
    let (before, mut after) = overlapping(&c);
    after.steps.push(step("invalid-kind", "unknown", &[]));
    let mut client = Replay::new(vec![
        proposal(&before),
        proposal(&after),
        AssistantReply::text(""),
    ]);
    let result = crate::planner::runner::generate_step_plan_with_ui_for_phase(
        &mut client,
        &before.goal,
        &c,
        &crate::tui::NOOP_UI,
        Some("core-implementation"),
        false,
        false,
    );
    assert!(result.is_err());
    let log = events(&c);
    assert_eq!(
        admission_events(&c).len(),
        1,
        "second proposal must reach lint"
    );
    assert!(log.iter().any(|e| e["planner_stage"] == "lint"), "{log:?}");
    let requests = client.requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    let third = requests[2]
        .iter()
        .find(|m| m.content.contains("Frozen obligation sources:"))
        .unwrap();
    assert_eq!(
        frozen_json(&third.content, "Frozen obligation sources:"),
        admission_events(&c)[0]["original_obligation_sources"]
    );
}
