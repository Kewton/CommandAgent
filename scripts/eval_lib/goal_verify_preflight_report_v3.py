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
    *, contract: dict[str, Any], records: list[dict[str, Any]], blind_complete: bool
) -> dict[str, Any]:
    primary = {
        row["case_id"] for row in contract["selected_cells"] if row["lane"] == "primary"
    }
    primary_records = [row for row in records if row["source_case_id"] in primary]
    conformance = [row["lanes"]["contract_conformance"] for row in primary_records]
    held = [row["lanes"]["held_out_synthesis"] for row in primary_records]
    conformance_valid = sum(
        row["validation"].get("valid") is True for row in conformance
    )
    held_valid = sum(row["validation"].get("valid") is True for row in held)
    candidate_evaluations = [
        evaluation
        for row in conformance
        for evaluation in row.get("execution", {}).get("evaluations", [])
    ]
    command_rows = [
        row
        for row in candidate_evaluations
        if row.get("classification") == "executable"
        and row.get("executor_kind")
        in {
            "sandbox_command",
            "stage_command",
            "fixture_hash_command",
            "regression_set",
        }
    ]
    fix_rows = [
        row
        for record, lane in zip(primary_records, conformance, strict=True)
        if record["intent"] == "fix"
        for row in lane.get("execution", {}).get("evaluations", [])
    ]
    smoke_by_case = {
        case_id: any(
            evaluation.get("executed") is True
            for record, lane in zip(primary_records, held, strict=True)
            if record["source_case_id"] == case_id
            for evaluation in lane.get("execution", {}).get("evaluations", [])
        )
        for case_id in primary
    }
    checks = {
        "record_count": len(records) == 40,
        "conformance_schema": conformance_valid >= 34,
        "held_out_schema": held_valid >= 34,
        "held_out_scoring_coverage": all(
            row.get("execution", {}).get("scoring_coverage") is True
            for row in held
            if row["validation"].get("valid")
        ),
        "held_out_classification_coverage": all(
            evaluation.get("classification")
            in {
                "executable",
                "executor_unavailable",
                "policy_rejected",
                "concretization_failure",
            }
            for row in held
            for evaluation in row.get("execution", {}).get("evaluations", [])
        ),
        "held_out_smoke": all(smoke_by_case.values()),
        "command_oracle_success": bool(command_rows)
        and sum(row.get("result") == "pass" for row in command_rows) / len(command_rows)
        >= 0.8,
        "fix_registry_integrity": bool(fix_rows)
        and all(row.get("adapter_id") is not None for row in fix_rows),
        "baseline_complete": len(primary_records) == 35
        and all(
            row["baseline"].get("status") == "completed" for row in primary_records
        ),
        "semantic_blind_review_complete": blind_complete,
    }
    return {
        "schema_version": "commandagent.goal_verify.phase6_preflight_report.v3",
        "contract_id": contract["contract_id"],
        "counts": {
            "records": len(records),
            "primary_records": len(primary_records),
            "conformance_schema_valid": conformance_valid,
            "held_out_schema_valid": held_valid,
            "command_oracles": len(command_rows),
            "fix_oracles": len(fix_rows),
        },
        "smoke_by_case": smoke_by_case,
        "checks": checks,
        "ready_for_full_experiment_design": all(checks.values()),
    }
