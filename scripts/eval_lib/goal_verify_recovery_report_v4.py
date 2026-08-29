from __future__ import annotations

import statistics
from typing import Any


def build_recovery_report(
    *, records: list[dict[str, Any]], contract: dict[str, Any]
) -> dict[str, Any]:
    expected = int(contract["smoke"]["expected_pair_count"])
    configured_zero = []
    configured_one = []
    ineligible_recovery_violations = []
    snapshot_mismatches = []
    oracle_source_violations = []
    resource_missing = []
    manifest_policy_violations = []
    execution_action_violations = []
    executed_recovery_pairs = []
    transitions = []
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
        if runtime_eligible is not True:
            ineligible_recovery_violations.append(
                f"ineligible_recovery_executed:{pair_id}"
            )
        recovery_result = recovery.get("result", {})
        recovery_attempts = recovery_result.get("recovery_plan_attempts", {})
        executed_recovery_runs = recovery_attempts.get("executed_recovery_runs")
        configured_one.append(
            recovery_attempts.get("configured_recovery_runs") == 1
            and isinstance(executed_recovery_runs, int)
            and not isinstance(executed_recovery_runs, bool)
            and 0 <= executed_recovery_runs <= 1
        )
        if executed_recovery_runs == 1:
            executed_recovery_pairs.append(pair_id)
        if initial.get("input_manifest", {}).get("snapshot_sha256") != recovery.get(
            "input_manifest", {}
        ).get("snapshot_sha256"):
            snapshot_mismatches.append(pair_id)
        _check_oracle_source(recovery, pair_id, oracle_source_violations)
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
        for field, values in deltas.items():
            value = comparison.get("resource_delta", {}).get(field)
            if isinstance(value, int) and not isinstance(value, bool):
                values.append(value)
    paired_execution_action = contract.get("paired_run_contract", {}).get(
        "execution_action", "plan_run"
    )
    smoke = contract.get("smoke", {})
    minimum_executed = smoke.get("minimum_executed_recovery_pairs", 0)
    checks = {
        "target_pairs_complete": len(records) == expected,
        "initial_arm_configured_zero": bool(configured_zero) and all(configured_zero),
        "recovery_arm_configured_one_or_preregistered_not_run": all(configured_one),
        "maximum_one_recovery_executed": all(configured_one),
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
    }
    return {
        "schema_version": "commandagent.goal_verify.recovery_report.v4_a14",
        "contract_id": contract["contract_id"],
        "run_id": contract["smoke_run_id"],
        "inference_role": "instrument diagnostic only",
        "effect_claim_allowed": False,
        "record_count": len(records),
        "checks": checks,
        "instrument_ready": all(checks.values()),
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
        },
    }


def _check_oracle_source(arm: dict[str, Any], pair_id: Any, errors: list[str]) -> None:
    if (
        arm.get("external_oracles", {}).get("source")
        != "frozen_host_adapter_post_execution"
    ):
        errors.append(str(pair_id))


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
