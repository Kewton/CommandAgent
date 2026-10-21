use super::*;

fn projected_split(c: &Config) -> (StepPlan, FormationScope, StepPlan) {
    let original: StepPlan = serde_json::from_str(&fixture("saved-scope.json")).unwrap();
    let sources: serde_json::Value =
        serde_json::from_str(&fixture("saved-obligation-sources.json")).unwrap();
    let sources = FormationScope {
        model: serde_json::from_value(sources["model"].clone()).unwrap(),
        host: serde_json::from_value(sources["host"].clone()).unwrap(),
    };
    let mut candidate = saved(true);
    crate::planner::step_plan::repair_generated_step_plan_contract(&mut candidate);
    // Run the real preset transformation, then project the already-proven
    // comparison to the original command for scope/boundary checks.
    let runtime = crate::planner::profile::resolve_profile_runtime(
        crate::planner::profile_descriptor::NEXTJS_PROFILE_ID,
    );
    runtime.convert_preset_phase_setup_steps(
        &mut candidate,
        &c.workspace_root,
        &saved(false).goal,
        Some(("core-implementation", false)),
        true,
        None,
    );
    candidate.steps[6].verify[0] = saved(false).steps[4].verify[0].clone();
    (original, sources, candidate)
}

#[test]
fn issue484_split_rejects_every_source_writer_and_boundary_loss() {
    let root = tempfile::tempdir().unwrap();
    let c = super::super::issue478::nextjs_config(root.path());
    let (original, sources, candidate) = projected_split(&c);
    sources.preserve(&original, &candidate).unwrap();
    let cases: Vec<String> = serde_json::from_str(&fixture("owner-refusals.json")).unwrap();
    for case in cases {
        let mut p = candidate.clone();
        let mut s = sources.clone();
        match case.as_str() {
            "model_text" => p.steps[4].instruction.push_str(" Changed duty."),
            "host_text" => p.steps[5].instruction.pop().map(|_| ()).unwrap(),
            "model_result" => p.steps[4].expected_result = "fail".into(),
            "host_result" => p.steps[5].expected_result = "fail".into(),
            "model_scope" => p.steps[4].expected_paths = vec!["other.json".into()],
            "host_scope" => p.steps[5].expected_paths = vec!["other.json".into()],
            "delete_model" => {
                p.steps.remove(4);
            }
            "delete_host" => {
                p.steps.remove(5);
            }
            "third_writer" | "alias_third_writer" | "non_normalized_writer" => {
                let mut third = p.steps[5].clone();
                third.id = "third-writer".into();
                third.expected_paths = vec![
                    match case.as_str() {
                        "alias_third_writer" => "./package.json",
                        "non_normalized_writer" => "config/../package.json",
                        _ => "package.json",
                    }
                    .into(),
                ];
                p.steps.insert(6, third);
            }
            "same_id" => p.steps[5].id = p.steps[4].id.clone(),
            "host_before_model" => p.steps.swap(4, 5),
            "check_between_writers" => p.steps.swap(5, 6),
            "check_before_writers" => p.steps.swap(4, 6),
            "partial_checks" => {
                p.steps[6].verify.remove(1);
            }
            "host_source_verify" => s.host.steps[0].verify = vec!["test -f package.json".into()],
            "host_source_result" => s.host.steps[0].expected_result = "fail".into(),
            _ => unreachable!(),
        }
        assert!(s.preserve(&original, &p).is_err(), "{case}");
    }
}

#[test]
fn issue484_lint_requires_complete_ordered_split_and_runtime_keeps_owners_executable() {
    let root = tempfile::tempdir().unwrap();
    let c = super::super::issue478::nextjs_config(root.path());
    let (_, _, mut candidate) = projected_split(&c);
    candidate.steps[6].verify[0] = check();
    let runtime = crate::planner::profile::resolve_profile_runtime(
        crate::planner::profile_descriptor::NEXTJS_PROFILE_ID,
    );
    for owner in &candidate.steps[4..6] {
        assert_eq!(owner.step_kind(), StepKind::Implement);
        assert!(!runtime.step_short_circuit_precheck_applicable(owner));
        let (executed, synthesized) = runtime.runtime_step_with_profile_checks(
            root.path(),
            &candidate.goal,
            owner,
            Some("core-implementation"),
            None,
        );
        assert!(!synthesized);
        assert_eq!(executed, *owner);
    }
    let permits = |p: &StepPlan| {
        super::super::super::package_owner_scope::ordered_split(
            p,
            "package.json",
            &p.steps[4].id,
            &p.steps[5].id,
        )
    };
    assert!(permits(&candidate));
    for case in [
        "wrong_literal",
        "no_final_check",
        "early_final_check",
        "third_writer",
        "unknown_host",
        "missing_host_check",
    ] {
        let mut p = candidate.clone();
        match case {
            "wrong_literal" => p.steps[6].verify[0] = check().replace("60302", "3000"),
            "no_final_check" => p.steps[6].verify.clear(),
            "early_final_check" => p.steps.swap(5, 6),
            "third_writer" => {
                let mut extra = p.steps[5].clone();
                extra.id = "third".into();
                p.steps.push(extra);
            }
            "unknown_host" => p.steps[5].instruction = "Maintain package.json.".into(),
            "missing_host_check" => {
                p.steps[6].verify.pop();
            }
            _ => unreachable!(),
        }
        assert!(!permits(&p), "{case}");
        assert!(
            !crate::planner::lint::lint_plan_for_execution(&p, Some(root.path())).is_pass(),
            "{case}"
        );
    }
}
