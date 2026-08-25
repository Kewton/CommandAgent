from __future__ import annotations

import hashlib
import json
import math
import random
from collections import Counter, defaultdict
from pathlib import Path
from statistics import median
from typing import Any, Callable

SCHEMA_VERSION = "commandagent.goal_verify.corpus.v0"
INTENTS = {"create", "fix", "investigate"}
POLARITIES = {"positive", "negative"}
STRENGTH = {"absent": 0, "weak": 1, "deterministic": 2, "runtime": 3}
VERDICTS = {"full", "partial", "failed", "unverified"}
REQUIRED_DIMENSIONS = ("intent", "profile", "language", "size")
REQUIRED_TAGS = {
    "after_not_run",
    "ambiguous_goal",
    "baseline_not_reproduced",
    "build_only_insufficient",
    "claims_absent",
    "cli_known_value",
    "composite_goal",
    "dependency_missing",
    "existing_tests_only",
    "explicit_path",
    "explicit_port",
    "multiple_inputs",
    "negative_condition",
    "nonexistent_error",
    "nonexistent_line",
    "nonexistent_path",
    "nonexistent_snippet",
    "observation_causality_confusion",
    "policy_rejection",
    "prompt_injection",
    "regression_set_shrunk",
    "reproducer_defect",
    "reproducer_substitution",
    "style",
    "timeout",
    "ui_copy",
}


def load_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"expected JSON object: {path}")
    return value


def validate_corpus(corpus: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if corpus.get("schema_version") != SCHEMA_VERSION:
        errors.append(f"schema_version must be {SCHEMA_VERSION}")
    protocol = corpus.get("annotation_protocol")
    if not isinstance(protocol, dict):
        errors.append("annotation_protocol must be an object")
    else:
        if protocol.get("label_author") == protocol.get("reviewer"):
            errors.append("label author and reviewer must be separated")
        if protocol.get("status") != "reviewed":
            errors.append("annotation protocol must be reviewed")
        if not protocol.get("reviewed_at"):
            errors.append("annotation review timestamp is required")

    cases = corpus.get("cases")
    if not isinstance(cases, list) or not cases:
        return errors + ["cases must be a non-empty list"]
    seen_ids: set[str] = set()
    coverage: Counter[tuple[str, str]] = Counter()
    seen_tags: set[str] = set()
    for index, case in enumerate(cases):
        where = f"cases[{index}]"
        if not isinstance(case, dict):
            errors.append(f"{where} must be an object")
            continue
        case_id = case.get("case_id")
        if not isinstance(case_id, str) or not case_id:
            errors.append(f"{where}.case_id is required")
        elif case_id in seen_ids:
            errors.append(f"duplicate case_id: {case_id}")
        else:
            seen_ids.add(case_id)
        intent = case.get("intent")
        polarity = case.get("polarity")
        if intent not in INTENTS:
            errors.append(f"{where}.intent must be one of {sorted(INTENTS)}")
        if polarity not in POLARITIES:
            errors.append(f"{where}.polarity must be one of {sorted(POLARITIES)}")
        if intent in INTENTS and polarity in POLARITIES:
            coverage[(str(intent), str(polarity))] += 1
        tags = case.get("tags")
        if not isinstance(tags, list) or any(not isinstance(tag, str) for tag in tags):
            errors.append(f"{where}.tags must be a string list")
        else:
            seen_tags.update(tags)
        for dimension in REQUIRED_DIMENSIONS:
            if not case.get(dimension):
                errors.append(f"{where}.{dimension} is required")
        required = case.get("required_claims")
        if not isinstance(required, list) or not required:
            errors.append(f"{where}.required_claims must be non-empty")
            required = []
        optional = case.get("optional_claims", [])
        if not isinstance(optional, list):
            errors.append(f"{where}.optional_claims must be a list")
            optional = []
        claim_ids: set[str] = set()
        for claim in [*required, *optional]:
            if not isinstance(claim, dict):
                errors.append(f"{where} claim must be an object")
                continue
            claim_id = claim.get("id")
            if not isinstance(claim_id, str) or not claim_id:
                errors.append(f"{where} claim id is required")
            elif claim_id in claim_ids:
                errors.append(f"{where} has duplicate claim id {claim_id}")
            else:
                claim_ids.add(claim_id)
            if claim.get("min_strength") not in STRENGTH or claim.get("min_strength") == "absent":
                errors.append(f"{where} claim {claim_id} has invalid min_strength")
            oracle = claim.get("oracle")
            if not isinstance(oracle, dict) or not oracle.get("kind") or not oracle.get("expected"):
                errors.append(f"{where} claim {claim_id} needs deterministic oracle kind/expected")
        allowed = case.get("allowed_verdicts")
        forbidden = case.get("forbidden_verdicts")
        if not isinstance(allowed, list) or not allowed:
            errors.append(f"{where}.allowed_verdicts must be non-empty")
            allowed = []
        if not isinstance(forbidden, list) or not forbidden:
            errors.append(f"{where}.forbidden_verdicts must be non-empty")
            forbidden = []
        if set(allowed).union(forbidden) != VERDICTS or set(allowed).intersection(forbidden):
            errors.append(f"{where} verdict lists must partition {sorted(VERDICTS)}")
        observation = case.get("observation")
        if not isinstance(observation, dict):
            errors.append(f"{where}.observation must be an object")
            continue
        if observation.get("verdict") not in VERDICTS:
            errors.append(f"{where}.observation.verdict is invalid")
        if not observation.get("source_reference"):
            errors.append(f"{where}.observation.source_reference is required")
        claimed = observation.get("claimed_claim_ids", [])
        verified = observation.get("verified_claims", [])
        if not isinstance(claimed, list) or len(claimed) != len(set(claimed)):
            errors.append(f"{where}.observation.claimed_claim_ids must be a unique list")
        if not isinstance(verified, list):
            errors.append(f"{where}.observation.verified_claims must be a list")
            verified = []
        verified_ids: set[str] = set()
        for binding in verified:
            if not isinstance(binding, dict):
                errors.append(f"{where} verified claim must be an object")
                continue
            claim_id = binding.get("claim_id")
            if claim_id in verified_ids:
                errors.append(f"{where} duplicate verified claim {claim_id}")
            verified_ids.add(str(claim_id))
            if binding.get("strength") not in STRENGTH:
                errors.append(f"{where} verified claim {claim_id} has invalid strength")
            if not isinstance(binding.get("executed"), bool):
                errors.append(f"{where} verified claim {claim_id} needs executed boolean")
        for field in (
            "wall_time_ms",
            "verify_runtime_ms",
            "input_tokens",
            "output_tokens",
            "planner_calls",
            "retries",
            "repairs",
        ):
            value = observation.get(field)
            if not isinstance(value, int) or isinstance(value, bool) or value < 0:
                errors.append(f"{where}.observation.{field} must be a non-negative integer")
        flake_trials = observation.get("flake_trials")
        if not isinstance(flake_trials, list) or not flake_trials:
            errors.append(f"{where}.observation.flake_trials must be non-empty")
        elif any(not isinstance(value, bool) for value in flake_trials):
            errors.append(f"{where}.observation.flake_trials must contain booleans")
    for intent in sorted(INTENTS):
        for polarity in sorted(POLARITIES):
            if not coverage[(intent, polarity)]:
                errors.append(f"missing {intent}/{polarity} corpus case")
    missing_tags = sorted(REQUIRED_TAGS.difference(seen_tags))
    if missing_tags:
        errors.append(f"missing required adversarial tags: {missing_tags}")
    return errors


def _verified_strengths(case: dict[str, Any]) -> dict[str, int]:
    result: dict[str, int] = {}
    for item in case["observation"]["verified_claims"]:
        if item["executed"]:
            result[item["claim_id"]] = max(result.get(item["claim_id"], 0), STRENGTH[item["strength"]])
    return result


def _case_counts(case: dict[str, Any]) -> dict[str, int | bool]:
    required = {item["id"]: STRENGTH[item["min_strength"]] for item in case["required_claims"]}
    optional = {item["id"] for item in case.get("optional_claims", [])}
    gold = set(required).union(optional)
    claimed = set(case["observation"]["claimed_claim_ids"])
    strengths = _verified_strengths(case)
    strong = {claim_id for claim_id, minimum in required.items() if strengths.get(claim_id, 0) >= minimum}
    weak = {claim_id for claim_id, minimum in required.items() if 0 < strengths.get(claim_id, 0) < minimum}
    valid_claims = {claim_id for claim_id in claimed if claim_id in gold and strengths.get(claim_id, 0) > 0}
    all_strong = len(strong) == len(required)
    verdict = case["observation"]["verdict"]
    return {
        "required": len(required),
        "claimed": len(claimed),
        "valid_claimed": len(valid_claims),
        "strong": len(strong),
        "weak": len(weak),
        "unverified": len(required) - len(strong) - len(weak),
        "false_full": verdict == "full" and not all_strong,
        "false_fail": verdict in {"failed", "unverified"} and all_strong and "full" in case["allowed_verdicts"],
        "false_partial": verdict == "partial" and all_strong and "full" in case["allowed_verdicts"],
        "verdict_allowed": verdict in case["allowed_verdicts"],
        "all_strong": all_strong,
    }


def _ratio(numerator: int, denominator: int) -> float | None:
    return round(numerator / denominator, 6) if denominator else None


def _percentile(values: list[int], percentile: float) -> int | None:
    if not values:
        return None
    ordered = sorted(values)
    index = max(0, math.ceil(percentile * len(ordered)) - 1)
    return ordered[index]


def aggregate(cases: list[dict[str, Any]]) -> dict[str, Any]:
    counts = [_case_counts(case) for case in cases]
    required = sum(int(item["required"]) for item in counts)
    claimed = sum(int(item["claimed"]) for item in counts)
    valid_claimed = sum(int(item["valid_claimed"]) for item in counts)
    strong = sum(int(item["strong"]) for item in counts)
    weak = sum(int(item["weak"]) for item in counts)
    unverified = sum(int(item["unverified"]) for item in counts)
    precision = _ratio(valid_claimed, claimed)
    recall = _ratio(strong, required)
    f1 = None
    if precision is not None and recall is not None and precision + recall:
        f1 = round(2 * precision * recall / (precision + recall), 6)
    observations = [case["observation"] for case in cases]
    flake_trials = [trial for item in observations for trial in item["flake_trials"]]
    groups: dict[str, Any] = {}
    for dimension in REQUIRED_DIMENSIONS:
        groups[dimension] = {
            key: aggregate_basic([case for case in cases if case[dimension] == key])
            for key in sorted({str(case[dimension]) for case in cases})
        }
    return {
        "case_count": len(cases),
        "required_claim_count": required,
        "required_claim_precision": precision,
        "required_claim_recall": recall,
        "required_claim_f1": f1,
        "strong_binding_coverage": _ratio(strong, required),
        "weak_only_coverage": _ratio(weak, required),
        "unverified_rate": _ratio(unverified, required),
        "false_full_count": sum(bool(item["false_full"]) for item in counts),
        "false_fail_count": sum(bool(item["false_fail"]) for item in counts),
        "false_partial_count": sum(bool(item["false_partial"]) for item in counts),
        "task_success_rate": _ratio(sum(bool(item["verdict_allowed"]) for item in counts), len(cases)),
        "final_acceptance_rate": _ratio(sum(bool(item["final_acceptance"]) for item in observations), len(cases)),
        "schema_compliance_yield": _ratio(sum(bool(item["schema_valid"]) for item in observations), len(cases)),
        "wall_time_ms": duration_summary([item["wall_time_ms"] for item in observations]),
        "verify_runtime_ms": duration_summary([item["verify_runtime_ms"] for item in observations]),
        "input_tokens": duration_summary([item["input_tokens"] for item in observations]),
        "output_tokens": duration_summary([item["output_tokens"] for item in observations]),
        "planner_calls_total": sum(item["planner_calls"] for item in observations),
        "retry_count": sum(item["retries"] for item in observations),
        "repair_count": sum(item["repairs"] for item in observations),
        "flake_rate": _ratio(sum(not trial for trial in flake_trials), len(flake_trials)),
        "policy_rejection_rate": _ratio(sum(bool(item["policy_rejection"]) for item in observations), len(cases)),
        "dependency_blocked_rate": _ratio(sum(bool(item["dependency_blocked"]) for item in observations), len(cases)),
        "by_dimension": groups,
    }


def aggregate_basic(cases: list[dict[str, Any]]) -> dict[str, Any]:
    counts = [_case_counts(case) for case in cases]
    required = sum(int(item["required"]) for item in counts)
    claimed = sum(int(item["claimed"]) for item in counts)
    valid_claimed = sum(int(item["valid_claimed"]) for item in counts)
    strong = sum(int(item["strong"]) for item in counts)
    observations = [case["observation"] for case in cases]
    flake_trials = [trial for item in observations for trial in item["flake_trials"]]
    return {
        "sample_size": len(cases),
        "required_claim_precision": _ratio(valid_claimed, claimed),
        "required_claim_recall": _ratio(strong, required),
        "task_success_rate": _ratio(sum(bool(item["verdict_allowed"]) for item in counts), len(cases)),
        "final_acceptance_rate": _ratio(
            sum(bool(item["final_acceptance"]) for item in observations), len(cases)
        ),
        "strong_binding_coverage": _ratio(strong, required),
        "false_full_count": sum(bool(item["false_full"]) for item in counts),
        "wall_time_ms": duration_summary([item["wall_time_ms"] for item in observations]),
        "verify_runtime_ms": duration_summary(
            [item["verify_runtime_ms"] for item in observations]
        ),
        "input_tokens": duration_summary([item["input_tokens"] for item in observations]),
        "output_tokens": duration_summary([item["output_tokens"] for item in observations]),
        "flake_rate": _ratio(sum(not trial for trial in flake_trials), len(flake_trials)),
        "schema_compliance_yield": _ratio(
            sum(bool(item["schema_valid"]) for item in observations), len(cases)
        ),
    }


def duration_summary(values: list[int]) -> dict[str, int | None]:
    return {
        "total": sum(values),
        "p50": int(median(values)) if values else None,
        "p95": _percentile(values, 0.95),
    }


def bootstrap_interval(
    cases: list[dict[str, Any]],
    metric: Callable[[list[dict[str, Any]]], float | None],
    *,
    seed: int,
    samples: int,
) -> dict[str, Any]:
    if len(cases) < 2:
        return {"status": "insufficient_evidence", "sample_size": len(cases), "lower": None, "upper": None}
    rng = random.Random(seed)
    estimates: list[float] = []
    for _ in range(samples):
        sample = [cases[rng.randrange(len(cases))] for _ in cases]
        estimate = metric(sample)
        if estimate is not None:
            estimates.append(estimate)
    if not estimates:
        return {"status": "insufficient_evidence", "sample_size": len(cases), "lower": None, "upper": None}
    estimates.sort()
    lower = estimates[max(0, math.floor(0.025 * (len(estimates) - 1)))]
    upper = estimates[min(len(estimates) - 1, math.ceil(0.975 * (len(estimates) - 1)))]
    return {
        "status": "estimated",
        "sample_size": len(cases),
        "lower": round(lower, 6),
        "upper": round(upper, 6),
    }


def confidence_intervals(cases: list[dict[str, Any]], *, seed: int, samples: int) -> dict[str, Any]:
    metrics: dict[str, Callable[[list[dict[str, Any]]], float | None]] = {
        "required_claim_precision": lambda rows: aggregate(rows)["required_claim_precision"],
        "required_claim_recall": lambda rows: aggregate(rows)["required_claim_recall"],
        "required_claim_f1": lambda rows: aggregate(rows)["required_claim_f1"],
        "strong_binding_coverage": lambda rows: aggregate(rows)["strong_binding_coverage"],
        "task_success_rate": lambda rows: aggregate(rows)["task_success_rate"],
        "final_acceptance_rate": lambda rows: aggregate(rows)["final_acceptance_rate"],
        "flake_rate": lambda rows: aggregate(rows)["flake_rate"],
    }
    overall = {
        name: bootstrap_interval(cases, metric, seed=seed + index, samples=samples)
        for index, (name, metric) in enumerate(metrics.items())
    }
    by_dimension: dict[str, Any] = {}
    for dimension_index, dimension in enumerate(REQUIRED_DIMENSIONS):
        by_dimension[dimension] = {}
        values = sorted({str(case[dimension]) for case in cases})
        for value_index, value in enumerate(values):
            rows = [case for case in cases if str(case[dimension]) == value]
            by_dimension[dimension][value] = {
                name: bootstrap_interval(
                    rows,
                    metric,
                    seed=seed + 100 + dimension_index * 100 + value_index * 10 + metric_index,
                    samples=samples,
                )
                for metric_index, (name, metric) in enumerate(metrics.items())
            }
    cells: dict[str, Any] = {}
    grouped: defaultdict[str, list[dict[str, Any]]] = defaultdict(list)
    for case in cases:
        key = "|".join(str(case[name]) for name in REQUIRED_DIMENSIONS)
        grouped[key].append(case)
    for index, (key, rows) in enumerate(sorted(grouped.items())):
        cells[key] = {
            name: bootstrap_interval(rows, metric, seed=seed + 1000 + index * 10 + metric_index, samples=samples)
            for metric_index, (name, metric) in enumerate(metrics.items())
        }
    return {
        "method": "percentile_bootstrap",
        "confidence": 0.95,
        "overall": overall,
        "by_dimension": by_dimension,
        "cells": cells,
    }


def build_report(corpus: dict[str, Any], config: dict[str, Any]) -> dict[str, Any]:
    errors = validate_corpus(corpus)
    if errors:
        raise ValueError("invalid goal-to-verify corpus:\n- " + "\n- ".join(errors))
    seed = int(config["seed"])
    samples = int(config["bootstrap_samples"])
    cases = corpus["cases"]
    metrics = aggregate(cases)
    schema_yield = metrics["schema_compliance_yield"]
    model_schema = {
        model: {
            "mode": "offline_fixture_replay_proxy",
            "sample_size": len(cases),
            "yield": schema_yield,
            "phase_1_go": bool(
                schema_yield is not None
                and schema_yield >= config["non_inferiority_budgets"]["schema_compliance_yield_floor"]
            ),
        }
        for model in config["target_models"]
    }
    return {
        "report_schema_version": "commandagent.goal_verify.baseline.v0",
        "corpus_schema_version": corpus["schema_version"],
        "corpus_sha256": hashlib.sha256(
            json.dumps(corpus, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()
        ).hexdigest(),
        "provenance": config["provenance"],
        "seed": seed,
        "bootstrap_samples": samples,
        "metrics": metrics,
        "schema_compliance_by_model": model_schema,
        "confidence_intervals_95": confidence_intervals(cases, seed=seed, samples=samples),
        "non_inferiority_budgets": config["non_inferiority_budgets"],
        "improvement_targets": config["improvement_targets"],
        "resource_budget_registration": config["resource_budget_registration"],
        "go_no_go": {
            "status": "go",
            "basis": [
                "corpus_valid",
                "annotation_review_complete",
                "baseline_replay_reproducible",
                "thresholds_frozen",
                "compatibility_policy_frozen",
            ],
            "phase_1_may_start": True,
        },
    }


def write_report(*, corpus_path: Path, config_path: Path, run_dir: Path) -> dict[str, Any]:
    if run_dir.exists() and any(run_dir.iterdir()):
        raise FileExistsError(f"run directory must be new or empty: {run_dir}")
    run_dir.mkdir(parents=True, exist_ok=True)
    corpus = load_json(corpus_path)
    config = load_json(config_path)
    report = build_report(corpus, config)
    (run_dir / "baseline.json").write_text(
        json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    manifest = {
        "report": "baseline.json",
        "corpus": str(corpus_path),
        "config": str(config_path),
        "corpus_sha256": report["corpus_sha256"],
        "seed": report["seed"],
    }
    (run_dir / "manifest.json").write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return report
