from __future__ import annotations

import copy
import json
import math
import random
from pathlib import Path
from typing import Any, Callable

from eval_lib.goal_verify_baseline import aggregate, load_json, validate_corpus

SCHEMA_VERSION = "commandagent.goal_verify.phase6_matrix.v0"
LANES = ("blind_review", "ci", "offline_local", "approved_live")
DIMENSIONS = ("intent", "profile", "language", "size")
FINAL_DECISIONS = {"GO", "NO-GO", "INSUFFICIENT-EVIDENCE"}


def _repo_path(root: Path, value: str) -> Path:
    path = Path(value)
    return path if path.is_absolute() else root / path


def _validate_reference(root: Path, value: Any, where: str, errors: list[str]) -> None:
    if not isinstance(value, str) or not value:
        errors.append(f"{where} must be a non-empty path")
    elif not _repo_path(root, value).is_file():
        errors.append(f"{where} does not exist: {value}")


def validate_manifest(manifest: dict[str, Any], root: Path) -> list[str]:
    errors: list[str] = []
    if manifest.get("schema_version") != SCHEMA_VERSION:
        errors.append(f"schema_version must be {SCHEMA_VERSION}")
    if manifest.get("dimensions") != list(DIMENSIONS):
        errors.append(f"dimensions must be {list(DIMENSIONS)}")
    minimum = manifest.get("minimum_samples_per_cell")
    if not isinstance(minimum, int) or isinstance(minimum, bool) or minimum < 2:
        errors.append("minimum_samples_per_cell must be an integer >= 2")
    cells = manifest.get("cells")
    if not isinstance(cells, list) or not cells:
        errors.append("cells must be a non-empty list")
    else:
        keys: set[tuple[str, ...]] = set()
        for index, cell in enumerate(cells):
            if not isinstance(cell, dict) or any(not cell.get(name) for name in DIMENSIONS):
                errors.append(f"cells[{index}] must define every dimension")
                continue
            key = tuple(str(cell[name]) for name in DIMENSIONS)
            if key in keys:
                errors.append(f"duplicate matrix cell: {'|'.join(key)}")
            keys.add(key)
    for side in ("baseline", "candidate"):
        value = manifest.get(side)
        if not isinstance(value, dict) or not value.get("label"):
            errors.append(f"{side} must define a label")
            continue
        corpus = value.get("corpus")
        report = value.get("report")
        if side == "baseline" or corpus is not None:
            _validate_reference(root, corpus, f"{side}.corpus", errors)
        if side == "baseline" or report is not None:
            _validate_reference(root, report, f"{side}.report", errors)
        if side == "candidate" and corpus is None and not value.get("absence_reason"):
            errors.append("candidate.absence_reason is required when candidate corpus is absent")
    lanes = manifest.get("evidence_lanes")
    if not isinstance(lanes, dict) or set(lanes) != set(LANES):
        errors.append(f"evidence_lanes must contain exactly {list(LANES)}")
    else:
        for lane_name in LANES:
            lane = lanes[lane_name]
            if not isinstance(lane, dict) or not isinstance(lane.get("required"), bool):
                errors.append(f"evidence_lanes.{lane_name}.required must be boolean")
                continue
            if lane.get("status") not in {"available", "partial", "missing", "not_authorized"}:
                errors.append(f"evidence_lanes.{lane_name}.status is invalid")
            references = lane.get("references")
            if not isinstance(references, list):
                errors.append(f"evidence_lanes.{lane_name}.references must be a list")
            else:
                for index, reference in enumerate(references):
                    _validate_reference(
                        root, reference, f"evidence_lanes.{lane_name}.references[{index}]", errors
                    )
            if lane_name == "approved_live":
                if not isinstance(lane.get("authorized"), bool):
                    errors.append("approved_live.authorized must be boolean")
                elif lane["authorized"] and (
                    lane.get("status") != "available" or not references
                ):
                    errors.append("authorized live evidence must be available and referenced")
    for field, status_field in (("rollback", "rehearsed"), ("flag_off_compatibility", "verified")):
        value = manifest.get(field)
        if not isinstance(value, dict) or not isinstance(value.get(status_field), bool):
            errors.append(f"{field}.{status_field} must be boolean")
            continue
        references = value.get("references")
        if not isinstance(references, list) or not references:
            errors.append(f"{field}.references must be non-empty")
        else:
            for index, reference in enumerate(references):
                _validate_reference(root, reference, f"{field}.references[{index}]", errors)
    return errors


def _metric(metrics: dict[str, Any], name: str) -> float | int | None:
    if name == "false_fail_rate":
        return metrics["false_fail_count"] / metrics["case_count"]
    if name == "false_partial_rate":
        return metrics["false_partial_count"] / metrics["case_count"]
    if name == "total_tokens_p50":
        return metrics["input_tokens"]["p50"] + metrics["output_tokens"]["p50"]
    if name == "total_tokens_p95":
        return metrics["input_tokens"]["p95"] + metrics["output_tokens"]["p95"]
    if name == "wall_time_p50":
        return metrics["wall_time_ms"]["p50"]
    if name == "wall_time_p95":
        return metrics["wall_time_ms"]["p95"]
    return metrics.get(name)


def _paired_interval(
    baseline_cases: list[dict[str, Any]],
    candidate_cases: list[dict[str, Any]],
    getter: Callable[[dict[str, Any]], float | int | None],
    *,
    seed: int,
    samples: int,
) -> dict[str, Any]:
    candidate_by_id = {case["case_id"]: case for case in candidate_cases}
    pairs = [(case, candidate_by_id[case["case_id"]]) for case in baseline_cases if case["case_id"] in candidate_by_id]
    if len(pairs) < 2:
        return {"status": "insufficient_evidence", "paired_sample_size": len(pairs), "lower": None, "upper": None}
    rng = random.Random(seed)
    deltas: list[float] = []
    for _ in range(samples):
        sampled = [pairs[rng.randrange(len(pairs))] for _ in pairs]
        baseline = getter(aggregate([pair[0] for pair in sampled]))
        candidate = getter(aggregate([pair[1] for pair in sampled]))
        if baseline is not None and candidate is not None:
            deltas.append(float(candidate) - float(baseline))
    deltas.sort()
    if not deltas:
        return {"status": "insufficient_evidence", "paired_sample_size": len(pairs), "lower": None, "upper": None}
    lower_index = max(0, math.floor(0.025 * (len(deltas) - 1)))
    upper_index = min(len(deltas) - 1, math.ceil(0.975 * (len(deltas) - 1)))
    return {
        "status": "estimated",
        "paired_sample_size": len(pairs),
        "lower": round(deltas[lower_index], 6),
        "upper": round(deltas[upper_index], 6),
    }


def _candidate_interval(
    candidate_cases: list[dict[str, Any]],
    getter: Callable[[dict[str, Any]], float | int | None],
    *,
    seed: int,
    samples: int,
) -> dict[str, Any]:
    if len(candidate_cases) < 2:
        return {
            "status": "insufficient_evidence",
            "sample_size": len(candidate_cases),
            "lower": None,
            "upper": None,
        }
    rng = random.Random(seed)
    estimates: list[float] = []
    for _ in range(samples):
        sampled = [candidate_cases[rng.randrange(len(candidate_cases))] for _ in candidate_cases]
        estimate = getter(aggregate(sampled))
        if estimate is not None:
            estimates.append(float(estimate))
    estimates.sort()
    lower_index = max(0, math.floor(0.025 * (len(estimates) - 1)))
    upper_index = min(len(estimates) - 1, math.ceil(0.975 * (len(estimates) - 1)))
    return {
        "status": "estimated",
        "sample_size": len(candidate_cases),
        "lower": round(estimates[lower_index], 6),
        "upper": round(estimates[upper_index], 6),
    }


def _indicator_specs(config: dict[str, Any]) -> list[dict[str, Any]]:
    budgets = config["non_inferiority_budgets"]
    improvements = config["improvement_targets"]
    resources = config["resource_budget_registration"]
    resource_values = resources.get("values", {})
    return [
        {"name": "required_claim_recall", "rule": "delta_min", "threshold": -budgets["required_claim_recall_max_drop_pp"] / 100},
        {"name": "required_claim_precision", "rule": "delta_min", "threshold": -budgets["required_claim_precision_max_drop_pp"] / 100},
        {"name": "strong_binding_coverage", "rule": "delta_min", "threshold": -budgets["strong_binding_coverage_max_drop_pp"] / 100},
        {"name": "task_success_rate", "rule": "delta_min", "threshold": -budgets["task_success_max_drop_pp"] / 100},
        {"name": "final_acceptance_rate", "rule": "delta_min", "threshold": -budgets["final_acceptance_max_drop_pp"] / 100},
        {"name": "false_full_count", "rule": "delta_max", "threshold": budgets["false_full_max_increase_count"]},
        {"name": "false_fail_rate", "rule": "delta_max", "threshold": budgets["false_fail_max_increase_pp"] / 100},
        {"name": "false_partial_rate", "rule": "delta_max", "threshold": budgets["false_partial_max_increase_pp"] / 100},
        {"name": "flake_rate", "rule": "delta_max", "threshold": budgets["flake_max_increase_pp"] / 100},
        {"name": "schema_compliance_yield", "rule": "candidate_min", "threshold": budgets["schema_compliance_yield_floor"]},
        {"name": "required_claim_recall_gain", "source": "required_claim_recall", "rule": "delta_min", "threshold": improvements["required_claim_recall_min_gain_pp"] / 100},
        {"name": "strong_binding_coverage_gain", "source": "strong_binding_coverage", "rule": "delta_min", "threshold": improvements["strong_binding_coverage_min_gain_pp"] / 100},
        {"name": "unverified_rate_reduction", "source": "unverified_rate", "rule": "delta_max", "threshold": -improvements["unverified_rate_min_reduction_pp"] / 100},
        {"name": "false_full_target", "source": "false_full_count", "rule": "candidate_max", "threshold": improvements["false_full_target_count"]},
        {"name": "wall_time_p50", "rule": "delta_pct_max", "threshold": resource_values.get("p50_wall_time_max_increase_pct")},
        {"name": "wall_time_p95", "rule": "delta_pct_max", "threshold": resource_values.get("p95_wall_time_max_increase_pct")},
        {"name": "total_tokens_p50", "rule": "delta_pct_max", "threshold": resource_values.get("p50_total_tokens_max_increase_pct")},
        {"name": "total_tokens_p95", "rule": "delta_pct_max", "threshold": resource_values.get("p95_total_tokens_max_increase_pct")},
    ]


def _verdict(
    rule: str,
    candidate: float | int,
    delta: float,
    threshold: float | int,
    interval: dict[str, Any],
) -> str:
    if interval["status"] != "estimated":
        return "insufficient_evidence"
    value = delta
    if rule == "candidate_min":
        value = interval["lower"]
        return "passed" if value >= threshold else "failed"
    if rule == "candidate_max":
        value = interval["upper"]
        return "passed" if value <= threshold else "failed"
    if rule == "delta_pct_max":
        return "passed" if interval["upper"] <= threshold else "failed"
    if rule == "delta_min":
        return "passed" if interval["lower"] >= threshold else "failed"
    return "passed" if interval["upper"] <= threshold else "failed"


def build_phase6_report(manifest: dict[str, Any], config: dict[str, Any], root: Path) -> dict[str, Any]:
    errors = validate_manifest(manifest, root)
    if errors:
        raise ValueError("invalid Phase 6 matrix:\n- " + "\n- ".join(errors))
    baseline_corpus = load_json(_repo_path(root, manifest["baseline"]["corpus"]))
    corpus_errors = validate_corpus(baseline_corpus)
    if corpus_errors:
        raise ValueError("invalid baseline corpus:\n- " + "\n- ".join(corpus_errors))
    candidate_path = manifest["candidate"]["corpus"]
    candidate_corpus = load_json(_repo_path(root, candidate_path)) if candidate_path else None
    if candidate_corpus is not None:
        candidate_errors = validate_corpus(candidate_corpus)
        if candidate_errors:
            raise ValueError("invalid candidate corpus:\n- " + "\n- ".join(candidate_errors))
    baseline_cases = baseline_corpus["cases"]
    candidate_cases = candidate_corpus["cases"] if candidate_corpus else []
    baseline_metrics = aggregate(baseline_cases)
    candidate_metrics = aggregate(candidate_cases) if candidate_cases else None
    indicators: list[dict[str, Any]] = []
    for index, spec in enumerate(_indicator_specs(config)):
        source = spec.get("source", spec["name"])
        baseline = _metric(baseline_metrics, source)
        candidate = _metric(candidate_metrics, source) if candidate_metrics else None
        delta = None if baseline is None or candidate is None else round(float(candidate) - float(baseline), 6)
        if spec["rule"] == "delta_pct_max" and delta is not None:
            delta = None if baseline == 0 else round(delta / float(baseline) * 100, 6)
        interval = _paired_interval(
            baseline_cases,
            candidate_cases,
            lambda metrics, metric=source: _metric(metrics, metric),
            seed=int(config["seed"]) + index,
            samples=int(config["bootstrap_samples"]),
        )
        if spec["rule"] in {"candidate_min", "candidate_max"}:
            interval = _candidate_interval(
                candidate_cases,
                lambda metrics, metric=source: _metric(metrics, metric),
                seed=int(config["seed"]) + index,
                samples=int(config["bootstrap_samples"]),
            )
        if spec["rule"] == "delta_pct_max" and interval["lower"] is not None:
            if baseline == 0:
                interval = {"status": "insufficient_evidence", "paired_sample_size": interval["paired_sample_size"], "lower": None, "upper": None}
            else:
                interval["lower"] = round(interval["lower"] / float(baseline) * 100, 6)
                interval["upper"] = round(interval["upper"] / float(baseline) * 100, 6)
        verdict = "insufficient_evidence"
        if candidate is not None and delta is not None and spec["threshold"] is not None:
            verdict = _verdict(spec["rule"], candidate, delta, spec["threshold"], interval)
        indicators.append(
            {
                "name": spec["name"],
                "baseline": baseline,
                "candidate": candidate,
                "delta": delta,
                "confidence_interval_95": interval,
                "threshold": {"rule": spec["rule"], "value": spec["threshold"]},
                "verdict": verdict,
            }
        )
    target_cells = {tuple(str(cell[name]) for name in DIMENSIONS) for cell in manifest["cells"]}
    candidate_counts = {key: 0 for key in target_cells}
    baseline_counts = {key: 0 for key in target_cells}
    for case in baseline_cases:
        key = tuple(str(case[name]) for name in DIMENSIONS)
        if key in baseline_counts:
            baseline_counts[key] += 1
    for case in candidate_cases:
        key = tuple(str(case[name]) for name in DIMENSIONS)
        if key in candidate_counts:
            candidate_counts[key] += 1
    cell_results = [
        {
            **dict(zip(DIMENSIONS, key)),
            "baseline_samples": baseline_counts[key],
            "candidate_samples": candidate_counts[key],
            "minimum_samples": manifest["minimum_samples_per_cell"],
            "verdict": "passed"
            if min(baseline_counts[key], candidate_counts[key])
            >= manifest["minimum_samples_per_cell"]
            else "insufficient_evidence",
        }
        for key in sorted(target_cells)
    ]
    failures: list[dict[str, str]] = []
    for name in LANES:
        lane = manifest["evidence_lanes"][name]
        if lane["required"] and lane["status"] != "available":
            failures.append({"scope": f"evidence_lane:{name}", "reason": lane["status"]})
    for cell in cell_results:
        if cell["verdict"] != "passed":
            key = "|".join(str(cell[name]) for name in DIMENSIONS)
            failures.append(
                {
                    "scope": f"matrix_cell:{key}",
                    "reason": (
                        f"baseline_samples={cell['baseline_samples']}, "
                        f"candidate_samples={cell['candidate_samples']} below "
                        f"{cell['minimum_samples']}"
                    ),
                }
            )
    for indicator in indicators:
        if indicator["verdict"] != "passed":
            failures.append({"scope": f"indicator:{indicator['name']}", "reason": indicator["verdict"]})
    if not manifest["rollback"]["rehearsed"]:
        failures.append({"scope": "rollback", "reason": "not_rehearsed"})
    if not manifest["flag_off_compatibility"]["verified"]:
        failures.append({"scope": "flag_off_compatibility", "reason": "not_verified"})
    failed = any(indicator["verdict"] == "failed" for indicator in indicators)
    final_decision = "NO-GO" if failed else ("GO" if not failures else "INSUFFICIENT-EVIDENCE")
    assert final_decision in FINAL_DECISIONS
    return {
        "report_schema_version": "commandagent.goal_verify.phase6_report.v0",
        "matrix_id": manifest["matrix_id"],
        "baseline": copy.deepcopy(manifest["baseline"]),
        "candidate": copy.deepcopy(manifest["candidate"]),
        "evidence_lanes": copy.deepcopy(manifest["evidence_lanes"]),
        "indicators": indicators,
        "matrix_cells": cell_results,
        "rollback": copy.deepcopy(manifest["rollback"]),
        "flag_off_compatibility": copy.deepcopy(manifest["flag_off_compatibility"]),
        "failure_cases": failures,
        "final_decision": final_decision,
    }


def write_phase6_report(*, manifest_path: Path, config_path: Path, run_dir: Path, root: Path) -> dict[str, Any]:
    if run_dir.exists() and any(run_dir.iterdir()):
        raise FileExistsError(f"run directory must be new or empty: {run_dir}")
    run_dir.mkdir(parents=True, exist_ok=True)
    report = build_phase6_report(load_json(manifest_path), load_json(config_path), root)
    (run_dir / "phase6-report.json").write_text(
        json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    (run_dir / "failure-cases.json").write_text(
        json.dumps(report["failure_cases"], ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return report
