from __future__ import annotations

import statistics
from typing import Any

from eval_lib.goal_verify_recovery_experiment_v4 import (
    SMOKE_PROFILE_PATH_COVERAGE_POLICY,
    SMOKE_PROFILE_PATH_COVERAGE_POLICY_V2,
)
from eval_lib.goal_verify_recovery_full_report_v4 import (
    _effect_delta,
    _percentile,
    _resource_summary,
    build_recovery_full_report,
)
from eval_lib.goal_verify_recovery_report_v4 import build_recovery_report
from eval_lib.goal_verify_stats_v2 import (
    stratified_cluster_paired_bootstrap_interval,
    validate_cluster_design,
)


def build_recovery_a15_smoke_report(
    *,
    records: list[dict[str, Any]],
    contract: dict[str, Any],
    oracle_executability_preflight: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Require usable product and Recovery plumbing in every real profile."""
    base = build_recovery_report(
        records=records,
        contract=contract,
        oracle_executability_preflight=oracle_executability_preflight,
    )
    smoke = contract["smoke"]
    required_profiles = smoke["required_real_profiles"]
    minimum_pairs = int(smoke["minimum_pairs_per_real_profile"])
    minimum_executed = int(smoke["minimum_executed_recovery_pairs_per_real_profile"])
    minimum_executed_clusters = int(
        smoke.get("minimum_executed_recovery_clusters_per_real_profile", 0)
    )
    profile_path_policy = smoke.get("real_profile_path_coverage_policy")
    allow_current_success_coverage = profile_path_policy in (
        SMOKE_PROFILE_PATH_COVERAGE_POLICY,
        SMOKE_PROFILE_PATH_COVERAGE_POLICY_V2,
    )
    require_explicit_current_success_protection = (
        profile_path_policy == SMOKE_PROFILE_PATH_COVERAGE_POLICY
    )
    eligible = [
        row
        for row in records
        if row.get("eligibility", {}).get("preregistered", {}).get("eligible") is True
    ]
    profile_readiness = {}
    for profile in required_profiles:
        rows = [row for row in eligible if row.get("profile") == profile]
        executed = [
            row
            for row in rows
            if (row.get("comparison") or {}).get("executed_recovery_runs") == 1
        ]
        executed_clusters = sorted(
            {
                cluster
                for row in executed
                if isinstance((cluster := _source_task_id(row)), str)
            }
        )
        unusable = [
            row.get("pair_id") for row in rows if _instrumentation_unusable(row)
        ]
        explicit_suppressions = [
            row.get("pair_id")
            for row in rows
            if _explicit_current_success_suppressed(row)
        ]
        minimum_executed_met = len(executed) >= minimum_executed
        current_success_only_coverage = (
            allow_current_success_coverage
            and not minimum_executed_met
            and _current_success_only_coverage(
                rows,
                require_explicit_protection=require_explicit_current_success_protection,
            )
        )
        profile_readiness[profile] = {
            "pair_count": len(rows),
            "executed_recovery_pairs": len(executed),
            "executed_recovery_clusters": len(executed_clusters),
            "executed_recovery_cluster_ids": executed_clusters,
            "instrumentation_unusable_pair_ids": unusable,
            "explicit_current_success_suppression_pair_ids": explicit_suppressions,
            "explicit_current_success_suppression_count": len(explicit_suppressions),
            "minimum_pairs_met": len(rows) >= minimum_pairs,
            "minimum_executed_recovery_met": minimum_executed_met,
            "minimum_executed_recovery_clusters_met": (
                len(executed_clusters) >= minimum_executed_clusters
            ),
            "current_success_only_coverage": current_success_only_coverage,
            "path_coverage_mode": (
                "executed_recovery"
                if minimum_executed_met
                else (
                    (
                        "all_initial_oracle_pass_with_current_success_protection"
                        if require_explicit_current_success_protection
                        else "all_initial_oracle_pass_without_recovery"
                    )
                    if current_success_only_coverage
                    else "missing"
                )
            ),
            "path_coverage_met": minimum_executed_met or current_success_only_coverage,
            "external_oracles_usable": not unusable,
        }
    a15_checks = {
        "all_real_profiles_present": set(profile_readiness) == set(required_profiles)
        and all(row["minimum_pairs_met"] for row in profile_readiness.values()),
        "external_oracles_usable_in_every_real_profile": all(
            row["external_oracles_usable"] for row in profile_readiness.values()
        ),
    }
    if allow_current_success_coverage:
        a15_checks[
            "recovery_or_current_success_path_observed_in_every_real_profile"
        ] = all(row["path_coverage_met"] for row in profile_readiness.values())
    else:
        a15_checks["recovery_executed_in_every_real_profile"] = all(
            row["minimum_executed_recovery_met"] for row in profile_readiness.values()
        )
    if minimum_executed_clusters > 0:
        a15_checks["minimum_executed_recovery_clusters_in_every_real_profile"] = all(
            row["minimum_executed_recovery_clusters_met"]
            for row in profile_readiness.values()
        )
    ready = base["instrument_ready"] and all(a15_checks.values())
    resource_analysis = _smoke_resource_analysis(records, required_profiles)
    return {
        **base,
        "schema_version": "commandagent.goal_verify.recovery_report.v4_a15_smoke",
        "instrument_ready": ready,
        "a15_profile_smoke_checks": a15_checks,
        "profile_readiness": profile_readiness,
        "smoke_resource_analysis": resource_analysis,
        "go_no_go": "GO" if ready else "NO-GO",
    }


def build_recovery_a15_full_report(
    *,
    records: list[dict[str, Any]],
    contract: dict[str, Any],
    oracle_executability_preflight: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Require positive, usable Recovery evidence in every frozen profile."""
    base = build_recovery_full_report(
        records=records,
        contract=contract,
        oracle_executability_preflight=oracle_executability_preflight,
    )
    design = contract["full_experiment"]
    eligible = set(design["eligible_pair_ids"])
    eligible_records = [row for row in records if row.get("pair_id") in eligible]
    profile_cells = design["profile_cells"]
    minimum_executed = int(design["minimum_executed_recovery_pairs_per_profile"])
    profile_effects = {}
    for offset, (cell_id, profile) in enumerate(profile_cells.items(), 1):
        rows = [row for row in eligible_records if row.get("cell_id") == cell_id]
        executed = [
            row
            for row in rows
            if (row.get("comparison") or {}).get("executed_recovery_runs") == 1
        ]
        design_errors = validate_cluster_design(
            rows,
            minimum_clusters_per_cell=int(design["minimum_clusters_per_cell"]),
            minimum_pairs_per_cluster=int(design["pairs_per_eligible_cluster"]),
        )
        bootstrap = stratified_cluster_paired_bootstrap_interval(
            rows,
            delta=_effect_delta,
            samples=int(design["bootstrap_samples"]),
            seed=int(design["bootstrap_seed"]) + offset,
        )
        lower_positive = (
            bootstrap.get("status") == "estimated"
            and isinstance(bootstrap.get("lower"), (int, float))
            and bootstrap["lower"] > 0
        )
        resources = _resource_summary(executed, budgets=design["resource_budgets"])
        profile_effects[profile] = {
            "cell_id": cell_id,
            "eligible_pairs": len(rows),
            "executed_recovery_pairs": len(executed),
            "improved": sum(_effect_delta(row) == 1 for row in rows),
            "harmed": sum(_effect_delta(row) == -1 for row in rows),
            "point": (
                round(sum(_effect_delta(row) for row in rows) / len(rows), 6)
                if rows
                else None
            ),
            "bootstrap": bootstrap,
            "cluster_design_errors": design_errors,
            "minimum_executed_met": len(executed) >= minimum_executed,
            "ci_lower_above_zero": lower_positive,
            "resource_budget_evaluation": resources,
            "resource_budgets_met": (
                resources["measurement_complete"]
                and all(row["met"] for row in resources["checks"].values())
            ),
        }

    instrumentation_unusable = [
        row.get("pair_id") for row in eligible_records if _instrumentation_unusable(row)
    ]
    a15_checks = {
        "four_profiles_present": set(profile_effects)
        == {"cli", "generic", "data", "nextjs"},
        "minimum_executed_recovery_per_profile": all(
            row["minimum_executed_met"] for row in profile_effects.values()
        ),
        "profile_specific_ci_lower_above_zero": all(
            row["ci_lower_above_zero"] for row in profile_effects.values()
        ),
        "profile_resource_budgets_met": all(
            row["resource_budgets_met"] for row in profile_effects.values()
        ),
        "instrumentation_unusable_zero": not instrumentation_unusable,
    }
    ready = base["effect_claim_ready"] and all(a15_checks.values())
    return {
        **base,
        "schema_version": "commandagent.goal_verify.recovery_report.v4_a15_full",
        "inference_role": (
            "preregistered four-profile fix-intent Recovery 0-vs-1 effect estimate"
        ),
        "effect_claim_allowed": (design.get("effect_claim_allowed") is True and ready),
        "effect_claim_ready": ready,
        "all_profiles_quality_improved_claim_ready": ready,
        "go_no_go": "GO" if ready else "NO-GO",
        "a15_profile_checks": a15_checks,
        "profile_effects": profile_effects,
        "instrumentation_unusable_pair_ids": instrumentation_unusable,
    }


def _instrumentation_unusable(record: dict[str, Any]) -> bool:
    runtime = record.get("eligibility", {}).get("runtime", {})
    if runtime.get("category") == "instrumentation_unavailable":
        return True
    comparison = record.get("comparison")
    if not isinstance(comparison, dict):
        return True
    if comparison.get("quality_transition") in {"unusable", "unattributed"}:
        return True
    for arm in ("initial_only", "recovery_one"):
        value = record.get(arm)
        if not isinstance(value, dict):
            continue
        oracle = value.get("external_oracles")
        if isinstance(oracle, dict) and oracle.get("overall") == "unusable":
            return True
    return False


def _source_task_id(record: dict[str, Any]) -> str | None:
    for field in ("source_task_id", "case_id"):
        value = record.get(field)
        if isinstance(value, str) and value:
            return value
    pair_id = record.get("pair_id")
    if isinstance(pair_id, str) and "--pair-" in pair_id:
        return pair_id.rsplit("--pair-", 1)[0]
    return None


def _smoke_resource_analysis(
    records: list[dict[str, Any]], required_profiles: list[str]
) -> dict[str, Any]:
    executed = [
        row
        for row in records
        if (row.get("comparison") or {}).get("executed_recovery_runs") == 1
    ]
    improved = [
        row
        for row in executed
        if (row.get("comparison") or {}).get("quality_transition") == "improved"
    ]
    non_improving = [row for row in executed if row not in improved]
    return {
        "all_selected_pairs": _resource_distribution(records),
        "executed_recovery_pairs": _resource_distribution(executed),
        "improved_pairs": _resource_distribution(improved),
        "non_improving_executed_recovery_pairs": _resource_distribution(non_improving),
        "by_profile": {
            profile: {
                "all_pairs": _resource_distribution(
                    [row for row in records if row.get("profile") == profile]
                ),
                "executed_recovery_pairs": _resource_distribution(
                    [row for row in executed if row.get("profile") == profile]
                ),
            }
            for profile in required_profiles
        },
    }


def _resource_distribution(records: list[dict[str, Any]]) -> dict[str, Any]:
    fields = ("wall_time_ms", "input_tokens", "output_tokens", "total_tokens")
    values = {field: [] for field in fields}
    for record in records:
        resource_delta = (record.get("comparison") or {}).get("resource_delta", {})
        for field in fields:
            value = resource_delta.get(field)
            if isinstance(value, int) and not isinstance(value, bool):
                values[field].append(value)
    return {
        "pair_count": len(records),
        "measurement_complete": bool(records)
        and all(len(field_values) == len(records) for field_values in values.values()),
        "observed": {
            field: {
                "p50": statistics.median(field_values) if field_values else None,
                "p95": _percentile(field_values, 0.95) if field_values else None,
            }
            for field, field_values in values.items()
        },
    }


def _current_success_only_coverage(
    rows: list[dict[str, Any]], *, require_explicit_protection: bool
) -> bool:
    if not rows:
        return False
    explicit_protection_observed = False
    for row in rows:
        comparison = row.get("comparison")
        if not isinstance(comparison, dict):
            return False
        if (
            comparison.get("executed_recovery_runs") != 0
            or comparison.get("quality_transition") != "no_recovery_needed"
            or comparison.get("initial_oracle_status") != "pass"
            or comparison.get("recovery_oracle_status") != "pass"
            or comparison.get("regression_introduced") is not False
            or comparison.get("existing_artifact_harmed") is not False
        ):
            return False
        if _explicit_current_success_suppressed(row):
            explicit_protection_observed = True
    return explicit_protection_observed or not require_explicit_protection


def _explicit_current_success_suppressed(row: dict[str, Any]) -> bool:
    attempts = (
        row.get("recovery_one", {}).get("result", {}).get("recovery_plan_attempts", {})
    )
    return (
        attempts.get("current_success_suppressed") is True
        and attempts.get("terminal_stop_reason") == "current_success_protected"
    )
