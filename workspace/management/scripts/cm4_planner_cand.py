#!/usr/bin/env python3
"""Curate CM-4 planner-generation candidate observations."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any

import cm3_matrix
import cm3b_matrix

EXECUTION_REVISION = "f2072b570b5eddde06215e8025cce859335c7916"
BINARY_SHA256 = "b9f9818602d34c1b383a1910bcaf0c8737d596bcf0d792f5b3e0399d330c13fa"


def classify_terminal(reason: str, product_exit: int | None) -> str | None:
    if product_exit == 0:
        return None
    signatures = (
        ("community_computed_unregistered", "community_computed_unregistered"),
        ("community_esbuild_script_missing", "community_esbuild_script_missing"),
        ("path does not exist: app.spec.yaml", "community_spec_artifact_missing"),
        ("stdin is not a TTY", "community_verify_instruction_not_executable"),
        ("planner_empty_response", "community_planner_empty_response"),
    )
    for token, class_id in signatures:
        if token in reason:
            return class_id
    return "unclassified_terminal"


def live_rows(
    label: str, think: str, meta_path: Path, pricing: Path
) -> list[dict[str, Any]]:
    metadata = json.loads(meta_path.read_text(encoding="utf-8"))
    if metadata["suite"].get("think") != think:
        raise ValueError(
            f"campaign think mismatch: expected {think}, "
            f"observed {metadata['suite'].get('think')}"
        )
    preflight = metadata["preflight"]
    if preflight["head_sha"] != EXECUTION_REVISION:
        raise ValueError("campaign execution revision drifted")
    if preflight["binary_sha256"]["installed"] != BINARY_SHA256:
        raise ValueError("campaign binary SHA-256 drifted")
    records = {record["name"]: record for record in metadata["runs"]}
    rows = cm3_matrix.live_observations(label, meta_path, pricing)
    for row in rows:
        record = records[row["id"]]
        expected_argument = f"--think={think}"
        if expected_argument not in record["command_argv"]:
            raise ValueError(f"{row['id']} does not declare {expected_argument}")
        row["source"] = "cm4-planner-cand-001 live campaign"
        row["think"] = think
        row["stop_class"] = classify_terminal(
            str(record.get("terminal_reason") or ""), record.get("product_exit")
        )
        row["outcome_signature"] = row["stop_class"] or "full"
    return rows


def campaign_window(meta_path: Path) -> dict[str, int]:
    metadata = json.loads(meta_path.read_text(encoding="utf-8"))
    runs = metadata["runs"]
    return {
        "started_epoch": min(int(record["start_epoch"]) for record in runs),
        "ended_epoch": max(int(record["end_epoch"]) for record in runs),
        "wall_seconds": max(int(record["end_epoch"]) for record in runs)
        - min(int(record["start_epoch"]) for record in runs),
    }


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def evidence_hashes(label: str, meta_path: Path) -> dict[str, Any]:
    campaign = meta_path.parent
    events = []
    for path in sorted((campaign / "artifacts").rglob("events.jsonl")):
        events.append(
            {
                "path": str(path.relative_to(campaign)),
                "sha256": sha256_file(path),
            }
        )
    return {
        "arm": label,
        "uat_meta_sha256": sha256_file(meta_path),
        "events": events,
    }


def comparison(old: dict[str, Any], new: dict[str, Any]) -> dict[str, Any]:
    return cm3b_matrix.comparison(old, new)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--matrix-001", type=Path, required=True)
    parser.add_argument("--arm-e-meta", type=Path, required=True)
    parser.add_argument("--arm-f-meta", type=Path, required=True)
    parser.add_argument("--pricing", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    baseline_document = json.loads(args.matrix_001.read_text(encoding="utf-8"))
    baseline = baseline_document["arms"]["A"]
    rows = live_rows("E", "medium", args.arm_e_meta, args.pricing)
    rows.extend(live_rows("F", "high", args.arm_f_meta, args.pricing))
    arms = {
        label: cm3_matrix.summarize_arm(
            [row for row in rows if row["arm"] == label]
        )
        for label in ("E", "F")
    }
    document = {
        "schema_version": "commandagent.cm4-planner-candidate-summary/v1",
        "series_id": "cm4-planner-cand-001",
        "denominator": 24,
        "execution_revision": EXECUTION_REVISION,
        "binary_sha256": BINARY_SHA256,
        "baseline": {"label": "A", **baseline},
        "arms": arms,
        "comparisons": {
            "A_to_E": comparison(baseline, arms["E"]),
            "A_to_F": comparison(baseline, arms["F"]),
            "E_to_F": comparison(arms["E"], arms["F"]),
        },
        "campaign_windows": {
            "E": campaign_window(args.arm_e_meta),
            "F": campaign_window(args.arm_f_meta),
        },
        "evidence_hashes": [
            evidence_hashes("E", args.arm_e_meta),
            evidence_hashes("F", args.arm_f_meta),
        ],
        "live_cost_usd": sum(float(row["cost_usd"]) for row in rows),
        "adoption_decision": "owner_adjudication_pending",
        "runs": rows,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(document, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )


if __name__ == "__main__":
    main()
