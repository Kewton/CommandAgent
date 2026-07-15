pub(super) fn ids(phase_id: &str, final_phase: bool) -> Vec<&'static str> {
    crate::planner::profile_manifest::check_phase_scope::check_ids_for_phase(
        super::manifest::get(),
        phase_id,
        final_phase,
    )
}

pub(super) fn allows(allowed: Option<&[&str]>, id: &str) -> bool {
    allowed.is_none_or(|ids| ids.contains(&id))
}

pub(super) fn canonicalize_verify_commands(
    step: &mut crate::planner::step_plan::PlanStep,
    allowed: Option<&[&str]>,
    eval_events_path: Option<&std::path::Path>,
) -> usize {
    let original_commands = std::mem::take(&mut step.verify);
    let mut canonical = Vec::with_capacity(original_commands.len());
    let mut changes = 0;
    for original in original_commands {
        if let Some(id) = super::catalog_check_id(&original) {
            if allows(allowed, id) {
                super::push_unique(&mut canonical, original);
            } else {
                super::emit_canonicalized(
                    eval_events_path,
                    &step.id,
                    "verify",
                    &original,
                    "phase_out_of_scope",
                    "advisory",
                );
                changes += 1;
            }
            continue;
        }
        if !super::invented_verify_command(&original) {
            super::push_unique(&mut canonical, original);
            continue;
        }
        let replacements = super::inferred_catalog_checks(step, &original)
            .into_iter()
            .filter(|id| allows(allowed, id))
            .collect::<Vec<_>>();
        if replacements.is_empty() {
            super::emit_canonicalized(
                eval_events_path,
                &step.id,
                "verify",
                &original,
                "advisory",
                "advisory",
            );
        } else {
            for id in replacements {
                let replacement = super::catalog_check_command(id);
                super::push_unique(&mut canonical, replacement.clone());
                super::emit_canonicalized(
                    eval_events_path,
                    &step.id,
                    "verify",
                    &original,
                    &replacement,
                    "canonical",
                );
            }
        }
        changes += 1;
    }
    step.verify = canonical;
    changes
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUN3_FINAL_PLAN: &str = include_str!(
        "../../../../../tests/corpus/apps/test0715_data11_final_scope/fixtures/data5_qwen35_none_001/final-step-plan.yaml"
    );

    #[test]
    fn dynamic_final_uses_default_final_checks_without_inspection() {
        let final_ids = ids("final-verification-and-cleanup", true);
        assert!(final_ids.contains(&"data_claims_binding"));
        assert!(final_ids.contains(&"data_rerun_consistency"));
        assert!(!final_ids.contains(&"data_inspection_schema"));

        assert_eq!(ids("data-inspection", false), ["data_inspection_schema"]);
    }

    #[test]
    fn measured_dynamic_final_plan_rebinds_the_emptied_step_to_final_checks() {
        let mut plan = crate::planner::step_plan::parse_step_plan(RUN3_FINAL_PLAN).unwrap();
        let changes = super::super::canonicalize_step_plan(
            &mut plan,
            Some(("final-verification-and-cleanup", true)),
            None,
        );

        assert_eq!(changes, 2);
        let rebound = &plan.steps[3].verify;
        assert!(
            rebound
                .iter()
                .all(|command| !command.ends_with(":data_inspection_schema"))
        );
        assert!(
            rebound
                .iter()
                .any(|command| command.ends_with(":data_rerun_consistency"))
        );
        assert!(rebound.contains(&"test -f output/inspection.json".to_string()));
    }
}
