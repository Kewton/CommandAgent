#!/usr/bin/env python3
"""Curate the CM-3b B-prime/D-prime calibration observations."""
from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
from typing import Any

import cm3_matrix


def classify_terminal(reason: str, product_exit: int | None) -> str | None:
    if product_exit == 0:
        return None
    if "community_spec_closed_vocabulary" in reason:
        return "community_spec_closed_vocabulary"
    if "community_esbuild_script_missing" in reason:
        return "community_esbuild_script_missing"
    if "dependency_setup_authority_required" in reason:
        return "community_verify_instruction_not_executable"
    if "path does not exist: schema/app-spec.schema.sha256sums" in reason:
        return "community_schema_pin_path_invented"
    if "path does not exist: app.spec.yaml" in reason:
        return "community_spec_artifact_missing"
    if "path does not exist: package.json" in reason:
        return "community_package_artifact_missing"
    return "unclassified_terminal"


def live_rows(label: str, meta_path: Path, pricing: Path) -> list[dict[str, Any]]:
    metadata = json.loads(meta_path.read_text(encoding="utf-8"))
    records = {record["name"]: record for record in metadata["runs"]}
    rows = cm3_matrix.live_observations(label, meta_path, pricing)
    for row in rows:
        record = records[row["id"]]
        row["source"] = "cm3-matrix-002 live campaign"
        row["stop_class"] = classify_terminal(
            str(record.get("terminal_reason") or ""), record.get("product_exit")
        )
        row["outcome_signature"] = row["stop_class"] or "full"
    return rows


def newcombe_difference(
    new_successes: int,
    new_trials: int,
    old_successes: int,
    old_trials: int,
) -> list[float]:
    """Newcombe hybrid-score 95% CI for two independent proportions."""
    new_rate = new_successes / new_trials
    old_rate = old_successes / old_trials
    new_lower, new_upper = cm3_matrix.wilson(new_successes, new_trials)
    old_lower, old_upper = cm3_matrix.wilson(old_successes, old_trials)
    difference = new_rate - old_rate
    lower = difference - math.sqrt(
        (new_rate - new_lower) ** 2 + (old_upper - old_rate) ** 2
    )
    upper = difference + math.sqrt(
        (new_upper - new_rate) ** 2 + (old_rate - old_lower) ** 2
    )
    return [max(-1.0, lower), min(1.0, upper)]


def comparison(old: dict[str, Any], new: dict[str, Any]) -> dict[str, Any]:
    full_delta = new["full_rate"] - old["full_rate"]
    one_shot_delta = new["one_shot_full_rate"] - old["one_shot_full_rate"]
    return {
        "full_rate_difference": full_delta,
        "full_rate_difference_newcombe_95": newcombe_difference(
            new["full"], new["n"], old["full"], old["n"]
        ),
        "one_shot_full_rate_difference": one_shot_delta,
        "one_shot_full_rate_difference_newcombe_95": newcombe_difference(
            new["one_shot_full"],
            new["n"],
            old["one_shot_full"],
            old["n"],
        ),
        "duration_p50_difference_secs": (
            new["duration_secs"]["p50"] - old["duration_secs"]["p50"]
        ),
        "duration_p95_difference_secs": (
            new["duration_secs"]["p95"] - old["duration_secs"]["p95"]
        ),
        "cost_total_difference_usd": (
            new["cost_usd"]["total"] - old["cost_usd"]["total"]
        ),
    }


def d_prime_decision(summary: dict[str, Any]) -> dict[str, Any]:
    high_full = summary["full_rate"] >= 0.9
    thirty_seconds = summary["duration_secs"]["p50"] <= 30.0
    return {
        "rule": "repair-included full >= 90% AND duration p50 <= 30 seconds",
        "high_full_pass": high_full,
        "p50_pass": thirty_seconds,
        "established": high_full and thirty_seconds,
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--matrix-001", type=Path, required=True)
    parser.add_argument("--arm-b-prime-meta", type=Path, required=True)
    parser.add_argument("--arm-d-prime-meta", type=Path, required=True)
    parser.add_argument("--pricing", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    old = json.loads(args.matrix_001.read_text(encoding="utf-8"))
    rows = live_rows("B-prime", args.arm_b_prime_meta, args.pricing)
    rows.extend(live_rows("D-prime", args.arm_d_prime_meta, args.pricing))
    arms = {
        label: cm3_matrix.summarize_arm(
            [row for row in rows if row["arm"] == label]
        )
        for label in ("B-prime", "D-prime")
    }
    document = {
        "schema_version": "commandagent.cm3b-matrix-summary/v1",
        "series_id": "cm3-matrix-002",
        "denominator": 24,
        "budget_ceiling_usd": 3.0,
        "execution_revision": "6352bbdbc46577d905d4c4a88c40b3fb00587615",
        "binary_sha256": "d11b216d09d7b483becad1317c200b45477708d357838fe21fb2ad48299eeded",
        "live_cost_usd": sum(row["cost_usd"] for row in rows),
        "arms": arms,
        "comparisons": {
            "B_to_B_prime": comparison(old["arms"]["B"], arms["B-prime"]),
            "D_to_D_prime": comparison(old["arms"]["D"], arms["D-prime"]),
        },
        "d_prime_30_second_high_full": d_prime_decision(arms["D-prime"]),
        "runs": rows,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(document, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )


if __name__ == "__main__":
    main()
