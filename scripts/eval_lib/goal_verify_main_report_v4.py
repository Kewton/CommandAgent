from __future__ import annotations

import math
from collections import defaultdict
from typing import Any

from eval_lib.goal_verify_stats_v2 import (
    cluster_paired_bootstrap_interval,
    validate_cluster_design,
)

LANES = ("contract_conformance", "held_out_synthesis")
METRICS = ("required_claim_recall", "strong_binding", "unverified_rate")


def evaluate_main_semantic_review(
    *,
    contract: dict[str, Any],
    blind_report: dict[str, Any] | None,
    semantic_review_complete: bool,
) -> dict[str, Any]:
    sample = contract["semantic_review"]["main_sample"]
    decision = sample["decision_rule"]
    review = (
        blind_report.get("calibration_review", blind_report.get("human_review"))
        if isinstance(blind_report, dict)
        else None
    )
    review = review if isinstance(review, dict) else {}
    verdict_counts = review.get("verdict_counts", {})
    axis_pass_counts = review.get("axis_pass_counts", {})
    expected_count = int(sample["size"])
    unusable_count = verdict_counts.get("unusable")
    false_positive_pass_count = axis_pass_counts.get(
        "false_positive_or_overconstraint_risk_acceptable"
    )
    checks = {
        "complete": semantic_review_complete,
        "review_count": review.get("review_count") == expected_count,
        "unusable_count": isinstance(unusable_count, int)
        and not isinstance(unusable_count, bool)
        and unusable_count <= int(decision["unusable_max_count"]),
        "false_positive_or_overconstraint_risk": isinstance(
            false_positive_pass_count, int
        )
        and not isinstance(false_positive_pass_count, bool)
        and expected_count - false_positive_pass_count
        <= int(decision["false_positive_or_overconstraint_risk_axis_fail_max_count"]),
    }
    return {
        "review_count": review.get("review_count"),
        "verdict_counts": verdict_counts,
        "axis_pass_counts": axis_pass_counts,
        "decision_rule": decision,
        "checks": checks,
        "pass": all(checks.values()),
    }


def build_main_smoke_report(
    *,
    contract: dict[str, Any],
    records: list[dict[str, Any]],
    manifest: dict[str, Any],
) -> dict[str, Any]:
    smoke = contract["smoke"]
    expected_pair_ids = smoke["pair_ids"]
    lanes = [lane for record in records for lane in record.get("lanes", {}).values()]
    valid = [lane for lane in lanes if lane.get("validation", {}).get("valid") is True]
    evaluations = [
        evaluation
        for lane in lanes
        for evaluation in lane.get("execution", {}).get("evaluations", [])
    ]
    executable = [
        evaluation
        for evaluation in evaluations
        if evaluation.get("classification") == "executable"
    ]
    additive = [
        lane.get("additive_comparison")
        for lane in lanes
        if isinstance(lane.get("additive_comparison"), dict)
    ]
    baselines = [record.get("baseline", {}) for record in records]
    primary_rows = [
        _measurement_row(record, contract["main_analysis"]["primary_lane"])
        for record in records
    ]
    resource_report = _resource_report(primary_rows, contract["resource_budgets"])
    cluster_manifest = manifest.get("cluster_design", {})
    expected_manifest_cluster = {
        "cluster_unit": "source_task_id",
        "population_cell_count": int(contract["full_experiment"]["cells"]),
        "population_source_task_count": len(contract["selected_cells"]),
        "runs_per_source_task": int(contract["samples_per_cell"]),
        "population_pair_count": int(
            contract["full_experiment"]["minimum_total_pairs"]
        ),
        "selected_cell_count": int(contract["full_experiment"]["cells"]),
        "selected_source_task_count": len(expected_pair_ids),
        "selected_pair_count": len(expected_pair_ids),
    }
    false_full = sum(
        row.get("shadow_verdict") == "pass"
        and any(
            claim.get("status") != "strong"
            for claim in row.get("combined_score", {}).get("claims", [])
        )
        for row in additive
    )
    checks = {
        "exact_preregistered_pairs": [record.get("pair_id") for record in records]
        == expected_pair_ids,
        "campaign_manifest": manifest.get("campaign_role") == "preregistered_smoke"
        and manifest.get("request_namespace") == smoke["request_namespace"]
        and manifest.get("selected_pair_ids") == expected_pair_ids
        and manifest.get("target_pairs") == len(expected_pair_ids),
        "cluster_manifest": cluster_manifest == expected_manifest_cluster,
        "lane_records_complete": len(lanes) == int(smoke["minimum_lanes"])
        and all(set(record.get("lanes", {})) == set(LANES) for record in records),
        "raw_and_canonical_schema_valid": len(valid) == len(lanes)
        and all(
            lane.get("validation", {}).get("valid_before_host_repairs") is True
            for lane in lanes
        ),
        "host_repairs_zero": all(
            not lane.get("validation", {}).get("host_repairs") for lane in lanes
        ),
        "product_snapshot_recorded": all(
            record.get("snapshot_manifests", {})
            .get("product", {})
            .get("snapshot_sha256")
            for record in records
        ),
        "same_snapshot": all(
            lane.get("execution", {}).get("same_snapshot") is True for lane in lanes
        ),
        "reference_fallback_zero": _lane_sum(records, "reference_fallback_count") == 0,
        "gold_used_for_execution_zero": _lane_sum(
            records, "gold_used_for_execution_count"
        )
        == 0,
        "executable_oracle_attempts_recorded": all(
            evaluation.get("execution_attempt_recorded") is True
            for evaluation in executable
        ),
        "additive_comparison_recorded": len(additive) == len(lanes),
        "baseline_failure_not_overridden": len(additive) == len(lanes)
        and all(row.get("baseline_failure_overridden") is False for row in additive),
        "shadow_false_full_zero": false_full == 0,
        "baseline_task_contract_bound": all(
            baseline.get("completion_contract_bound") is True for baseline in baselines
        ),
        "baseline_run_discovered": all(
            bool(baseline.get("product_run_dir")) for baseline in baselines
        ),
        "baseline_honest_terminal_recorded": all(
            _baseline_honest_terminal(baseline) for baseline in baselines
        ),
        "recovery_plan_auto_runs_zero": all(
            baseline.get("recovery_plan_auto_runs") == 0 for baseline in baselines
        ),
        "resource_measurement_complete": resource_report["measurement_complete"],
    }
    return {
        "schema_version": "commandagent.goal_verify.phase6_main_smoke_report.v4",
        "contract_id": contract["contract_id"],
        "run_id": smoke["run_id"],
        "counts": {
            "records": len(records),
            "lanes": len(lanes),
            "schema_valid": len(valid),
            "candidate_oracles": len(evaluations),
            "executable_candidate_oracles": len(executable),
            "baseline_product_runs": sum(
                bool(baseline.get("product_run_dir")) for baseline in baselines
            ),
            "shadow_false_full": false_full,
        },
        "resource_measurement": resource_report,
        "checks": checks,
        "ready_for_main_collection": all(checks.values()),
        "final_decision": "GO" if all(checks.values()) else "NO-GO",
    }


def build_main_report(
    *,
    contract: dict[str, Any],
    config: dict[str, Any],
    records: list[dict[str, Any]],
    semantic_review_complete: bool,
    semantic_review_evaluation: dict[str, Any] | None = None,
) -> dict[str, Any]:
    full = contract["full_experiment"]
    analysis = contract["main_analysis"]
    primary_lane = analysis["primary_lane"]
    expected_pairs = int(full["minimum_total_pairs"])
    cluster_errors = validate_cluster_design(
        records,
        minimum_clusters_per_cell=int(full["minimum_distinct_source_tasks_per_cell"]),
        minimum_pairs_per_cluster=int(full["minimum_runs_per_source_task"]),
    )
    cell_counts = defaultdict(int)
    for record in records:
        cell_counts[record.get("cell_id")] += 1
    expected_cell_count = int(full["minimum_pairs_per_cell"])
    cell_count_complete = len(cell_counts) == int(full["cells"]) and all(
        count == expected_cell_count for count in cell_counts.values()
    )
    lane_reports = {
        lane: _lane_report(
            records=records,
            lane_name=lane,
            samples=int(analysis["bootstrap_samples"]),
            seed=int(analysis["seed"]),
            config=config,
            resource_budgets=contract["resource_budgets"],
        )
        for lane in LANES
    }
    primary = lane_reports[primary_lane]
    conformance = lane_reports["contract_conformance"]
    improvement = config["improvement_targets"]
    expected_clusters = int(full["cells"]) * int(
        full["minimum_distinct_source_tasks_per_cell"]
    )
    checks = {
        "record_count": len(records) == expected_pairs,
        "cluster_design": not cluster_errors,
        "cluster_count": len({row.get("source_task_id") for row in records})
        == expected_clusters,
        "cell_counts": cell_count_complete,
        "pair_metadata_unique": len({row.get("pair_id") for row in records})
        == len(records),
        "lane_records_complete": all(
            set(record.get("lanes", {})) == set(LANES) for record in records
        ),
        "schema_compliance": primary["schema_compliance_yield"]
        >= float(config["non_inferiority_budgets"]["schema_compliance_yield_floor"]),
        "contract_conformance_schema_compliance": conformance["schema_compliance_yield"]
        >= float(config["non_inferiority_budgets"]["schema_compliance_yield_floor"]),
        "same_snapshot": _valid_lanes_all(
            records,
            lambda lane: lane.get("execution", {}).get("same_snapshot") is True,
        ),
        "reference_fallback_zero": _lane_sum(records, "reference_fallback_count") == 0,
        "gold_used_for_execution_zero": _lane_sum(
            records, "gold_used_for_execution_count"
        )
        == 0,
        "baseline_failure_not_overridden": _all_additive(
            records,
            lambda additive: additive.get("baseline_failure_overridden") is False,
        ),
        "shadow_false_full_zero": primary["shadow_false_full_count"]
        <= int(improvement["false_full_target_count"]),
        "required_claim_recall_gain": _ci_lower_at_least(
            primary["bootstrap"]["required_claim_recall"],
            float(improvement["required_claim_recall_min_gain_pp"]) / 100.0,
        ),
        "strong_binding_gain": _ci_lower_at_least(
            primary["bootstrap"]["strong_binding"],
            float(improvement["strong_binding_coverage_min_gain_pp"]) / 100.0,
        ),
        "unverified_rate_reduction": _ci_upper_at_most(
            primary["bootstrap"]["unverified_rate"],
            -float(improvement["unverified_rate_min_reduction_pp"]) / 100.0,
        ),
        "resource_measurement_complete": primary["resources"]["measurement_complete"],
        "wall_time_p50_budget": primary["resources"]["checks"]["p50_wall_time"],
        "wall_time_p95_budget": primary["resources"]["checks"]["p95_wall_time"],
        "total_tokens_p50_budget": primary["resources"]["checks"]["p50_total_tokens"],
        "total_tokens_p95_budget": primary["resources"]["checks"]["p95_total_tokens"],
        "semantic_review_complete": semantic_review_complete,
        "semantic_review_safety": isinstance(semantic_review_evaluation, dict)
        and semantic_review_evaluation.get("pass") is True,
    }
    return {
        "schema_version": "commandagent.goal_verify.phase6_main_report.v4",
        "contract_id": contract["contract_id"],
        "estimand": contract["comparison"]["estimand"],
        "primary_lane": primary_lane,
        "counts": {
            "records": len(records),
            "expected_records": expected_pairs,
            "cells": dict(sorted(cell_counts.items(), key=lambda item: str(item[0]))),
            "clusters": len({row.get("source_task_id") for row in records}),
        },
        "cluster_design_errors": cluster_errors,
        "lane_reports": lane_reports,
        "semantic_review": semantic_review_evaluation,
        "checks": checks,
        "final_decision": "GO" if all(checks.values()) else "NO-GO",
    }


def _lane_report(
    *,
    records: list[dict[str, Any]],
    lane_name: str,
    samples: int,
    seed: int,
    config: dict[str, Any],
    resource_budgets: dict[str, Any],
) -> dict[str, Any]:
    rows = [_measurement_row(record, lane_name) for record in records]
    valid = sum(row["schema_valid"] for row in rows)
    metadata_complete = all(
        isinstance(row.get("cell_id"), str)
        and row["cell_id"]
        and isinstance(row.get("source_task_id"), str)
        and row["source_task_id"]
        for row in rows
    )
    bootstrap = (
        {
            metric: cluster_paired_bootstrap_interval(
                rows,
                delta=lambda row, field=metric: float(row[field]),
                samples=samples,
                seed=seed + index,
                hierarchical=True,
            )
            for index, metric in enumerate(METRICS)
        }
        if metadata_complete
        else {metric: _insufficient_interval(rows) for metric in METRICS}
    )
    cells: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in rows:
        cells[row["cell_id"]].append(row)
    cell_bootstrap = (
        {
            cell_id: {
                metric: cluster_paired_bootstrap_interval(
                    cell_rows,
                    delta=lambda row, field=metric: float(row[field]),
                    samples=samples,
                    seed=seed + (cell_index * 10) + metric_index,
                    hierarchical=True,
                )
                for metric_index, metric in enumerate(METRICS)
            }
            for cell_index, (cell_id, cell_rows) in enumerate(sorted(cells.items()), 1)
        }
        if metadata_complete
        else {}
    )
    resources = _resource_report(rows, resource_budgets)
    return {
        "pair_count": len(rows),
        "schema_valid_count": valid,
        "schema_compliance_yield": valid / len(rows) if rows else 0.0,
        "point_estimates": {
            metric: round(sum(row[metric] for row in rows) / len(rows), 6)
            if rows
            else None
            for metric in METRICS
        },
        "bootstrap": bootstrap,
        "cell_bootstrap": cell_bootstrap,
        "shadow_false_full_count": sum(row["shadow_false_full"] for row in rows),
        "resources": resources,
        "threshold_source": {
            "improvement_targets": config["improvement_targets"],
            "resource_budgets": resource_budgets,
        },
    }


def _measurement_row(record: dict[str, Any], lane_name: str) -> dict[str, Any]:
    lane = record.get("lanes", {}).get(lane_name, {})
    validation = lane.get("validation", {})
    additive = lane.get("additive_comparison")
    delta = additive.get("paired_delta", {}) if isinstance(additive, dict) else {}
    combined = additive.get("combined_score", {}) if isinstance(additive, dict) else {}
    candidate_wall_ms, candidate_tokens = _candidate_resources(lane)
    baseline_usage = record.get("baseline", {}).get("resource_usage", {})
    baseline_wall_ms = baseline_usage.get("wall_time_ms")
    baseline_tokens = baseline_usage.get("total_tokens")
    return {
        "pair_id": record.get("pair_id"),
        "cell_id": record.get("cell_id"),
        "source_task_id": record.get("source_task_id"),
        "schema_valid": validation.get("valid") is True,
        "required_claim_recall": float(delta.get("required_claim_recall", 0.0)),
        "strong_binding": float(delta.get("strong_binding", 0.0)),
        "unverified_rate": float(delta.get("unverified_rate", 0.0)),
        "shadow_false_full": bool(
            isinstance(additive, dict)
            and additive.get("shadow_verdict") == "pass"
            and any(
                claim.get("status") != "strong" for claim in combined.get("claims", [])
            )
        ),
        "baseline_wall_ms": baseline_wall_ms,
        "candidate_overhead_wall_ms": candidate_wall_ms,
        "baseline_total_tokens": baseline_tokens,
        "candidate_overhead_tokens": candidate_tokens,
    }


def _candidate_resources(lane: dict[str, Any]) -> tuple[float | None, int | None]:
    usage = lane.get("resource_usage")
    if not isinstance(usage, dict):
        return None, None
    wall_time_ms = usage.get("wall_time_ms")
    total_tokens = usage.get("total_tokens")
    if not (
        isinstance(wall_time_ms, (int, float))
        and not isinstance(wall_time_ms, bool)
        and wall_time_ms >= 0
        and isinstance(total_tokens, int)
        and not isinstance(total_tokens, bool)
        and total_tokens >= 0
        and usage.get("token_measurement_complete") is True
    ):
        return None, None
    return float(wall_time_ms), total_tokens


def _resource_report(
    rows: list[dict[str, Any]], budgets: dict[str, Any]
) -> dict[str, Any]:
    complete = all(
        isinstance(row[field], (int, float))
        and not isinstance(row[field], bool)
        and row[field] is not None
        and row[field] > 0
        for row in rows
        for field in (
            "baseline_wall_ms",
            "candidate_overhead_wall_ms",
            "baseline_total_tokens",
            "candidate_overhead_tokens",
        )
    )
    if complete:
        wall_increases = [
            100.0 * row["candidate_overhead_wall_ms"] / row["baseline_wall_ms"]
            for row in rows
        ]
        token_increases = [
            100.0 * row["candidate_overhead_tokens"] / row["baseline_total_tokens"]
            for row in rows
        ]
        percentiles = {
            "p50_wall_time_increase_pct": _percentile(wall_increases, 0.50),
            "p95_wall_time_increase_pct": _percentile(wall_increases, 0.95),
            "p50_total_tokens_increase_pct": _percentile(token_increases, 0.50),
            "p95_total_tokens_increase_pct": _percentile(token_increases, 0.95),
        }
    else:
        percentiles = {
            "p50_wall_time_increase_pct": None,
            "p95_wall_time_increase_pct": None,
            "p50_total_tokens_increase_pct": None,
            "p95_total_tokens_increase_pct": None,
        }
    return {
        "measurement_complete": complete,
        "method": "nearest-rank percentage increase of baseline-plus-candidate over the shared baseline product run",
        "percentiles": percentiles,
        "checks": {
            "p50_wall_time": complete
            and percentiles["p50_wall_time_increase_pct"]
            <= float(budgets["p50_wall_time_max_increase_pct"]),
            "p95_wall_time": complete
            and percentiles["p95_wall_time_increase_pct"]
            <= float(budgets["p95_wall_time_max_increase_pct"]),
            "p50_total_tokens": complete
            and percentiles["p50_total_tokens_increase_pct"]
            <= float(budgets["p50_total_tokens_max_increase_pct"]),
            "p95_total_tokens": complete
            and percentiles["p95_total_tokens_increase_pct"]
            <= float(budgets["p95_total_tokens_max_increase_pct"]),
        },
    }


def _percentile(values: list[float], quantile: float) -> float:
    if not values:
        raise ValueError("percentile requires values")
    ordered = sorted(values)
    index = max(0, math.ceil(quantile * len(ordered)) - 1)
    return round(ordered[index], 6)


def _valid_lanes_all(records, predicate) -> bool:
    valid = [
        lane
        for record in records
        for lane in record.get("lanes", {}).values()
        if lane.get("validation", {}).get("valid") is True
    ]
    return bool(valid) and all(predicate(lane) for lane in valid)


def _lane_sum(records, field: str) -> int:
    return sum(
        int(lane.get("execution", {}).get(field, 0))
        for record in records
        for lane in record.get("lanes", {}).values()
    )


def _all_additive(records, predicate) -> bool:
    additive = [
        lane.get("additive_comparison")
        for record in records
        for lane in record.get("lanes", {}).values()
        if isinstance(lane.get("additive_comparison"), dict)
    ]
    return bool(additive) and all(predicate(row) for row in additive)


def _ci_lower_at_least(interval: dict[str, Any], threshold: float) -> bool:
    return (
        interval.get("status") == "estimated" and float(interval["lower"]) >= threshold
    )


def _ci_upper_at_most(interval: dict[str, Any], threshold: float) -> bool:
    return (
        interval.get("status") == "estimated" and float(interval["upper"]) <= threshold
    )


def _insufficient_interval(rows: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "status": "insufficient_evidence",
        "cluster_count": 0,
        "pair_count": len(rows),
        "lower": None,
        "upper": None,
    }


def _baseline_honest_terminal(row: dict[str, Any]) -> bool:
    if row.get("completion_verify_attempt_recorded") is True:
        return True
    return (
        row.get("status") == "failed"
        and isinstance(row.get("returncode"), int)
        and not isinstance(row.get("returncode"), bool)
        and row["returncode"] != 0
        and bool(row.get("product_run_dir"))
    )
