from __future__ import annotations

from typing import Any

from eval_lib.goal_verify_recovery_a15_report import (
    build_recovery_a15_smoke_report,
)
from eval_lib.goal_verify_recovery_experiment_v4 import recovery_contract_errors

RECOVERY_INNER_VERIFY_BINDING_POLICY_V2 = {
    "schema_version": "commandagent.goal_verify.recovery_inner_verify_binding.v2",
    "promoted_or_product_mutating_attempt": (
        "requires both read-only inspection and completion-contract final-success "
        "bindings"
    ),
    "rejected_before_product_mutation": (
        "permits read-only inspection bindings without a final-success binding only "
        "when the attempted product delta is empty, the treatment is rejected, and "
        "control is retained"
    ),
}


def recovery_a25_contract_errors(contract: dict[str, Any]) -> list[str]:
    errors = recovery_contract_errors(contract)
    if (
        contract.get("smoke", {}).get(
            "require_registered_inner_recovery_verify_commands"
        )
        is not True
    ):
        errors.append("registered_inner_recovery_verify_commands_must_be_required")
    if (
        contract.get("smoke", {}).get("recovery_inner_verify_binding_policy")
        != RECOVERY_INNER_VERIFY_BINDING_POLICY_V2
    ):
        errors.append("recovery_inner_verify_binding_policy_invalid")
    return errors


def build_recovery_a25_pilot_report(
    *,
    records: list[dict[str, Any]],
    contract: dict[str, Any],
    oracle_executability_preflight: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Evaluate A25 with honest pre-mutation inner-binding semantics."""
    contract_errors = recovery_a25_contract_errors(contract)
    if contract_errors:
        raise ValueError("; ".join(contract_errors))
    base = build_recovery_a15_smoke_report(
        records=records,
        contract=contract,
        oracle_executability_preflight=oracle_executability_preflight,
    )
    require_pre_lint = (
        contract.get("analysis", {}).get("recovery_generated_step_binding_timing")
        == "before StepPlan lint"
    )
    inner_binding_violations = []
    executed_records = []
    for record in records:
        attempts = (
            record.get("recovery_one", {})
            .get("result", {})
            .get("recovery_plan_attempts", {})
        )
        if attempts.get("executed_recovery_runs") != 1:
            continue
        executed_records.append(record)
        if not _valid_inner_recovery_bindings_v2(
            attempts.get("step_plan_contract_bindings"),
            attempts,
            require_pre_lint=require_pre_lint,
        ):
            inner_binding_violations.append(str(record.get("pair_id")))

    base["checks"][
        "registered_inner_recovery_verify_commands"
    ] = not inner_binding_violations
    base["diagnostics"]["inner_recovery_verify_command_violations"] = (
        inner_binding_violations
    )
    base_checks_ready = all(base["checks"].values())
    attribution_observations = [
        (record.get("comparison") or {}).get("effect_attribution_ready") is True
        for record in executed_records
    ]
    base["effect_attribution_ready"] = (
        base_checks_ready
        and bool(attribution_observations)
        and all(attribution_observations)
    )
    profile_checks = base.get("a15_profile_smoke_checks")
    profile_checks_ready = (
        isinstance(profile_checks, dict)
        and bool(profile_checks)
        and all(profile_checks.values())
    )
    pilot_instrument_ready = base_checks_ready and profile_checks_ready
    base["instrument_ready"] = pilot_instrument_ready
    base["go_no_go"] = "GO" if pilot_instrument_ready else "NO-GO"

    design = contract["pilot_design"]
    threshold = design["natural_exposure_confirmation_threshold"]
    required_profiles = contract["smoke"]["required_real_profiles"]
    profile_readiness = base.get("profile_readiness", {})
    minimum_clusters = int(threshold["minimum_executed_recovery_clusters_per_profile"])
    profile_exposure = {
        profile: {
            "executed_recovery_clusters": int(
                profile_readiness.get(profile, {}).get("executed_recovery_clusters", 0)
            ),
            "minimum_required": minimum_clusters,
            "threshold_met": (
                int(
                    profile_readiness.get(profile, {}).get(
                        "executed_recovery_clusters", 0
                    )
                )
                >= minimum_clusters
            ),
        }
        for profile in required_profiles
    }
    profiles_meeting = sum(
        row["threshold_met"] is True for row in profile_exposure.values()
    )
    diagnostics = base.get("diagnostics", {})
    instrumentation_unusable = sorted(
        {
            str(pair_id)
            for pair_id in diagnostics.get("instrumentation_unusable_pair_ids", [])
        }
    )
    base_checks = base.get("checks", {})
    safety_check_names = threshold["safety_check_names"]
    failed_safety_checks = [
        name for name in safety_check_names if base_checks.get(name) is not True
    ]
    threshold_checks = {
        "pilot_instrument_ready": pilot_instrument_ready,
        "all_required_profiles_reported": set(profile_readiness)
        == set(required_profiles),
        "minimum_profiles_meeting_exposure_threshold": (
            profiles_meeting >= int(threshold["minimum_profiles_meeting_threshold"])
        ),
        "instrumentation_unusable_pairs_within_limit": (
            len(instrumentation_unusable)
            <= int(threshold["maximum_instrumentation_unusable_pairs"])
        ),
        "safety_violations_within_limit": (
            len(failed_safety_checks) <= int(threshold["maximum_safety_violations"])
        ),
    }
    natural_exposure_ready = all(threshold_checks.values())
    if not pilot_instrument_ready:
        decision = "pilot_invalid_requires_diagnosis"
        threshold_status = "INVALID"
    elif natural_exposure_ready:
        decision = "preregister_natural_exposure_confirmatory_experiment"
        threshold_status = "MET"
    else:
        decision = "preregister_deterministic_fault_boundary_experiment"
        threshold_status = "NOT_MET"
    return {
        **base,
        "schema_version": (
            "commandagent.goal_verify.recovery_natural_exposure_pilot_report.v2"
        ),
        "inference_role": design["inference_role"],
        "effect_claim_allowed": False,
        "effect_claim_ready": False,
        "pilot_instrument_ready": pilot_instrument_ready,
        "pilot_go_no_go": "GO" if pilot_instrument_ready else "NO-GO",
        "natural_exposure_confirmation_ready": natural_exposure_ready,
        "natural_exposure_threshold_status": threshold_status,
        "next_design_decision": decision,
        "natural_exposure_threshold_checks": threshold_checks,
        "profile_exposure": profile_exposure,
        "profiles_meeting_exposure_threshold": profiles_meeting,
        "instrumentation_unusable_pair_ids": instrumentation_unusable,
        "failed_safety_check_names": failed_safety_checks,
    }


def _attempted_product_changed_paths(attempts: dict[str, Any]) -> list[str] | None:
    deltas = attempts.get("treatment_deltas")
    if not isinstance(deltas, list) or len(deltas) != 1:
        return None
    delta = deltas[0]
    if not isinstance(delta, dict):
        return None
    product = delta.get("attempted_product_delta")
    if not isinstance(product, dict):
        return None
    paths = []
    for field in ("changed_paths", "added_paths", "removed_paths"):
        values = product.get(field)
        if not isinstance(values, list) or not all(
            isinstance(path, str) for path in values
        ):
            return None
        paths.extend(values)
    return sorted(set(paths))


def _valid_inner_recovery_bindings_v2(
    value: Any,
    attempts: dict[str, Any],
    *,
    require_pre_lint: bool = False,
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
            if bound != [] or not isinstance(registered, list) or not registered:
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
    complete_modes = {"read_only_inspection", "completion_contract_final_success"}
    if modes == complete_modes:
        return True
    if modes != {"read_only_inspection"}:
        return False
    attempted_paths = _attempted_product_changed_paths(attempts)
    return (
        attempted_paths == []
        and attempts.get("promotion_decisions")
        == [{"decision": "rejected", "reason": "recovery_execution_failed"}]
        and attempts.get("control_retained_count") == 1
        and attempts.get("control_restore_failed_count") == 0
        and attempts.get("terminal_stop_reason") == "not_recoverable"
    )
