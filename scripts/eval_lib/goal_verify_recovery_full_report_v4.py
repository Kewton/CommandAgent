from __future__ import annotations

import math
import statistics
from typing import Any

from eval_lib.goal_verify_recovery_report_v4 import build_recovery_report
from eval_lib.goal_verify_stats_v2 import (
    stratified_cluster_paired_bootstrap_interval,
    validate_cluster_design,
)


def build_recovery_full_report(
    *,
    records: list[dict[str, Any]],
    contract: dict[str, Any],
    oracle_executability_preflight: dict[str, Any] | None = None,
) -> dict[str, Any]:
    base = build_recovery_report(
        records=records,
        contract=contract,
        oracle_executability_preflight=oracle_executability_preflight,
    )
    design = contract["full_experiment"]
    eligible_pair_ids = set(design["eligible_pair_ids"])
    sentinel_pair_ids = set(design["sentinel_pair_ids"])
    eligible_records = [
        row for row in records if row.get("pair_id") in eligible_pair_ids
    ]
    sentinel_records = [
        row for row in records if row.get("pair_id") in sentinel_pair_ids
    ]
    observed_pair_ids = {row.get("pair_id") for row in records}
    design_errors = validate_cluster_design(
        eligible_records,
        minimum_clusters_per_cell=int(design["minimum_clusters_per_cell"]),
        minimum_pairs_per_cluster=int(design["pairs_per_eligible_cluster"]),
    )
    observed_cells = {row.get("cell_id") for row in eligible_records}
    if observed_cells != set(design["eligible_cell_ids"]):
        design_errors.append("eligible cells differ from the frozen design")
    executed_records = [
        row
        for row in eligible_records
        if (row.get("comparison") or {}).get("executed_recovery_runs") == 1
    ]
    improved = sum(_effect_delta(row) == 1 for row in eligible_records)
    harmed = sum(_effect_delta(row) == -1 for row in eligible_records)
    regressions = sum(
        row.get("comparison", {}).get("regression_introduced") is True
        for row in eligible_records
    )
    unusable = sum(
        row.get("comparison", {}).get("quality_transition") == "unusable"
        for row in eligible_records
    )
    bootstrap = stratified_cluster_paired_bootstrap_interval(
        eligible_records,
        delta=_effect_delta,
        samples=int(design["bootstrap_samples"]),
        seed=int(design["bootstrap_seed"]),
    )
    point = (
        sum(_effect_delta(row) for row in eligible_records) / len(eligible_records)
        if eligible_records
        else None
    )
    resource_summary = _resource_summary(
        executed_records, budgets=design["resource_budgets"]
    )
    full_checks = {
        "frozen_population_complete": (
            observed_pair_ids == eligible_pair_ids | sentinel_pair_ids
            and len(records) == len(eligible_pair_ids) + len(sentinel_pair_ids)
        ),
        "eligible_cluster_design": not design_errors,
        "eligible_pair_role_bound": all(
            row.get("eligibility", {}).get("preregistered", {}).get("eligible")
            is True
            for row in eligible_records
        ),
        "sentinel_pair_role_bound": all(
            row.get("eligibility", {}).get("preregistered", {}).get("eligible")
            is False
            for row in sentinel_records
        ),
        "minimum_executed_recovery_pairs_observed": (
            len(executed_records) >= int(design["minimum_executed_recovery_pairs"])
        ),
        "primary_effect_ci_lower_above_zero": (
            bootstrap.get("status") == "estimated"
            and isinstance(bootstrap.get("lower"), (int, float))
            and bootstrap["lower"] > 0
        ),
        "improvements_exceed_harms": improved > harmed,
        "existing_artifact_harm_zero": harmed == 0,
        "regression_introduced_zero": regressions == 0,
        "unusable_zero": unusable == 0,
        "dependency_sentinel_recovery_zero": all(
            row.get("recovery_one", {}).get("result", {})
            .get("recovery_plan_attempts", {})
            .get("executed_recovery_runs")
            == 0
            for row in sentinel_records
        ),
        "resource_budgets_met": all(
            row["met"] for row in resource_summary["checks"].values()
        )
        and resource_summary["measurement_complete"],
    }
    effect_claim_ready = (
        base["instrument_ready"]
        and base["effect_attribution_ready"]
        and all(full_checks.values())
    )
    return {
        **base,
        "schema_version": "commandagent.goal_verify.recovery_report.v4_a14_full",
        "inference_role": "preregistered fix-intent Recovery 0-vs-1 effect estimate",
        "effect_claim_allowed": (
            design.get("effect_claim_allowed") is True and effect_claim_ready
        ),
        "effect_claim_ready": effect_claim_ready,
        "go_no_go": "GO" if effect_claim_ready else "NO-GO",
        "full_experiment_checks": full_checks,
        "primary_effect": {
            "estimand": design["primary_estimand"],
            "point": round(point, 6) if point is not None else None,
            "improved": improved,
            "harmed": harmed,
            "denominator": len(eligible_records),
            "bootstrap": bootstrap,
        },
        "safety": {
            "existing_artifact_harmed": harmed,
            "regression_introduced": regressions,
            "unusable": unusable,
        },
        "recovery_execution": {
            "eligible_pairs": len(eligible_records),
            "executed_recovery_pairs": len(executed_records),
            "sentinel_pairs": len(sentinel_records),
        },
        "resource_budget_evaluation": resource_summary,
        "full_experiment_diagnostics": {
            "cluster_design_errors": design_errors,
            "missing_pair_ids": sorted(
                (eligible_pair_ids | sentinel_pair_ids) - observed_pair_ids
            ),
            "unexpected_pair_ids": sorted(
                observed_pair_ids - (eligible_pair_ids | sentinel_pair_ids)
            ),
        },
    }


def _effect_delta(record: dict[str, Any]) -> int:
    transition = (record.get("comparison") or {}).get("quality_transition")
    if transition == "improved":
        return 1
    if transition == "harmed":
        return -1
    return 0


def _resource_summary(
    records: list[dict[str, Any]], *, budgets: dict[str, Any]
) -> dict[str, Any]:
    values = {"wall_time_ms": [], "total_tokens": []}
    for record in records:
        resource_delta = (record.get("comparison") or {}).get("resource_delta", {})
        for field in ("wall_time_ms", "total_tokens"):
            value = resource_delta.get(field)
            if isinstance(value, int) and not isinstance(value, bool):
                values[field].append(value)
    measurement_complete = all(
        len(rows) == len(records) for rows in values.values()
    ) and bool(records)
    observed = {
        field: {
            "p50": statistics.median(rows) if rows else None,
            "p95": _percentile(rows, 0.95) if rows else None,
        }
        for field, rows in values.items()
    }
    checks = {}
    for field, percentiles in observed.items():
        for percentile, value in percentiles.items():
            budget = budgets[field][percentile]
            checks[f"{field}_{percentile}"] = {
                "observed": value,
                "maximum": budget,
                "met": value is not None and value <= budget,
            }
    return {
        "population": "executed_recovery_pairs",
        "measurement_complete": measurement_complete,
        "observed": observed,
        "checks": checks,
    }


def _percentile(values: list[int], quantile: float) -> float:
    ordered = sorted(values)
    if len(ordered) == 1:
        return float(ordered[0])
    position = (len(ordered) - 1) * quantile
    lower = math.floor(position)
    upper = math.ceil(position)
    if lower == upper:
        return float(ordered[lower])
    weight = position - lower
    return ordered[lower] * (1 - weight) + ordered[upper] * weight
