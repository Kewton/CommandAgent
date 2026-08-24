use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn commit(
    config: &mut Config,
    plan: &mut UltraPlan,
    ultra_context: &mut UltraRunContext,
    final_expected_paths: &mut Vec<String>,
    promotion_state: &mut ProfilePromotionState,
    phase: &UltraPhase,
    index: usize,
    setup_authority_state: &mut UltraRunSetupAuthorityState,
) -> anyhow::Result<()> {
    bounded_process::reap_registered_server_children_for_workspace(
        config.eval_events_path.as_deref(),
        "phase_transition",
        &config.workspace_root,
    );
    reconcile_manifest_changed_dependencies_if_needed(
        config,
        &plan.profile,
        setup_authority_state,
    )?;
    if try_promote_profile_at_phase_boundary(
        config,
        plan,
        ultra_context,
        final_expected_paths,
        promotion_state,
        phase,
        index,
    )?
    .is_some()
    {
        setup_authority_state.grant("profile_promotion");
        reconcile_run_dependency_setup(
            config,
            &plan.profile,
            DependencyReconciliationTrigger::Promotion,
            setup_authority_state,
        )?;
    }
    Ok(())
}
