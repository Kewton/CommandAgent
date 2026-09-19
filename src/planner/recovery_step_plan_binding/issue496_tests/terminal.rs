use super::*;

#[derive(serde::Deserialize)]
struct LegacyAdmission {
    event: String,
    phase_id: String,
    stage: String,
    classification: String,
    reason: String,
    model_proposal: StepPlan,
    host_augmented_proposal: StepPlan,
    original_scope: StepPlan,
    original_obligation_sources: serde_json::Value,
    package_script_formation: serde_json::Value,
    planner_attempt: usize,
    remaining_planner_attempts: usize,
    recovery_budget_changed: bool,
    status: String,
}

#[cfg(test)]
#[test]
fn issue498_saved_proof_has_independent_arithmetic_and_compatible_event_fields() {
    let root = tempfile::tempdir().unwrap();
    let c = super::super::issue478::nextjs_config(root.path());
    let raw = saved();
    let model = &target(&raw).instruction;
    let host = include_str!(
        "../../../../tests/corpus/apps/issue496-profile-guidance-capacity/host-guidance.txt"
    );
    let m: Vec<_> = model.chars().collect();
    let h: Vec<_> = host.chars().collect();
    assert!(!model.contains(host) && !host.contains(model));
    // Independent exhaustive comparison, separate from the production KMP.
    for size in 1..=m.len().min(h.len()) {
        assert_ne!(m[m.len() - size..], h[..size]);
        assert_ne!(h[h.len() - size..], m[..size]);
    }
    assert_eq!(m.len() + h.len(), 2766);
    let mut a = admission::Admission::default();
    let (mut candidate, _) = through_policy(&c, &mut a, &raw);
    assert!(
        a.check(&c, Some("core-implementation"), &raw, &mut candidate, 1)
            .is_err()
    );
    let event = events(&c)
        .into_iter()
        .find(|e| e["event"] == "recovery_verifier_plan_admission")
        .unwrap();
    let old: LegacyAdmission = serde_json::from_value(event.clone()).unwrap();
    assert_eq!(old.event, "recovery_verifier_plan_admission");
    assert_eq!(old.phase_id, "core-implementation");
    assert_eq!(old.stage, "preclosure_formation");
    assert_eq!(old.classification, "proposal_repairable");
    assert!(old.reason.contains("bounded repair is not representable"));
    assert_eq!(old.model_proposal, raw);
    assert_eq!(old.host_augmented_proposal, candidate);
    assert_eq!(old.original_scope, candidate);
    assert_eq!(old.planner_attempt, 1);
    assert_eq!(old.remaining_planner_attempts, 2);
    assert!(!old.recovery_budget_changed);
    assert_eq!(old.status, "stopped");
    assert_eq!(
        old.original_obligation_sources,
        old.package_script_formation["sources"]["plans"]
    );
    let proof = &event["bounded_repair_proof"];
    for (source, text) in [("model", model.as_str()), ("host", host)] {
        assert_eq!(
            proof[format!("{source}_instruction_sha256")],
            format!("{:x}", Sha256::digest(text.as_bytes()))
        );
    }
    assert_eq!(proof["registered_split"], "package_owner_scope");
    assert!(
        proof["split_inapplicable_reason"]
            .as_str()
            .unwrap()
            .contains("multiple normalized paths")
    );
    // New readers must give explicit terminal control precedence over the
    // legacy classification. Old readers can still deserialize all fields.
    assert_eq!(event["terminal_reason"], "bounded_repair_infeasible");
    assert_eq!(event["retry_allowed"], false);
    assert_eq!(
        event["preservation_sources"]["host"]["steps"],
        old.original_obligation_sources["host"]["steps"]
    );
    assert!(event["marker_obligations"].is_array());
    assert!(event["marker_capture_error"].is_null());
}

#[cfg(test)]
#[test]
fn issue498_four_path_duties_cannot_escape_through_split_aliases_or_fragments() {
    let root = tempfile::tempdir().unwrap();
    let c = super::super::issue478::nextjs_config(root.path());
    let raw = saved();
    let mut original = StepPlan {
        goal: raw.goal.clone(),
        steps: vec![target(&raw).clone()],
    };
    let addition = profile_augmentation::strengthen_step_plan_for_profile(&mut original, &c);
    let sources = formation_scope::FormationScope::capture(&original, addition.as_ref());
    assert!(package_owner_scope::split_duties(&sources.model, &sources.host).is_none());
    for case in [
        "path_order",
        "alias",
        "duplicate_full",
        "only_model",
        "only_host",
        "fragment",
        "split_paths",
        "package_only",
    ] {
        let mut proposed = original.clone();
        let mut second = proposed.steps[0].clone();
        second.id = "second-owner".into();
        match case {
            "path_order" => proposed.steps[0].expected_paths.reverse(),
            "alias" => proposed.steps[0]
                .expected_paths
                .iter_mut()
                .for_each(|p| *p = format!("./{p}")),
            "duplicate_full" => proposed.steps.push(second),
            "only_model" | "only_host" => {
                proposed.steps[0].instruction = sources.model.steps[0].instruction.clone();
                second.instruction = sources.host.steps[0].instruction.clone();
                if case == "only_host" {
                    proposed.steps[0].instruction = second.instruction.clone();
                }
                proposed.steps.push(second);
            }
            "fragment" => {
                let tail = proposed.steps[0].instruction.split_off(1500);
                second.instruction = tail;
                proposed.steps.push(second);
            }
            "split_paths" => {
                second.expected_paths = proposed.steps[0].expected_paths.split_off(2);
                proposed.steps.push(second);
            }
            "package_only" => {
                proposed.steps[0].expected_paths = vec!["package.json".into()];
                proposed.steps[0].instruction = sources.model.steps[0].instruction.clone();
                second.expected_paths = vec!["package.json".into()];
                second.instruction = sources.host.steps[0].instruction.clone();
                proposed.steps.push(second);
            }
            _ => unreachable!(),
        }
        let model_check = admission::preserve(&sources.model, &proposed);
        let host_check = admission::preserve(&sources.host, &proposed);
        let fully_retained = model_check.is_ok() && host_check.is_ok();
        if matches!(case, "path_order" | "alias") {
            assert!(fully_retained, "{case}");
        }
        // Every attempted bypass either loses a duty/check or still violates
        // capacity. Full duplicate owners gain no bounded representation.
        assert!(
            !fully_retained
                || proposed
                    .steps
                    .iter()
                    .any(|s| s.instruction.chars().count() > 2500),
            "{case}"
        );
        assert!(
            package_owner_scope::views(&sources.model, &sources.host, &original, &proposed)
                .is_none()
        );
    }
}

#[cfg(test)]
#[test]
fn issue498_initial_bounded_multi_path_owner_is_ready() {
    let root = tempfile::tempdir().unwrap();
    let c = super::super::issue478::nextjs_config(root.path());
    let mut raw = bounded_plan(&c, 2499);
    raw.steps[0].expected_paths = vec!["a.txt".into(), "b.txt".into()];
    let mut a = admission::Admission::default();
    let (mut candidate, _) = through_policy(&c, &mut a, &raw);
    assert!(matches!(
        a.check(&c, None, &raw, &mut candidate, 1).unwrap(),
        admission::Decision::Ready(false)
    ));
    assert!(crate::planner::lint::lint_template_contract(&candidate, Some(root.path())).is_pass());
    assert!(a.finish(&c, None, candidate).is_ok());
}

#[cfg(test)]
#[test]
fn issue498_multi_path_overlap_is_unknown_then_existing_repair_reaches_ready() {
    let root = tempfile::tempdir().unwrap();
    let c = super::super::issue478::nextjs_config(root.path());
    let mut raw = bounded_plan(&c, 2501);
    raw.steps[0].expected_paths = vec!["a.txt".into(), "b.txt".into()];
    let guidance = crate::planner::profile::resolve_profile_runtime(&c.profile)
        .guidance(&raw.goal)
        .unwrap();
    raw.steps[0].instruction = guidance.chars().skip(1).collect();
    let mut a = admission::Admission::default();
    let (mut candidate, _) = through_policy(&c, &mut a, &raw);
    assert!(candidate.steps[0].instruction.chars().count() > 2500);
    assert!(matches!(
        a.check(&c, None, &raw, &mut candidate, 1).unwrap(),
        admission::Decision::Retry(_)
    ));
    raw.steps[0].instruction = guidance;
    let (mut candidate, _) = through_policy(&c, &mut a, &raw);
    assert!(matches!(
        a.check(&c, None, &raw, &mut candidate, 2).unwrap(),
        admission::Decision::Ready(false)
    ));
    assert!(crate::planner::lint::lint_template_contract(&candidate, Some(root.path())).is_pass());
    assert!(a.finish(&c, None, candidate).is_ok());
}
