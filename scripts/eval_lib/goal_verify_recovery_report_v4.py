from __future__ import annotations

import statistics
from typing import Any

from eval_lib.goal_verify_recovery_experiment_v4 import (
    RECOVERY_FIX_TERMINAL_OUTCOME_POLICY,
)


def build_recovery_report(
    *,
    records: list[dict[str, Any]],
    contract: dict[str, Any],
    oracle_executability_preflight: dict[str, Any] | None = None,
) -> dict[str, Any]:
    expected = int(contract["smoke"]["expected_pair_count"])
    configured_zero = []
    configured_one = []
    maximum_one_executed = []
    ineligible_recovery_violations = []
    snapshot_mismatches = []
    oracle_source_violations = []
    resource_missing = []
    manifest_policy_violations = []
    execution_action_violations = []
    executed_recovery_pairs = []
    transitions = []
    shared_history = []
    boundary_matches = []
    semantic_validation = []
    pre_recovery_handoffs = []
    changed_paths_recorded = []
    initial_success_attribution_violations = []
    matrix_recorded = []
    attribution_ready = []
    browser_oracle_unavailable = []
    current_success_suppressions = []
    transaction_control_violations = []
    handoff_fidelity_violations = []
    treatment_isolation_violations = []
    typed_reproducer_violations = []
    recovery_verify_command_source_violations = []
    inner_recovery_verify_command_violations = []
    fix_contract_continuity_violations = []
    recovery_fix_terminal_completion_violations = []
    recovery_handoff_fidelity_v2_violations = []
    recovery_product_mutation_observation_violations = []
    recovery_fix_safety_verification_violations = []
    recovery_treatment_delta_violations = []
    recovery_bounded_local_repair_violations = []
    discarded_valid_treatments = []
    discarded_valid_treatment_count = 0
    observed_pair_ids = [record.get("pair_id") for record in records]
    typed_reproducer_commands = contract.get("smoke", {}).get(
        "typed_fix_reproducer_commands", {}
    )
    deltas: dict[str, list[int]] = {
        "wall_time_ms": [],
        "input_tokens": [],
        "output_tokens": [],
        "total_tokens": [],
    }
    for record in records:
        pair_id = record.get("pair_id")
        initial = record.get("initial_only", {})
        initial_result = initial.get("result", {})
        initial_attempts = initial_result.get("recovery_plan_attempts", {})
        configured_zero.append(initial_attempts.get("configured_recovery_runs") == 0)
        _check_oracle_source(initial, pair_id, oracle_source_violations)
        if (
            contract.get("smoke", {}).get("require_separate_browser_oracle_preflight")
            is not True
        ):
            _check_browser_oracle_executability(
                initial, pair_id, browser_oracle_unavailable
            )
        _check_resources(initial_result, pair_id, "initial_only", resource_missing)
        _check_manifest_policy(initial, pair_id, manifest_policy_violations)
        _check_execution_action(
            initial_result,
            contract=contract,
            pair_id=pair_id,
            arm="initial_only",
            errors=execution_action_violations,
        )
        recovery = record.get("recovery_one", {})
        runtime_eligible = (
            record.get("eligibility", {}).get("runtime", {}).get("run_recovery_one_arm")
        )
        if recovery.get("status") != "completed":
            if runtime_eligible is True:
                ineligible_recovery_violations.append(
                    f"eligible_recovery_not_completed:{pair_id}"
                )
            continue
        recovery_result = recovery.get("result", {})
        comparison_for_terminal = record.get("comparison")
        if not isinstance(comparison_for_terminal, dict):
            comparison_for_terminal = {}
        expected_reproducer = (
            typed_reproducer_commands.get(pair_id)
            if isinstance(typed_reproducer_commands, dict)
            else None
        )
        if isinstance(expected_reproducer, str) and not _typed_reproducer_matches(
            recovery_result.get("fix_reproducer_binding"), expected_reproducer
        ):
            typed_reproducer_violations.append(str(pair_id))
        recovery_attempts = recovery_result.get("recovery_plan_attempts", {})
        discarded_count = recovery_attempts.get("discarded_valid_treatment_count")
        if not isinstance(discarded_count, int):
            discarded_count = int(
                recovery_attempts.get("discarded_valid_treatment") is True
            )
        if discarded_count > 0:
            discarded_valid_treatments.append(str(pair_id))
            discarded_valid_treatment_count += discarded_count
        executed_recovery_runs = recovery_attempts.get("executed_recovery_runs")
        if (
            recovery_attempts.get("terminal_stop_reason") == "current_success_protected"
            or recovery_attempts.get("current_success_suppressed") is True
        ):
            current_success_suppressions.append(pair_id)
        if recovery_attempts.get("control_restore_failed_count", 0) != 0:
            transaction_control_violations.append(f"restore_failed:{pair_id}")
        rejected_count = recovery_attempts.get("treatment_regression_rejected_count", 0)
        retained_count = recovery_attempts.get("control_retained_count", 0)
        if (
            isinstance(rejected_count, int)
            and isinstance(retained_count, int)
            and rejected_count > retained_count
        ):
            transaction_control_violations.append(
                f"rejected_without_control_retention:{pair_id}"
            )
        shared_record = record.get("pairing_unit") == "shared_pre_recovery_snapshot"
        preregistered_eligible = (
            record.get("eligibility", {}).get("preregistered", {}).get("eligible")
        )
        configured_eligibility = (
            preregistered_eligible
            if isinstance(preregistered_eligible, bool)
            else runtime_eligible
        )
        configured_runs = recovery_attempts.get("configured_recovery_runs")
        configured_valid = (
            configured_runs == (1 if configured_eligibility is True else 0)
            if shared_record
            else configured_runs == 1
            and isinstance(executed_recovery_runs, int)
            and not isinstance(executed_recovery_runs, bool)
            and 0 <= executed_recovery_runs <= 1
        )
        configured_one.append(configured_valid)
        maximum_one_executed.append(
            isinstance(executed_recovery_runs, int)
            and not isinstance(executed_recovery_runs, bool)
            and 0 <= executed_recovery_runs <= 1
        )
        if runtime_eligible is not True and (
            not shared_record or executed_recovery_runs != 0
        ):
            ineligible_recovery_violations.append(
                f"ineligible_recovery_executed:{pair_id}"
            )
        if executed_recovery_runs == 1:
            executed_recovery_pairs.append(pair_id)
            recovery_attempt = next(
                (
                    row
                    for row in recovery_attempts.get("attempts", [])
                    if row.get("attempt_index") == 1
                ),
                {},
            )
            if recovery_attempt.get("recovery_candidate_scope") not in {
                "step",
                "phase",
            } or not isinstance(
                recovery_attempt.get("recovery_verify_command_count"), int
            ):
                handoff_fidelity_violations.append(str(pair_id))
            if (
                contract.get("smoke", {}).get(
                    "require_registered_recovery_verify_commands"
                )
                is True
                and recovery_attempt.get("recovery_verify_command_source")
                != "completion_contract"
            ):
                recovery_verify_command_source_violations.append(str(pair_id))
            if contract.get("smoke", {}).get(
                "require_registered_inner_recovery_verify_commands"
            ) is True and not _valid_inner_recovery_bindings(
                recovery_attempts.get("step_plan_contract_bindings"),
                require_pre_lint=(
                    contract.get("analysis", {}).get(
                        "recovery_generated_step_binding_timing"
                    )
                    == "before StepPlan lint"
                ),
            ):
                inner_recovery_verify_command_violations.append(str(pair_id))
            if contract.get("smoke", {}).get("require_fix_contract_continuity") is True:
                resumptions = recovery_attempts.get("fix_contract_resumptions")
                if not _valid_fix_contract_continuity(
                    resumptions,
                    expected_reproducer,
                    require_immutable_origin=(
                        "recovery_fix_origin_evidence_policy"
                        in contract.get("analysis", {})
                    ),
                ):
                    fix_contract_continuity_violations.append(str(pair_id))
            if contract.get("smoke", {}).get(
                "require_recovery_fix_terminal_completion"
            ) is True and not _valid_recovery_fix_terminal_completion(
                recovery_result,
                recovery_attempts,
                comparison_for_terminal,
                allow_honest_not_recoverable=(
                    contract.get("smoke", {}).get(
                        "recovery_fix_terminal_outcome_policy"
                    )
                    == RECOVERY_FIX_TERMINAL_OUTCOME_POLICY
                ),
            ):
                recovery_fix_terminal_completion_violations.append(str(pair_id))
            if contract.get("smoke", {}).get(
                "require_recovery_handoff_fidelity_v2"
            ) is True and not _valid_recovery_handoff_fidelity_v2(
                recovery_attempts.get("handoff_fidelity")
            ):
                recovery_handoff_fidelity_v2_violations.append(str(pair_id))
            mutation_observations = recovery_attempts.get(
                "product_mutation_observations"
            )
            if contract.get("smoke", {}).get(
                "require_recovery_product_mutation_observation"
            ) is True and not _valid_product_mutation_observations(
                mutation_observations
            ):
                recovery_product_mutation_observation_violations.append(str(pair_id))
            if contract.get("smoke", {}).get(
                "require_recovery_fix_safety_verification"
            ) is True and not _valid_fix_safety_verifications(
                recovery_attempts.get("fix_safety_verifications")
            ):
                recovery_fix_safety_verification_violations.append(str(pair_id))
            if contract.get("smoke", {}).get(
                "require_recovery_bounded_local_repair_max_one"
            ) is True and not _bounded_local_repair_at_most_one(mutation_observations):
                recovery_bounded_local_repair_violations.append(str(pair_id))
            if contract.get("smoke", {}).get(
                "require_recovery_treatment_delta"
            ) is True and not _valid_recovery_treatment_delta(
                recovery_attempts.get("treatment_deltas")
            ):
                recovery_treatment_delta_violations.append(str(pair_id))
            treatment_path = recovery_attempt.get("recovery_treatment_path")
            if not (
                isinstance(treatment_path, str)
                and treatment_path.startswith(
                    ".commandagent/recovery-treatments/attempt-"
                )
                and treatment_path.endswith("/workspace")
            ):
                treatment_isolation_violations.append(str(pair_id))
        if initial.get("input_manifest", {}).get("snapshot_sha256") != recovery.get(
            "input_manifest", {}
        ).get("snapshot_sha256"):
            snapshot_mismatches.append(pair_id)
        _check_oracle_source(recovery, pair_id, oracle_source_violations)
        if (
            contract.get("smoke", {}).get("require_separate_browser_oracle_preflight")
            is not True
        ):
            _check_browser_oracle_executability(
                recovery, pair_id, browser_oracle_unavailable
            )
        _check_resources(recovery_result, pair_id, "recovery_one", resource_missing)
        _check_manifest_policy(recovery, pair_id, manifest_policy_violations)
        _check_execution_action(
            recovery_result,
            contract=contract,
            pair_id=pair_id,
            arm="recovery_one",
            errors=execution_action_violations,
        )
        comparison = record.get("comparison")
        if not isinstance(comparison, dict):
            ineligible_recovery_violations.append(f"comparison_missing:{pair_id}")
            continue
        transitions.append(comparison.get("quality_transition"))
        if record.get("pairing_unit") == "shared_pre_recovery_snapshot":
            shared_history.append(comparison.get("shared_initial_history") is True)
            # A preregistered dependency/capability exclusion has no Recovery
            # treatment and therefore no before/after fix polarity to validate.
            # It remains governed by ineligible_recovery_not_executed; applying
            # treatment semantics here would make an honest exclusion fail the
            # unrelated Recovery-effect gate.
            if runtime_eligible is True:
                semantic_validation.append(
                    comparison.get("oracle_semantics", {}).get("valid") is True
                )
            if executed_recovery_runs == 1:
                boundary_matches.append(
                    comparison.get("control_snapshot_matches_boundary") is True
                )
                pre_recovery_handoffs.append(
                    initial_result.get("terminal_status", {}).get(
                        "recovery_handoff_kind"
                    )
                    is not None
                )
                changed_paths_recorded.append(
                    isinstance(comparison.get("recovery_changed_paths"), dict)
                    and isinstance(
                        comparison.get("recovery_changed_paths", {}).get(
                            "change_count"
                        ),
                        int,
                    )
                )
                attribution_ready.append(
                    comparison.get("effect_attribution_ready") is True
                )
            matrix_recorded.append(
                isinstance(comparison.get("internal_external_outcome_matrix"), dict)
            )
            runtime_category = (
                record.get("eligibility", {}).get("runtime", {}).get("category")
            )
            if (
                runtime_category == "initial_success"
                and comparison.get("success_improved") is True
            ):
                initial_success_attribution_violations.append(str(pair_id))
        for field, values in deltas.items():
            value = comparison.get("resource_delta", {}).get(field)
            if isinstance(value, int) and not isinstance(value, bool):
                values.append(value)
    paired_execution_action = contract.get("paired_run_contract", {}).get(
        "execution_action", "plan_run"
    )
    smoke = contract.get("smoke", {})
    minimum_executed = smoke.get("minimum_executed_recovery_pairs", 0)
    minimum_suppressions = smoke.get("minimum_current_success_suppressions", 0)
    attributed_harm_pair_ids = [
        str(record.get("pair_id"))
        for record in records
        if (record.get("comparison") or {}).get("quality_transition") == "harmed"
    ]
    regression_introduced_pair_ids = [
        str(record.get("pair_id"))
        for record in records
        if (record.get("comparison") or {}).get("regression_introduced") is True
    ]
    existing_artifact_harmed_pair_ids = [
        str(record.get("pair_id"))
        for record in records
        if (record.get("comparison") or {}).get("existing_artifact_harmed") is True
    ]
    instrumentation_unusable_pair_ids = [
        str(record.get("pair_id"))
        for record in records
        if record.get("eligibility", {}).get("runtime", {}).get("category")
        == "instrumentation_unavailable"
        or (record.get("comparison") or {}).get("quality_transition") == "unusable"
    ]
    if smoke.get("require_separate_browser_oracle_preflight") is True:
        browser_oracle_unavailable.extend(
            _browser_preflight_errors(
                oracle_executability_preflight,
                contract=contract,
            )
        )
    checks = {
        "target_pairs_complete": len(records) == expected,
        "initial_arm_configured_zero": bool(configured_zero) and all(configured_zero),
        "recovery_arm_configured_one_or_preregistered_not_run": all(configured_one),
        "maximum_one_recovery_executed": all(maximum_one_executed),
        "ineligible_recovery_not_executed": not ineligible_recovery_violations,
        "paired_input_snapshot_match": not snapshot_mismatches,
        "frozen_external_oracle_post_execution": not oracle_source_violations,
        "resource_measurement_complete": not resource_missing,
        "candidate_manifest_policy_applied": not manifest_policy_violations,
        "execution_action_matches_contract": not execution_action_violations,
        "recovery_capable_execution_action": (
            smoke.get("require_recovery_capable_execution_action") is not True
            or paired_execution_action == "ultra_plan_run"
        ),
        "minimum_executed_recovery_pairs_observed": (
            isinstance(minimum_executed, int)
            and not isinstance(minimum_executed, bool)
            and len(executed_recovery_pairs) >= minimum_executed
        ),
        "current_success_suppression_observed": (
            isinstance(minimum_suppressions, int)
            and not isinstance(minimum_suppressions, bool)
            and len(current_success_suppressions) >= minimum_suppressions
        ),
        "browser_oracle_executability_preflight": (
            smoke.get("require_browser_oracle_executability") is not True
            or not browser_oracle_unavailable
        ),
        "transaction_control_retention": (
            smoke.get("require_transaction_control_retention") is not True
            or not transaction_control_violations
        ),
        "recovery_handoff_fidelity": (
            smoke.get("require_recovery_handoff_fidelity") is not True
            or not handoff_fidelity_violations
        ),
        "isolated_recovery_treatment": (
            smoke.get("require_isolated_treatment_workspace") is not True
            or not treatment_isolation_violations
        ),
        "typed_fix_reproducer_binding": not typed_reproducer_violations,
    }
    if smoke.get("require_preselected_pair_denominator_exact") is True:
        selected_pair_ids = smoke.get("selected_pair_ids", [])
        checks["preselected_pair_denominator_exact"] = (
            len(observed_pair_ids) == len(selected_pair_ids)
            and len(set(observed_pair_ids)) == len(observed_pair_ids)
            and set(observed_pair_ids) == set(selected_pair_ids)
        )
    if smoke.get("require_recovery_safety_zero") is True:
        checks.update(
            {
                "attributed_harm_zero": not attributed_harm_pair_ids,
                "regression_introduced_zero": not regression_introduced_pair_ids,
                "existing_artifact_harm_zero": (not existing_artifact_harmed_pair_ids),
                "instrumentation_unusable_zero": (
                    not instrumentation_unusable_pair_ids
                ),
            }
        )
    if smoke.get("require_registered_recovery_verify_commands") is True:
        checks[
            "registered_recovery_verify_commands"
        ] = not recovery_verify_command_source_violations
    if smoke.get("require_registered_inner_recovery_verify_commands") is True:
        checks[
            "registered_inner_recovery_verify_commands"
        ] = not inner_recovery_verify_command_violations
    if smoke.get("require_fix_contract_continuity") is True:
        checks["fix_contract_continuity"] = not fix_contract_continuity_violations
    if smoke.get("require_recovery_fix_terminal_completion") is True:
        checks[
            "recovery_fix_terminal_completion"
        ] = not recovery_fix_terminal_completion_violations
    if smoke.get("require_recovery_handoff_fidelity_v2") is True:
        checks[
            "recovery_handoff_fidelity_v2"
        ] = not recovery_handoff_fidelity_v2_violations
    if smoke.get("require_recovery_product_mutation_observation") is True:
        checks[
            "recovery_product_mutation_observation"
        ] = not recovery_product_mutation_observation_violations
    if smoke.get("require_recovery_fix_safety_verification") is True:
        checks[
            "recovery_fix_safety_verification"
        ] = not recovery_fix_safety_verification_violations
    if smoke.get("require_recovery_bounded_local_repair_max_one") is True:
        checks[
            "recovery_bounded_local_repair_max_one"
        ] = not recovery_bounded_local_repair_violations
    if smoke.get("require_recovery_treatment_delta") is True:
        checks["recovery_treatment_delta"] = not recovery_treatment_delta_violations
    if smoke.get("require_discarded_valid_treatment_zero") is True:
        checks["discarded_valid_treatment_zero"] = not discarded_valid_treatments
    shared_pairing = contract.get("paired_run_contract", {}).get("pairing_unit") == (
        "shared_pre_recovery_snapshot"
    )
    if shared_pairing:
        require_attributed = smoke.get(
            "require_executed_recovery_for_attribution", True
        )
        checks.update(
            {
                "recovery_attribution_requires_shared_initial_history": (
                    bool(shared_history) and all(shared_history)
                ),
                "pre_recovery_snapshot_matches_control": (
                    (bool(boundary_matches) and all(boundary_matches))
                    or (not require_attributed and not boundary_matches)
                ),
                "pre_recovery_failure_handoff_recorded": (
                    (bool(pre_recovery_handoffs) and all(pre_recovery_handoffs))
                    or (not require_attributed and not pre_recovery_handoffs)
                ),
                "final_success_oracle_semantics_validated": (
                    bool(semantic_validation) and all(semantic_validation)
                ),
                "fix_before_and_after_polarity_distinct": (
                    bool(semantic_validation) and all(semantic_validation)
                ),
                "initial_success_pair_not_attributed": (
                    not initial_success_attribution_violations
                ),
                "recovery_changed_paths_recorded": (
                    (bool(changed_paths_recorded) and all(changed_paths_recorded))
                    or (not require_attributed and not changed_paths_recorded)
                ),
                "internal_external_outcome_matrix_recorded": (
                    bool(matrix_recorded) and all(matrix_recorded)
                ),
            }
        )
    return {
        "schema_version": "commandagent.goal_verify.recovery_report.v4_a14",
        "contract_id": contract["contract_id"],
        "run_id": contract["smoke_run_id"],
        "inference_role": "instrument diagnostic only",
        "effect_claim_allowed": False,
        "record_count": len(records),
        "checks": checks,
        "instrument_ready": all(checks.values()),
        "effect_attribution_ready": (
            all(checks.values()) and bool(attribution_ready) and all(attribution_ready)
        ),
        "counts": {
            "attributed_improved": transitions.count("improved"),
            "attributed_harmed": transitions.count("harmed"),
            "unchanged_pass": transitions.count("unchanged_pass"),
            "unchanged_fail": transitions.count("unchanged_fail"),
            "initial_attempt_divergence": transitions.count(
                "initial_attempt_divergence"
            ),
            "no_recovery_needed": transitions.count("no_recovery_needed"),
            "no_recovery_executed": transitions.count("no_recovery_executed"),
            "unusable": transitions.count("unusable"),
            "discarded_valid_treatment": discarded_valid_treatment_count,
        },
        "median_resource_delta": {
            field: statistics.median(values) if values else None
            for field, values in deltas.items()
        },
        "diagnostics": {
            "ineligible_recovery_violations": ineligible_recovery_violations,
            "snapshot_mismatches": snapshot_mismatches,
            "oracle_source_violations": oracle_source_violations,
            "resource_missing": resource_missing,
            "manifest_policy_violations": manifest_policy_violations,
            "execution_action_violations": execution_action_violations,
            "paired_execution_action": paired_execution_action,
            "executed_recovery_pair_ids": executed_recovery_pairs,
            "current_success_suppression_pair_ids": current_success_suppressions,
            "browser_oracle_unavailable": browser_oracle_unavailable,
            "transaction_control_violations": transaction_control_violations,
            "handoff_fidelity_violations": handoff_fidelity_violations,
            "treatment_isolation_violations": treatment_isolation_violations,
            "typed_reproducer_violations": typed_reproducer_violations,
            "discarded_valid_treatment_pair_ids": discarded_valid_treatments,
            "missing_preselected_pair_ids": sorted(
                set(smoke.get("selected_pair_ids", [])) - set(observed_pair_ids)
            ),
            "unexpected_observed_pair_ids": sorted(
                pair_id
                for pair_id in set(observed_pair_ids)
                - set(smoke.get("selected_pair_ids", []))
                if isinstance(pair_id, str)
            ),
            "attributed_harm_pair_ids": attributed_harm_pair_ids,
            "regression_introduced_pair_ids": regression_introduced_pair_ids,
            "existing_artifact_harmed_pair_ids": (existing_artifact_harmed_pair_ids),
            "instrumentation_unusable_pair_ids": instrumentation_unusable_pair_ids,
            "initial_success_attribution_violations": (
                initial_success_attribution_violations
            ),
            **(
                {
                    "recovery_verify_command_source_violations": (
                        recovery_verify_command_source_violations
                    )
                }
                if smoke.get("require_registered_recovery_verify_commands") is True
                else {}
            ),
            **(
                {
                    "inner_recovery_verify_command_violations": (
                        inner_recovery_verify_command_violations
                    )
                }
                if smoke.get("require_registered_inner_recovery_verify_commands")
                is True
                else {}
            ),
            **(
                {
                    "fix_contract_continuity_violations": (
                        fix_contract_continuity_violations
                    )
                }
                if smoke.get("require_fix_contract_continuity") is True
                else {}
            ),
            **(
                {
                    "recovery_fix_terminal_completion_violations": (
                        recovery_fix_terminal_completion_violations
                    )
                }
                if smoke.get("require_recovery_fix_terminal_completion") is True
                else {}
            ),
            **(
                {
                    "recovery_handoff_fidelity_v2_violations": (
                        recovery_handoff_fidelity_v2_violations
                    )
                }
                if smoke.get("require_recovery_handoff_fidelity_v2") is True
                else {}
            ),
            **(
                {
                    "recovery_product_mutation_observation_violations": (
                        recovery_product_mutation_observation_violations
                    )
                }
                if smoke.get("require_recovery_product_mutation_observation") is True
                else {}
            ),
            **(
                {
                    "recovery_fix_safety_verification_violations": (
                        recovery_fix_safety_verification_violations
                    )
                }
                if smoke.get("require_recovery_fix_safety_verification") is True
                else {}
            ),
            **(
                {
                    "recovery_bounded_local_repair_violations": (
                        recovery_bounded_local_repair_violations
                    )
                }
                if smoke.get("require_recovery_bounded_local_repair_max_one") is True
                else {}
            ),
            **(
                {
                    "recovery_treatment_delta_violations": (
                        recovery_treatment_delta_violations
                    )
                }
                if smoke.get("require_recovery_treatment_delta") is True
                else {}
            ),
        },
    }


def _valid_recovery_handoff_fidelity_v2(value: Any) -> bool:
    if not isinstance(value, list) or len(value) != 1:
        return False
    row = value[0]
    return (
        isinstance(row, dict)
        and row.get("event") == "recovery_handoff_fidelity_bound"
        and row.get("fidelity_ok") is True
        and row.get("goal_source") == "completion_contract"
        and row.get("contract_bound") is True
        and isinstance(row.get("verify_command_count"), int)
        and not isinstance(row.get("verify_command_count"), bool)
        and row["verify_command_count"] > 0
        and isinstance(row.get("repair_target_count"), int)
        and not isinstance(row.get("repair_target_count"), bool)
        and row["repair_target_count"] > 0
    )


def _valid_product_mutation_observations(value: Any) -> bool:
    return (
        isinstance(value, list)
        and bool(value)
        and all(
            isinstance(row, dict)
            and row.get("stage") in {"initial", "bounded_local_repair"}
            and isinstance(row.get("reported_changed_paths"), list)
            and isinstance(row.get("observed_changed_paths"), list)
            and isinstance(row.get("no_op_reported_paths"), list)
            and isinstance(row.get("unreported_mutation_paths"), list)
            and isinstance(row.get("mutation_observed"), bool)
            for row in value
        )
    )


def _bounded_local_repair_at_most_one(value: Any) -> bool:
    return (
        _valid_product_mutation_observations(value)
        and sum(row.get("stage") == "bounded_local_repair" for row in value) <= 1
    )


def _valid_fix_safety_verifications(value: Any) -> bool:
    return (
        isinstance(value, list)
        and bool(value)
        and all(
            isinstance(row, dict)
            and isinstance(row.get("registered_verify_commands"), list)
            and bool(row["registered_verify_commands"])
            and isinstance(row.get("referenced_api_surface_count"), int)
            and not isinstance(row.get("referenced_api_surface_count"), bool)
            and row["referenced_api_surface_count"] >= 0
            and isinstance(row.get("referenced_api_violations"), list)
            and isinstance(row.get("changed_paths"), list)
            and isinstance(row.get("ok"), bool)
            for row in value
        )
    )


def _valid_recovery_treatment_delta(value: Any) -> bool:
    if not isinstance(value, list) or len(value) != 1:
        return False
    row = value[0]
    if not isinstance(row, dict) or row.get("status") != "observed":
        return False
    return all(
        isinstance(row.get(field), dict)
        and all(
            isinstance(row[field].get(path_field), list)
            for path_field in ("changed_paths", "added_paths", "removed_paths")
        )
        for field in ("attempted_product_delta", "treatment_runtime_evidence_delta")
    )


def _valid_fix_contract_continuity(
    value: Any,
    expected_reproducer: Any,
    *,
    require_immutable_origin: bool = False,
) -> bool:
    if not isinstance(value, list) or len(value) != 1:
        return False
    row = value[0]
    valid = (
        isinstance(row, dict)
        and row.get("original_intent") == "fix"
        and row.get("contract_origin") == "fix_intent_v0"
        and row.get("contract_version") == "v0"
        and row.get("contract_ref") == "docs/fix-intent-contract.md"
        and isinstance(row.get("fix_run_id"), str)
        and bool(row.get("fix_run_id"))
        and row.get("reproducer_command") == expected_reproducer
        and row.get("source") == "host_owned_recovery_fix_origin"
        and row.get("external_oracle_used") is False
    )
    if not require_immutable_origin:
        return valid
    return (
        valid
        and row.get("origin_evidence_path")
        == ".commandagent/recovery-runtime/fix-origin-evidence.json"
        and isinstance(row.get("origin_evidence_sha256"), str)
        and len(row["origin_evidence_sha256"]) == 64
    )


def _valid_recovery_fix_terminal_completion(
    result: dict[str, Any],
    attempts: dict[str, Any],
    comparison: dict[str, Any],
    *,
    allow_honest_not_recoverable: bool = False,
) -> bool:
    recovery_attempt = next(
        (row for row in attempts.get("attempts", []) if row.get("attempt_index") == 1),
        {},
    )
    promotion_decisions = attempts.get("promotion_decisions")
    terminal = result.get("terminal_status", {})
    promoted_success = (
        result.get("status") == "completed"
        and result.get("returncode") == 0
        and result.get("completion_verify_passed") is True
        and terminal.get("ok") is True
        and terminal.get("status") == "completed"
        and recovery_attempt.get("status") == "succeeded"
        and recovery_attempt.get("stop_reason") == "recovery_succeeded"
        and attempts.get("terminal_stop_reason") == "recovery_succeeded"
        and isinstance(promotion_decisions, list)
        and len(promotion_decisions) == 1
        and promotion_decisions[0].get("decision") == "promoted"
    )
    if promoted_success:
        return True
    if not allow_honest_not_recoverable:
        return False
    return (
        result.get("status") == "failed"
        and isinstance(result.get("returncode"), int)
        and not isinstance(result.get("returncode"), bool)
        and result["returncode"] != 0
        and result.get("completion_verify_attempt_recorded") is True
        and result.get("completion_verify_passed") is False
        and terminal.get("recorded") is True
        and terminal.get("ok") is False
        and terminal.get("status") == "failed"
        and recovery_attempt.get("status") == "failed"
        and recovery_attempt.get("stop_reason") == "not_recoverable"
        and attempts.get("terminal_stop_reason") == "not_recoverable"
        and isinstance(promotion_decisions, list)
        and promotion_decisions
        == [{"decision": "rejected", "reason": "recovery_execution_failed"}]
        and attempts.get("control_retained_count") == 1
        and attempts.get("control_restore_failed_count") == 0
        and comparison.get("quality_transition") == "unchanged_fail"
        and comparison.get("raw_oracle_transition") == "unchanged_fail"
        and comparison.get("initial_oracle_status") == "fail"
        and comparison.get("recovery_oracle_status") == "fail"
        and comparison.get("recovery_regression_status") == "pass"
        and comparison.get("regression_introduced") is False
        and comparison.get("existing_artifact_harmed") is False
        and comparison.get("control_snapshot_matches_boundary") is True
        and comparison.get("shared_initial_history") is True
    )


def _valid_inner_recovery_bindings(
    value: Any, *, require_pre_lint: bool = False
) -> bool:
    if not isinstance(value, list) or not value:
        return False
    modes = set()
    for row in value:
        if not isinstance(row, dict):
            return False
        mode = row.get("binding_mode")
        modes.add(mode)
        if (
            row.get("source") != "product_visible_completion_contract"
            or row.get("external_oracle_used") is not False
            or (require_pre_lint and row.get("binding_stage") != "pre_lint")
        ):
            return False
        bound = row.get("bound_verify_commands")
        registered = row.get("registered_verify_commands")
        if mode == "read_only_inspection":
            if bound != []:
                return False
        elif mode == "completion_contract_final_success":
            if (
                not isinstance(registered, list)
                or not registered
                or bound != registered
            ):
                return False
        else:
            return False
    return modes == {"read_only_inspection", "completion_contract_final_success"}


def _typed_reproducer_matches(value: Any, expected_command: str) -> bool:
    return isinstance(value, dict) and value == {
        "observed": True,
        "synthesized_event_count": 1,
        "r_bases": ["completion_contract:fix_reproducer_command"],
        "reproduce_before_step_counts": [1],
        "before_evidence_count": 1,
        "executed_before_failure_count": 1,
        "binding_ids": [expected_command],
    }


def _check_oracle_source(arm: dict[str, Any], pair_id: Any, errors: list[str]) -> None:
    if (
        arm.get("external_oracles", {}).get("source")
        != "frozen_host_adapter_post_execution"
    ):
        errors.append(str(pair_id))


def _check_browser_oracle_executability(
    arm: dict[str, Any], pair_id: Any, errors: list[str]
) -> None:
    for row in arm.get("external_oracles", {}).get("outcomes", []):
        if row.get("executor_kind") != "playwright_script":
            continue
        outcome = row.get("outcome", {})
        if outcome.get("executed") is not True:
            errors.append(
                f"{pair_id}:{row.get('adapter_id')}:{outcome.get('reason', 'unknown')}"
            )


def _browser_preflight_errors(
    preflight: dict[str, Any] | None,
    *,
    contract: dict[str, Any],
) -> list[str]:
    if not isinstance(preflight, dict):
        return ["separate_preflight_missing"]
    errors = []
    if preflight.get("contract_id") != contract.get("contract_id"):
        errors.append("separate_preflight_contract_mismatch")
    if preflight.get("run_id") != contract.get("smoke_run_id"):
        errors.append("separate_preflight_run_mismatch")
    if preflight.get("source") != "frozen_reference_workspace":
        errors.append("separate_preflight_source_invalid")
    if preflight.get("passed_to_product_or_recovery") is not False:
        errors.append("separate_preflight_information_boundary_invalid")
    rows = preflight.get("outcomes")
    if not isinstance(rows, list) or not rows:
        return [*errors, "separate_preflight_outcomes_missing"]
    for row in rows:
        if not isinstance(row, dict):
            errors.append("separate_preflight:invalid:row_invalid")
            continue
        adapter_id = row.get("adapter_id")
        if row.get("candidate_visible") is not False:
            errors.append(
                f"separate_preflight:{adapter_id}:candidate_visibility_invalid"
            )
        build = row.get("build", {}).get("outcome", {})
        if build.get("executed") is not True or build.get("result") != "pass":
            errors.append(
                f"separate_preflight:{adapter_id}:"
                f"{build.get('reason', 'reference_build_invalid')}"
            )
        outcome = row.get("outcome", {}) if isinstance(row, dict) else {}
        if outcome.get("executed") is not True or outcome.get("result") != "pass":
            errors.append(
                f"separate_preflight:{adapter_id}:{outcome.get('reason', 'unknown')}"
            )
    return errors


def _check_resources(
    result: dict[str, Any], pair_id: Any, arm: str, errors: list[str]
) -> None:
    usage = result.get("resource_usage", {})
    if any(
        not isinstance(usage.get(field), int) or isinstance(usage.get(field), bool)
        for field in ("wall_time_ms", "input_tokens", "output_tokens", "total_tokens")
    ):
        errors.append(f"{pair_id}:{arm}")


def _check_manifest_policy(
    arm: dict[str, Any], pair_id: Any, errors: list[str]
) -> None:
    manifest = arm.get("output_artifact_manifest", {})
    if manifest.get("candidate_visibility_policy") != (
        "commandagent.goal_verify.candidate_manifest.source_config_v1"
    ):
        errors.append(str(pair_id))


def _check_execution_action(
    result: dict[str, Any],
    *,
    contract: dict[str, Any],
    pair_id: Any,
    arm: str,
    errors: list[str],
) -> None:
    action = contract.get("paired_run_contract", {}).get("execution_action", "plan_run")
    expected = {
        "plan_run": "--plan-run",
        "ultra_plan_run": "--ultra-plan-run",
    }.get(action)
    argv = result.get("argv")
    action_flags = {"--plan-run", "--ultra-plan-run"}
    observed = (
        [argument for argument in argv if argument in action_flags]
        if isinstance(argv, list)
        else []
    )
    if expected is None or observed != [expected]:
        errors.append(f"{pair_id}:{arm}:{observed}")
