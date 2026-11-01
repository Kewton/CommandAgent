//! The one place the fixed identifiers live (Issue #548).
//!
//! A *fixed identifier* is a value the product itself owns: every `event` name,
//! every `commandagent.*` schema name, the status/verdict/result machine values,
//! and the built-in tool/protocol names. They must never be rewritten by the
//! secret scrub, and a credential value that equals one of them (or is a
//! substring of one) is refused at registration because redacting it would
//! corrupt the schema.
//!
//! There is no second copy: [`fixed_schema_keys`] and [`is_fixed_identifier`]
//! read these same constants, and the completeness test in `tests.rs` scans the
//! source literals and fails when a new event or schema name is missing here.

/// Keys whose value is a fixed identifier inside a schema container. Such a
/// value is preserved only when it is itself a fixed identifier
/// ([`is_fixed_identifier`]); any other value is scrubbed.
pub const FIXED_SCHEMA_KEYS: &[&str] = &[
    "event",
    "schema_version",
    "status",
    "verdict",
    "type",
    "kind",
    "classification",
    "level",
    "ok",
    "phase",
    "stage",
    "family",
    "envelope_version",
    "lifecycle_stage",
    "tool_name",
    "step_kind",
    "judgement",
    "direction",
    "action",
];

/// Keys whose direct children are themselves a fixed schema (their schema keys
/// are fixed at depth two). `evidence_envelope` is the real product case.
pub const FIXED_SCHEMA_CONTAINERS: &[&str] = &["evidence_envelope"];

/// Every fixed identifier: all event names, every `commandagent.*` schema name,
/// the status/verdict/result machine values, and the built-in tool/protocol
/// names.
pub const FIXED_IDENTIFIERS: &[&str] = &[
    "Bash",
    "COMMANDAGENT_REQUIREMENT_PLAN",
    "Edit",
    "Glob",
    "Grep",
    "Read",
    "Write",
    "aborted",
    "accounted_by",
    "accounting_record",
    "adapter",
    "adapter_diagnostics",
    "adopted_unreplayed",
    "anchor_stagnation_interlock",
    "artifact-check-diagnostics",
    "artifact_contract_check_result",
    "artifact_stagnation_feedback",
    "bash_failures_after_last_write",
    "bash_path_confinement_rejected",
    "bash_results_after_last_write",
    "bash_results_after_prompt",
    "bash_sandbox_status",
    "bash_sandbox_unavailable",
    "blocked",
    "blocked_by_dependency_setup",
    "browser_interaction_probe",
    "browser_probe",
    "budget_exhausted",
    "build_pass",
    "call_index",
    "captured",
    "check",
    "closed",
    "code",
    "command_timeout_repetition",
    "commandagent.eval.score/v0",
    "commandagent.extensions/v1",
    "commandagent.fetch-cache/v0",
    "commandagent.fetch-evidence/v0",
    "commandagent.fetch-recording/v0",
    "commandagent.fetch/v0",
    "commandagent.gui-contract/v1",
    "commandagent.gui-working-directory/v1",
    "commandagent.headless-summary/v1",
    "commandagent.pack.assist/v0",
    "commandagent.pack.eval/v0",
    "commandagent.pass/v1",
    "commandagent.run-trace/v1",
    "commandagent.runs/v1",
    "commandagent.score-checkpoint/v0",
    "community_profile_verification",
    "compile_error_extraction",
    "compile_no_snapshot_narrow_retry",
    "compile_rollback_applied",
    "compile_rollback_context_carried",
    "compile_rollback_failed",
    "compile_rollback_skipped",
    "compile_snapshot_saved",
    "complete",
    "completed",
    "completion_contract",
    "completion_contract_bound",
    "completion_contract_command_observation",
    "completion_obligation_review",
    "completion_verify",
    "context_truncation_suspected",
    "contract_attribute_repair_guidance",
    "contract_observation_incomplete",
    "declarative_command_check_result",
    "degraded",
    "dependency_build_lifecycle",
    "dependency_denial_guidance",
    "dependency_setup_reconciliation",
    "depth_profile",
    "deterministic_step_plan_used",
    "dev_server_lifecycle",
    "dev_server_port_in_use_retry",
    "disposition",
    "draft",
    "draft_profile",
    "edit_anchor_recovery",
    "edit_anchor_salvaged",
    "edits_after_prompt",
    "empty_response_escalation",
    "empty_response_recovered",
    "error",
    "escalation_carryover",
    "example_count",
    "expected_fail_python_traceback_observed",
    "extension_tool_call",
    "extension_tool_rejected",
    "extension_tool_result",
    "extracted",
    "fail",
    "failed",
    "fallback",
    "fallback_decision",
    "final_acceptance_cycle_complete",
    "final_acceptance_cycle_summary",
    "final_acceptance_deterministic_remedies",
    "final_acceptance_repair_complete",
    "final_acceptance_repair_exhausted",
    "final_acceptance_repair_failed",
    "final_acceptance_repair_no_source_change",
    "final_acceptance_repair_start",
    "first_request",
    "fix_evidence_recorded",
    "fix_plan_synthesized",
    "fix_regression_contract_bound",
    "fix_reproducer_suggested",
    "fixture",
    "foreign_toolchain_detected",
    "full",
    "function",
    "future_phase_annotation",
    "generic_contract_bound",
    "groups",
    "gui_trial_stop_completed",
    "gui_trial_stop_requested",
    "h05_collision",
    "hidden_path_feedback",
    "hook_snapshot_feedback",
    "hook_snapshot_restored",
    "hook_snapshot_saved",
    "host_env_contamination",
    "host_env_normalized",
    "human_directive_confirmed",
    "human_directive_continuation_started",
    "human_directive_continuation_stopped",
    "human_directive_proposed",
    "incomplete",
    "ingest_candidate_ids_injected",
    "ingest_plan_synthesized",
    "ingest_snapshot_structure_injected",
    "inspect_command_normalized",
    "inspect_mutation_tool_rejected",
    "insufficient_remaining_iterations",
    "intent_resolved",
    "interrupted",
    "invalid",
    "investigation_adjudicated",
    "investigation_plan_synthesized",
    "items",
    "iteration",
    "iteration_budget_exhausted",
    "iteration_limit",
    "iterations_used",
    "last_bash_after_last_write_succeeded",
    "last_successful_write_iteration",
    "legacy_detail",
    "local_pack",
    "loop_stop",
    "message",
    "min_remaining_iterations",
    "missing",
    "native_tool_parse_retry_feedback",
    "next_action_selected",
    "next_final",
    "no_progress_feedback",
    "no_provider_request",
    "no_successful_edit",
    "normal_final_after_successful_edit",
    "normal_marker",
    "not_provided",
    "not_run",
    "note_chars",
    "obligation_count",
    "observed",
    "ok",
    "omitted",
    "openrouter_provider_observed",
    "other_path",
    "oversize",
    "owner",
    "pack_check_result",
    "pack_injected",
    "panic_caught",
    "partial",
    "pass",
    "passed",
    "path_fallback_evaluated",
    "pending",
    "phase_verification_result",
    "pipeline_dropped",
    "pipeline_error_extraction",
    "plan_final_contract",
    "plan_only_turn",
    "plan_preset_resolved",
    "plan_run_step",
    "plan_step_started",
    "planner_error",
    "planner_fallback_plan",
    "planner_plan_sanitized",
    "planner_quality_issue",
    "planner_quality_retry",
    "planner_quality_retry_degraded",
    "planner_quality_retry_exhausted",
    "planner_quality_warning",
    "planner_raw_output_shape",
    "planner_verify_command_normalized",
    "preclosure_verifier_replacements_validated",
    "preset_step_converted",
    "preset_ultra_plan_used",
    "probe_preflight",
    "profile_behavior_probe",
    "profile_inferred",
    "profile_invariant_repair",
    "profile_invariant_repair_build_verify",
    "profile_not_generic",
    "profile_reinferred",
    "prompt_version",
    "protected_path_mutation_rejected",
    "provider_billed_cost_usd",
    "provider_error",
    "provider_parse_error",
    "provider_request",
    "provider_response",
    "provider_retry",
    "provider_turn_aborted_by_user",
    "provider_turn_duration",
    "provider_turn_timeout",
    "provider_turn_timeout_recovered",
    "provider_unsupported",
    "python_cli_implementation_plan_canonicalized",
    "read_only_stagnation_feedback",
    "read_only_tool_rejected",
    "ready",
    "recovery_boundary_snapshot",
    "recovery_candidate_completion_requirements_bound",
    "recovery_candidate_verify_commands_bound",
    "recovery_capability_observer_bound",
    "recovery_completion_contract_bound",
    "recovery_continuation_prepared",
    "recovery_control_retained",
    "recovery_fix_contract_resumed",
    "recovery_fix_safety_verification",
    "recovery_generated_completion_contract_completed",
    "recovery_handoff_fidelity_bound",
    "recovery_handoff_fidelity_failed",
    "recovery_host_final_success_verification_failed",
    "recovery_host_final_success_verification_passed",
    "recovery_observation_effect_policy_bound",
    "recovery_plan_auto_run_complete",
    "recovery_plan_auto_run_start",
    "recovery_preflight_control_audit",
    "recovery_preflight_effect_observation",
    "recovery_preflight_observation",
    "recovery_product_mutation_observed",
    "recovery_promotion_decision",
    "recovery_prompt_save_failed",
    "recovery_prompt_saved",
    "recovery_read_only_post_step_repair_suppressed",
    "recovery_repair_obligation_observed",
    "recovery_step_plan_verify_commands_bound",
    "recovery_treatment_delta",
    "recovery_treatment_promoted",
    "recovery_ultra_plan_save_failed",
    "recovery_verifier_obligations_formed",
    "recovery_verifier_owner_binding",
    "recovery_verifier_plan_admission",
    "recovery_verifier_plan_return_rejected",
    "recovery_verifier_repairability",
    "released",
    "remaining_iterations",
    "repair_applied",
    "repair_regeneration",
    "repair_start_write_guidance_queued",
    "repair_target_reclassified",
    "repair_unreachable",
    "replay_turn_unrecorded",
    "replayed",
    "reported",
    "request",
    "request_id",
    "request_pattern_resolved",
    "requested",
    "requirement_plan",
    "resume_start",
    "resumed_conversation",
    "route_confirmed",
    "route_proposed",
    "route_unbound_recovery",
    "row_status",
    "run_completed",
    "run_failed",
    "run_interrupted",
    "run_start",
    "run_stop",
    "running",
    "runtime_bash_policy",
    "scope_exit_before_next_final",
    "score_checkpoint",
    "server_reaped",
    "setup_scaffold_completed",
    "side_effect_path_dropped",
    "skipped",
    "source",
    "speculative_phase_plan_provider_call",
    "state_binding_diagnosis",
    "step_capability_evidence_check",
    "step_completion_blocked",
    "step_obligation_scope",
    "step_oracle_tier_degraded",
    "step_prompt_contract",
    "step_short_circuited",
    "step_verify_failure",
    "step_verify_repair",
    "success",
    "successful_bash_after_prompt",
    "successful_bash_since_latest_edit",
    "successful_write_count",
    "target",
    "test_node_event",
    "time_profile",
    "tool",
    "tool_args_path_malformed",
    "tool_args_path_near_root_corruption",
    "tool_args_path_normalized",
    "tool_args_path_salvaged",
    "tool_args_recovered",
    "tool_call",
    "tool_call_raw",
    "tool_execute",
    "tool_parse_failure",
    "tool_policy_error",
    "tool_read_unchanged",
    "tool_validation_error",
    "triggered",
    "triggered_iteration",
    "truncated",
    "tui_command_start",
    "tui_command_stop",
    "ultra_context_initialized",
    "ultra_final_acceptance",
    "ultra_final_acceptance_failed",
    "ultra_partial_artifact_summary",
    "ultra_phase_complete",
    "ultra_phase_context_attached",
    "ultra_phase_context_updated",
    "ultra_phase_failed",
    "ultra_phase_start",
    "ultra_plan_complete",
    "ultra_plan_failed",
    "ultra_plan_generation_attempt",
    "ultra_plan_generation_failed",
    "ultra_plan_generation_metadata_normalized",
    "ultra_plan_generation_retry",
    "ultra_plan_generation_succeeded",
    "ultra_plan_generation_tool_call_rejected",
    "ultra_plan_raw_output_shape",
    "unavailable",
    "unchanged",
    "unconsumed",
    "unparseable",
    "unresolved",
    "unsettled",
    "unsuccessful_bash_after_prompt",
    "user_update",
    "valid_block",
    "verify_canonicalized",
    "verify_command_normalized_at_runtime",
    "verify_command_observed",
    "verify_command_substituted",
    "verify_command_timeout",
    "verify_command_timeout_substitution",
    "verify_default_bound",
    "verify_demoted_advisory",
    "verify_install_substituted",
    "verify_refusal_feedback_queued",
    "verify_repair_progress",
    "verify_repair_turn",
    "workflow_adjudicated",
    "workflow_edge_fired",
    "workflow_node_run_created",
    "workflow_node_started",
    "workflow_reproducer_bound",
    "workflow_reproducer_prevalidated",
    "workflow_started",
    "workspace_cd_stripped",
    "write_failure_after_last_write",
    "write_required_off_target_write_allowed",
];

/// Whether `key` is a schema key whose value is a fixed identifier.
pub fn is_fixed_schema_key(key: &str) -> bool {
    FIXED_SCHEMA_KEYS.contains(&key)
}

/// Whether `key` names a container whose direct children are a fixed schema.
pub fn is_fixed_container(key: &str) -> bool {
    FIXED_SCHEMA_CONTAINERS.contains(&key)
}

/// Whether `value` is exactly one of the fixed identifiers.
pub fn is_fixed_identifier(value: &str) -> bool {
    FIXED_IDENTIFIERS.contains(&value)
}

/// Whether a credential/plain value collides with the fixed identifiers.
///
/// True when the value equals a fixed identifier, is a substring of one (a
/// fragment of a fixed identifier would corrupt the schema when redacted), or
/// starts with the reserved `commandagent.` schema prefix. The reverse — a fixed
/// value appearing only inside a longer value — is not a collision.
pub fn collides_with_fixed_identifier(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    if value.starts_with("commandagent.") {
        return true;
    }
    FIXED_IDENTIFIERS
        .iter()
        .any(|fixed| *fixed == value || fixed.contains(value))
}

/// Issue #947 (review B2): the event field `provider_billed_cost_usd` is a
/// fixed identifier, so a credential equal to it or to a fragment of it is
/// refused at registration instead of turning the field name into a marker.
#[cfg(test)]
mod billed_cost_field_tests {
    use serde_json::json;

    use super::{FIXED_IDENTIFIERS, collides_with_fixed_identifier, is_fixed_identifier};
    use crate::sensitive_data::{RegistrationRefusal, ReservedKind, SecretCatalog};

    const FIELD: &str = "provider_billed_cost_usd";

    #[test]
    fn the_field_is_reserved_once_in_name_order() {
        assert!(is_fixed_identifier(FIELD));
        assert_eq!(
            FIXED_IDENTIFIERS
                .iter()
                .filter(|fixed| **fixed == FIELD)
                .count(),
            1
        );
        let at = FIXED_IDENTIFIERS
            .iter()
            .position(|fixed| *fixed == FIELD)
            .unwrap();
        assert!(
            FIXED_IDENTIFIERS[at - 1] < FIELD,
            "{}",
            FIXED_IDENTIFIERS[at - 1]
        );
        assert!(
            FIELD < FIXED_IDENTIFIERS[at + 1],
            "{}",
            FIXED_IDENTIFIERS[at + 1]
        );
    }

    #[test]
    fn a_credential_equal_to_the_field_or_a_fragment_of_it_is_refused_unnamed() {
        for value in [
            FIELD,
            "provider_billed_cost",
            "billed_cost_usd",
            "_billed_cost_",
            "der_billed_c",
        ] {
            assert!(
                collides_with_fixed_identifier(value),
                "fragment {}",
                value.len()
            );
            let mut catalog = SecretCatalog::new();
            let error = catalog
                .register_credential("COMPAT_API_KEY", value)
                .unwrap_err();
            assert_eq!(
                error,
                RegistrationRefusal::Reserved {
                    name: "COMPAT_API_KEY".to_string(),
                    kind: ReservedKind::FixedIdentifier,
                }
            );
            assert!(catalog.is_empty());
            for text in [error.to_string(), format!("{error:?}")] {
                assert!(!text.contains(value), "the refusal names the value");
            }
        }
    }

    #[test]
    fn a_normal_credential_leaves_the_field_name_and_amount_unchanged() {
        let credential = "fake-credential-947-not-a-real-key";
        let mut catalog = SecretCatalog::new();
        catalog
            .register_credential("COMPAT_API_KEY", credential)
            .unwrap();
        for event in ["provider_response", "provider_turn_duration"] {
            let mut value = json!({
                "event": event,
                "provider": "openai-compatible",
                FIELD: "0.012345",
                "body_snippet": format!("echo {credential}"),
            });
            catalog.scrub_value_lenient(&mut value);
            assert_eq!(value[FIELD], "0.012345", "{value}");
            assert_eq!(value["event"], event, "{value}");
            assert!(!value.to_string().contains(credential), "{value}");
            let object = value.as_object().unwrap();
            assert!(object.contains_key(FIELD), "{value}");
            assert_eq!(object.len(), 4, "{value}");
        }
    }
}

/// Issue #955: every fixed key and fixed value of the accepted-diagnostics
/// projection (the receipt field `adapter_diagnostics` and the repair block,
/// delimited by `artifact-check-diagnostics`) is a fixed identifier, so a
/// credential equal to one or to a fragment of one is refused and the event
/// scrub never rewrites the schema. The dynamic values under those keys stay
/// subject to the scrub.
#[cfg(test)]
mod artifact_diagnostics_field_tests {
    use serde_json::{Value, json};

    use super::{FIXED_IDENTIFIERS, collides_with_fixed_identifier, is_fixed_identifier};
    use crate::planner::artifact_contract_checks::diagnostics::status_word;
    use crate::planner::artifact_contract_checks::protocol::RowStatus;
    use crate::sensitive_data::{MIN_GENERIC_SECRET_LEN, SecretCatalog};
    use crate::verification::report::artifact_diagnostics::{
        ArtifactDiagnostic, ArtifactDiagnosticGroup, ArtifactDiagnostics,
    };

    /// Every name the projection writes itself.
    const NAMES: [&str; 20] = [
        "adapter_diagnostics",
        "artifact-check-diagnostics",
        "groups",
        "items",
        "omitted",
        "source",
        "owner",
        "adapter",
        "request",
        "request_id",
        "truncated",
        "check",
        "target",
        "row_status",
        "code",
        "message",
        "unresolved",
        "not_provided",
        "draft_profile",
        "local_pack",
    ];

    /// A projection with every optional part present.
    fn full() -> Value {
        let group = ArtifactDiagnosticGroup {
            source: "draft_profile".to_string(),
            owner: "o".to_string(),
            adapter: "a".to_string(),
            request: "r".to_string(),
            request_id: Some("i".to_string()),
            truncated: vec!["source", "owner", "adapter", "request", "request_id"],
            items: vec![ArtifactDiagnostic {
                check: "c".to_string(),
                target: Some("t".to_string()),
                row_status: "failed",
                code: "k".to_string(),
                message: "m".to_string(),
            }],
            omitted: 1,
        };
        serde_json::to_value(ArtifactDiagnostics {
            groups: vec![group],
            omitted: 1,
        })
        .unwrap()
    }

    fn keys(value: &Value, out: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                for (key, child) in map {
                    out.push(key.clone());
                    keys(child, out);
                }
            }
            Value::Array(items) => items.iter().for_each(|child| keys(child, out)),
            _ => {}
        }
    }

    #[test]
    fn every_fixed_key_and_value_of_the_projection_is_reserved() {
        let mut names = Vec::new();
        keys(&full(), &mut names);
        names.extend(
            full()["groups"][0]["truncated"]
                .as_array()
                .unwrap()
                .iter()
                .map(|name| name.as_str().unwrap().to_string()),
        );
        for status in [
            RowStatus::Failed,
            RowStatus::Unresolved,
            RowStatus::NotRun,
            RowStatus::NotProvided,
        ] {
            names.push(status_word(status).to_string());
        }
        names.extend(["draft_profile", "local_pack"].map(str::to_string));
        for name in &names {
            assert!(is_fixed_identifier(name), "{name} is not reserved");
        }
        let mut written = Vec::new();
        keys(&full(), &mut written);
        for key in written {
            assert!(NAMES.contains(&key.as_str()), "{key} is missing from NAMES");
        }
    }

    #[test]
    fn the_names_are_reserved_once_in_name_order() {
        for name in NAMES {
            assert!(is_fixed_identifier(name), "{name}");
            let at = FIXED_IDENTIFIERS
                .iter()
                .position(|fixed| *fixed == name)
                .unwrap();
            assert_eq!(
                FIXED_IDENTIFIERS
                    .iter()
                    .filter(|fixed| **fixed == name)
                    .count(),
                1,
                "{name}"
            );
            assert!(FIXED_IDENTIFIERS[at - 1] < name, "{name}");
            assert!(name < FIXED_IDENTIFIERS[at + 1], "{name}");
        }
    }

    #[test]
    fn every_registrable_fragment_of_a_name_is_refused() {
        for name in NAMES {
            if name.len() < MIN_GENERIC_SECRET_LEN {
                // Too short for any credential to be a fragment of it.
                continue;
            }
            for len in MIN_GENERIC_SECRET_LEN..=name.len() {
                for start in 0..=name.len() - len {
                    let value = &name[start..start + len];
                    assert!(collides_with_fixed_identifier(value), "{value}");
                    let mut catalog = SecretCatalog::new();
                    assert!(
                        catalog
                            .register_credential("COMPAT_API_KEY", value)
                            .is_err(),
                        "{value}"
                    );
                    assert!(catalog.is_empty(), "{value}");
                }
            }
        }
        for value in ["row_stat", "ow_statu", "request_", "unresolv", "not_prov"] {
            let mut catalog = SecretCatalog::new();
            assert!(
                catalog
                    .register_credential("COMPAT_API_KEY", value)
                    .is_err(),
                "{value}"
            );
        }
    }

    #[test]
    fn values_under_the_field_are_still_scrubbed() {
        let credential = "fake-credential-955-not-a-real-key";
        let mut catalog = SecretCatalog::new();
        catalog
            .register_credential("COMPAT_API_KEY", credential)
            .unwrap();
        let mut value = json!({
            "event": "artifact_contract_check_result",
            "adapter_diagnostics": {
                "items": [{"check": "rows/alpha", "row_status": "failed", "code": credential, "message": format!("x {credential}")}],
                "omitted": 0,
            },
        });
        catalog.scrub_value_lenient(&mut value);
        assert!(value.get("adapter_diagnostics").is_some(), "{value}");
        assert!(!value.to_string().contains(credential), "{value}");
        let item = &value["adapter_diagnostics"]["items"][0];
        assert_eq!(item["check"], "rows/alpha");
        assert_eq!(item["row_status"], "failed");
    }
}

/// Issue #959: the nine budget-exhaustion `loop_stop` field names are fixed
/// identifiers, so a credential equal to one of them or to a fragment of one
/// is refused at registration instead of turning a field name into a marker.
#[cfg(test)]
mod budget_exhaustion_field_tests {
    use serde_json::json;

    use super::{FIXED_IDENTIFIERS, collides_with_fixed_identifier, is_fixed_identifier};
    use crate::sensitive_data::{RegistrationRefusal, ReservedKind, SecretCatalog};

    const FIELDS: [&str; 9] = [
        "iteration_limit",
        "iterations_used",
        "iteration_budget_exhausted",
        "successful_write_count",
        "last_successful_write_iteration",
        "bash_results_after_last_write",
        "bash_failures_after_last_write",
        "last_bash_after_last_write_succeeded",
        "write_failure_after_last_write",
    ];

    #[test]
    fn each_field_is_reserved_once_in_name_order() {
        for field in FIELDS {
            assert!(is_fixed_identifier(field), "{field}");
            assert_eq!(
                FIXED_IDENTIFIERS
                    .iter()
                    .filter(|fixed| **fixed == field)
                    .count(),
                1,
                "{field}"
            );
            let at = FIXED_IDENTIFIERS
                .iter()
                .position(|fixed| *fixed == field)
                .unwrap();
            assert!(FIXED_IDENTIFIERS[at - 1] < field, "{field}");
            assert!(field < FIXED_IDENTIFIERS[at + 1], "{field}");
        }
    }

    #[test]
    fn a_credential_equal_to_a_field_or_a_fragment_of_it_is_refused_unnamed() {
        let fragments = [
            "budget_exhausted",
            "ations_use",
            "_successful_write_",
            "results_after_last_wr",
            "failures_after_last",
            "after_last_write_succeeded",
            "write_failure_after",
        ];
        for value in FIELDS.into_iter().chain(fragments) {
            assert!(collides_with_fixed_identifier(value), "{value}");
            let mut catalog = SecretCatalog::new();
            let error = catalog
                .register_credential("COMPAT_API_KEY", value)
                .unwrap_err();
            assert_eq!(
                error,
                RegistrationRefusal::Reserved {
                    name: "COMPAT_API_KEY".to_string(),
                    kind: ReservedKind::FixedIdentifier,
                }
            );
            assert!(catalog.is_empty());
            for text in [error.to_string(), format!("{error:?}")] {
                assert!(!text.contains(value), "the refusal names the value");
            }
        }
    }

    #[test]
    fn a_normal_credential_leaves_the_field_names_and_values_unchanged() {
        let credential = "fake-credential-959-not-a-real-key";
        let mut catalog = SecretCatalog::new();
        catalog
            .register_credential("COMPAT_API_KEY", credential)
            .unwrap();
        let mut value = json!({
            "event": "loop_stop",
            "reason": "model_stagnation:no_progress_recorded",
            "last_blocking_reason": format!("echo {credential}"),
            "iteration_limit": 15,
            "iterations_used": 15,
            "iteration_budget_exhausted": true,
            "successful_write_count": 2,
            "last_successful_write_iteration": 9,
            "bash_results_after_last_write": 3,
            "bash_failures_after_last_write": 1,
            "last_bash_after_last_write_succeeded": null,
            "write_failure_after_last_write": false,
        });
        catalog.scrub_value_lenient(&mut value);
        assert!(!value.to_string().contains(credential), "{value}");
        let object = value.as_object().unwrap();
        assert_eq!(object.len(), 12, "{value}");
        for field in FIELDS {
            assert!(object.contains_key(field), "{field}: {value}");
        }
        assert_eq!(value["iteration_limit"], 15);
        assert_eq!(value["iteration_budget_exhausted"], true);
        assert_eq!(value["last_bash_after_last_write_succeeded"], json!(null));
        assert_eq!(value["write_failure_after_last_write"], false);
        assert_eq!(value["reason"], "model_stagnation:no_progress_recorded");
    }
}
