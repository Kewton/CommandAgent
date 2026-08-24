#!/usr/bin/env python3
"""Curate CM-3 arm observations from immutable campaign evidence."""
from __future__ import annotations

import argparse
import json
import math
import statistics
from collections import Counter
from pathlib import Path
from statistics import NormalDist
from typing import Any

import community_cost


def wilson(successes: int, trials: int, confidence: float = 0.95) -> list[float]:
    z = NormalDist().inv_cdf(0.5 + confidence / 2.0)
    estimate = successes / trials
    denominator = 1.0 + z * z / trials
    center = (estimate + z * z / (2.0 * trials)) / denominator
    half_width = (
        z
        * math.sqrt(
            estimate * (1.0 - estimate) / trials
            + z * z / (4.0 * trials * trials)
        )
        / denominator
    )
    return [max(0.0, center - half_width), min(1.0, center + half_width)]


def percentile(values: list[float], probability: float) -> float:
    ordered = sorted(values)
    position = (len(ordered) - 1) * probability
    lower = math.floor(position)
    upper = math.ceil(position)
    if lower == upper:
        return ordered[lower]
    return ordered[lower] + (ordered[upper] - ordered[lower]) * (position - lower)


def distribution(values: list[float]) -> dict[str, float]:
    return {
        "min": min(values),
        "q1": percentile(values, 0.25),
        "p50": statistics.median(values),
        "q3": percentile(values, 0.75),
        "p95": percentile(values, 0.95),
        "max": max(values),
    }


def event_rows(artifact: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for path in sorted(artifact.rglob("events.jsonl")):
        for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
            rows.append(json.loads(line))
    return rows


def classify_stop(record: dict[str, Any], events: list[dict[str, Any]]) -> str | None:
    if record.get("product_exit") == 0:
        return None
    reasons = [
        str(event.get("stop_reason") or event.get("reason") or "")
        for event in events
        if (event.get("event") == "run_stop" and event.get("ok") is False)
        or event.get("event") == "ultra_phase_failed"
    ]
    joined = "\n".join(reasons + [str(record.get("terminal_reason") or "")])
    for class_id in (
        "community_spec_closed_vocabulary",
        "community_package_missing",
        "community_computed_unregistered",
        "community_l2_verify_invocation_incomplete",
    ):
        if class_id in joined:
            return class_id
    if "path does not exist: app.spec.yaml" in joined:
        return "community_spec_artifact_missing"
    return "unclassified_terminal"


def task_from_run_id(run_id: str) -> str:
    for task in ("warikan", "mochimono", "vote"):
        if task in run_id:
            return task
    raise ValueError(f"cannot derive task from run ID: {run_id}")


def live_observations(
    arm: str, meta_path: Path, pricing: Path
) -> list[dict[str, Any]]:
    metadata = json.loads(meta_path.read_text(encoding="utf-8"))
    campaign = meta_path.parent
    observations: list[dict[str, Any]] = []
    for record in metadata["runs"]:
        artifact = campaign / "artifacts" / record["name"]
        events = event_rows(artifact)
        cost = community_cost.cost(artifact, pricing)
        assurance = str(record.get("assurance") or "")
        # Community Full is the profile contract projection: an exit-zero L2
        # has passed S/Z/material verification; an exit-zero L3/L4 has also
        # passed B. Generic completion-contract assurance is recorded but is
        # not the Community band definition used by golden-008.
        full = record.get("product_exit") == 0
        stop_class = classify_stop(record, events)
        provider_turns = [
            event for event in events if event.get("event") == "provider_turn_duration"
        ]
        model_returns = [
            {
                "provider": event.get("provider"),
                "requested": event.get("model"),
                "returned": event.get("provider_model_id"),
                "system_fingerprint": event.get("system_fingerprint"),
            }
            for event in provider_turns
            if event.get("provider_model_id") is not None
        ]
        observations.append(
            {
                "arm": arm,
                "id": record["name"],
                "task": task_from_run_id(record["name"]),
                "source": "cm3-matrix-001 live campaign",
                "level": "L3" if (artifact / "src/app-zone").is_dir() else "L2",
                "status": record["status"],
                "product_exit": record.get("product_exit"),
                "verdict": record.get("verdict"),
                "assurance": assurance,
                "full": full,
                "repair_cycles": sum(
                    event.get("event") == "step_verify_repair" for event in events
                ),
                "duration_secs": record["duration_seconds"],
                "cost_usd": cost["cost_usd"],
                "stop_class": stop_class,
                "outcome_signature": stop_class or "full",
                "scrub_ok": record["scrub"]["ok"],
                "provider_turns": len(provider_turns),
                "model_returns": model_returns,
            }
        )
    return observations


def baseline_observations(summary_path: Path) -> list[dict[str, Any]]:
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    observations = []
    for row in summary["runs"]:
        if not any(row["id"].endswith(f"_{index:03d}") for index in range(1, 5)):
            continue
        observations.append(
            {
                "arm": "A",
                "id": f"a_{row['id']}",
                "source_id": row["id"],
                "task": task_from_run_id(row["id"]),
                "source": "cm2b-golden-008 quoted; not re-run",
                "level": row["level"],
                "status": "historical_observation",
                "product_exit": None,
                "verdict": "full" if row["full"] else "failed",
                "assurance": "full" if row["full"] else "not_full",
                "full": row["full"],
                "repair_cycles": row["repair_cycles"],
                "duration_secs": row["duration_secs"],
                "cost_usd": row["cost_usd"],
                "stop_class": row["stop_class"],
                "outcome_signature": row["stop_class"] or "full",
                "scrub_ok": True,
                "provider_turns": None,
                "model_returns": [],
            }
        )
    if len(observations) != 12:
        raise ValueError(f"baseline subset must contain 12 rows, got {len(observations)}")
    return observations


def summarize_arm(rows: list[dict[str, Any]]) -> dict[str, Any]:
    full_count = sum(row["full"] for row in rows)
    one_shot_count = sum(row["full"] and row["repair_cycles"] == 0 for row in rows)
    durations = [float(row["duration_secs"]) for row in rows]
    costs = [float(row["cost_usd"]) for row in rows]
    model_returns = [item for row in rows for item in row["model_returns"]]
    drift = [
        item
        for item in model_returns
        if item["returned"] is not None and item["returned"] != item["requested"]
    ]
    tasks: dict[str, Any] = {}
    for task in ("warikan", "mochimono", "vote"):
        task_rows = [row for row in rows if row["task"] == task]
        tasks[task] = {
            "n": len(task_rows),
            "full": sum(row["full"] for row in task_rows),
            "duration_secs": distribution(
                [float(row["duration_secs"]) for row in task_rows]
            ),
        }
    return {
        "n": len(rows),
        "full": full_count,
        "full_rate": full_count / len(rows),
        "full_wilson_95": wilson(full_count, len(rows)),
        "one_shot_full": one_shot_count,
        "one_shot_full_rate": one_shot_count / len(rows),
        "one_shot_full_wilson_95": wilson(one_shot_count, len(rows)),
        "duration_secs": distribution(durations),
        "cost_usd": {**distribution(costs), "total": sum(costs)},
        "stop_classes": dict(
            sorted(Counter(row["stop_class"] for row in rows if row["stop_class"]).items())
        ),
        "outcome_signatures": dict(
            sorted(Counter(row["outcome_signature"] for row in rows).items())
        ),
        "levels": dict(sorted(Counter(row["level"] for row in rows).items())),
        "artifact_scrub_all": all(row["scrub_ok"] for row in rows),
        "model_identity": {
            "returned_id_observations": len(model_returns),
            "drift_count": len(drift),
            "drift": drift,
        },
        "tasks": tasks,
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--arm-b-meta", type=Path, required=True)
    parser.add_argument("--arm-c-meta", type=Path, required=True)
    parser.add_argument("--arm-d-meta", type=Path, required=True)
    parser.add_argument("--pricing", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    rows = baseline_observations(args.baseline)
    rows.extend(live_observations("B", args.arm_b_meta, args.pricing))
    rows.extend(live_observations("C", args.arm_c_meta, args.pricing))
    rows.extend(live_observations("D", args.arm_d_meta, args.pricing))
    arms = {
        arm: summarize_arm([row for row in rows if row["arm"] == arm])
        for arm in ("A", "B", "C", "D")
    }
    document = {
        "schema_version": "commandagent.cm3-matrix-summary/v1",
        "series_id": "cm3-matrix-001",
        "denominator": 48,
        "live_runs": 36,
        "quoted_runs": 12,
        "budget_ceiling_usd": 8.0,
        "live_cost_usd": sum(row["cost_usd"] for row in rows if row["arm"] != "A"),
        "quoted_baseline_cost_usd": sum(
            row["cost_usd"] for row in rows if row["arm"] == "A"
        ),
        "execution_revision": "b913268f8f045bb77dc07a320b597740f0542877",
        "binary_sha256": "3487e1ef08fc9ff462824a999e21e46652e02eec77d8db952e9b3fd949a35a44",
        "arms": arms,
        "runs": rows,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(document, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )


if __name__ == "__main__":
    main()
