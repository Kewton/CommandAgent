from __future__ import annotations

from typing import Any

from eval_lib.goal_verify_recovery_full_report_v4 import (
    _effect_delta,
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
    minimum_executed = int(
        smoke["minimum_executed_recovery_pairs_per_real_profile"]
    )
    eligible = [
        row
        for row in records
        if row.get("eligibility", {}).get("preregistered", {}).get("eligible")
        is True
    ]
    profile_readiness = {}
    for profile in required_profiles:
        rows = [row for row in eligible if row.get("profile") == profile]
        executed = [
            row
            for row in rows
            if (row.get("comparison") or {}).get("executed_recovery_runs") == 1
        ]
        unusable = [row.get("pair_id") for row in rows if _instrumentation_unusable(row)]
        profile_readiness[profile] = {
            "pair_count": len(rows),
            "executed_recovery_pairs": len(executed),
            "instrumentation_unusable_pair_ids": unusable,
            "minimum_pairs_met": len(rows) >= minimum_pairs,
            "minimum_executed_recovery_met": len(executed) >= minimum_executed,
            "external_oracles_usable": not unusable,
        }
    a15_checks = {
        "all_real_profiles_present": set(profile_readiness) == set(required_profiles)
        and all(row["minimum_pairs_met"] for row in profile_readiness.values()),
        "recovery_executed_in_every_real_profile": all(
            row["minimum_executed_recovery_met"]
            for row in profile_readiness.values()
        ),
        "external_oracles_usable_in_every_real_profile": all(
            row["external_oracles_usable"] for row in profile_readiness.values()
        ),
    }
    ready = base["instrument_ready"] and all(a15_checks.values())
    return {
        **base,
        "schema_version": "commandagent.goal_verify.recovery_report.v4_a15_smoke",
        "instrument_ready": ready,
        "a15_profile_smoke_checks": a15_checks,
        "profile_readiness": profile_readiness,
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
        resources = _resource_summary(
            executed, budgets=design["resource_budgets"]
        )
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
        row.get("pair_id")
        for row in eligible_records
        if _instrumentation_unusable(row)
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
        "effect_claim_allowed": (
            design.get("effect_claim_allowed") is True and ready
        ),
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
