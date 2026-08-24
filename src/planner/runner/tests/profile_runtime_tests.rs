use super::*;

#[test]
fn promoted_contract_union_never_drops_generic_interactive_requirements() {
    let goal = "ちょっとしたメモアプリを作って";
    let generic_id = ProfileId::Generic;
    let nextjs_id = ProfileId::Nextjs;
    let generic_runtime = ProfileRuntimeRegistry::resolve(&generic_id);
    let nextjs_runtime = ProfileRuntimeRegistry::resolve(&nextjs_id);
    let generic = runtime_contract_requirements(generic_runtime, &generic_id, goal);
    let mut promoted = runtime_contract_requirements(nextjs_runtime, &nextjs_id, goal);

    assert!(promoted.capabilities.is_empty());
    carry_pre_promotion_contract_requirements_with_runtime(
        nextjs_runtime,
        &nextjs_id,
        goal,
        &generic,
        &mut promoted,
    );
    merge_unique_strings(
        &mut promoted.evidence,
        &nextjs_runtime.required_evidence(goal, &promoted.capabilities),
    );

    for capability in [
        "stateful_interaction",
        "user_input_or_action",
        "visible_state_change",
    ] {
        assert!(
            promoted.capabilities.contains(&capability.to_string()),
            "{capability} missing from {promoted:?}"
        );
    }
    for evidence in GENERIC_INTERACTIVE_EVIDENCE_KEYS {
        assert!(
            promoted.evidence.contains(&evidence.to_string()),
            "{evidence} missing from {promoted:?}"
        );
    }
    assert!(
        promoted
            .evidence
            .contains(&"nextjs_route_evidence".to_string())
    );
    assert!(
        promoted
            .evidence
            .contains(&"build_command_or_dependency_missing_boundary".to_string())
    );
    assert!(
        generic
            .obligations
            .iter()
            .all(|obligation| promoted.obligations.contains(obligation))
    );
}
