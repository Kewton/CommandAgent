from __future__ import annotations

import json
from pathlib import Path
from typing import Any


def load_records(run_dir: Path) -> list[dict[str, Any]]:
    return [
        json.loads(path.read_text(encoding="utf-8"))
        for path in sorted((run_dir / "raw").glob("**/pair-*.json"))
    ]


def build_report(
    *, contract: dict[str, Any], records: list[dict[str, Any]], semantic_review_complete: bool
) -> dict[str, Any]:
    lanes = [lane for record in records for lane in record.get("lanes", {}).values()]
    valid = [lane for lane in lanes if lane.get("validation", {}).get("valid") is True]
    evaluations = [
        row for lane in lanes for row in lane.get("execution", {}).get("evaluations", [])
    ]
    executable = [row for row in evaluations if row.get("classification") == "executable"]
    additive = [
        lane["additive_comparison"]
        for lane in lanes
        if isinstance(lane.get("additive_comparison"), dict)
    ]
    false_full = sum(
        row.get("shadow_verdict") == "pass"
        and any(
            claim.get("status") != "strong"
            for claim in row.get("combined_score", {}).get("claims", [])
        )
        for row in additive
    )
    expected_records = len(contract["selected_cells"]) * int(contract["samples_per_cell"])
    checks = {
        "record_count": len(records) == expected_records,
        "schema_compliance": bool(lanes)
        and len(valid) / len(lanes) >= contract["preflight"]["schema_compliance_yield_floor"],
        "product_snapshot_recorded": all(
            record.get("snapshot_manifests", {}).get("product", {}).get("snapshot_sha256")
            for record in records
        ),
        "same_snapshot": all(
            lane.get("execution", {}).get("same_snapshot") is True for lane in valid
        ),
        "reference_fallback_zero": sum(
            int(lane.get("execution", {}).get("reference_fallback_count", 0)) for lane in lanes
        )
        == 0,
        "gold_used_for_execution_zero": sum(
            int(lane.get("execution", {}).get("gold_used_for_execution_count", 0))
            for lane in lanes
        )
        == 0,
        "executable_oracles_recorded": all(row.get("executed") is True for row in executable),
        "baseline_failure_not_overridden": all(
            row.get("baseline_failure_overridden") is False for row in additive
        ),
        "shadow_false_full_zero": false_full == 0,
        "semantic_review_complete": semantic_review_complete,
    }
    return {
        "schema_version": "commandagent.goal_verify.phase6_preflight_report.v4",
        "contract_id": contract["contract_id"],
        "counts": {
            "records": len(records),
            "lanes": len(lanes),
            "schema_valid": len(valid),
            "candidate_oracles": len(evaluations),
            "executable_candidate_oracles": len(executable),
            "shadow_false_full": false_full,
        },
        "paired_delta": {
            "required_claim_recall": [
                row["paired_delta"]["required_claim_recall"] for row in additive
            ],
            "strong_binding": [row["paired_delta"]["strong_binding"] for row in additive],
            "unverified_rate": [row["paired_delta"]["unverified_rate"] for row in additive],
        },
        "checks": checks,
        "ready_for_full_experiment_design": all(checks.values()),
    }
