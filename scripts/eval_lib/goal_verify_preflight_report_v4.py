from __future__ import annotations

import json
from pathlib import Path
from typing import Any


def load_records(run_dir: Path) -> list[dict[str, Any]]:
    return [
        json.loads(path.read_text(encoding="utf-8"))
        for path in sorted((run_dir / "raw").glob("**/pair-*.json"))
    ]


def semantic_review_gate(
    *, contract: dict[str, Any], blind_report: dict[str, Any] | None
) -> bool:
    if not isinstance(blind_report, dict):
        return False
    checks = blind_report.get("checks")
    if not isinstance(checks, dict) or not checks:
        return False
    if blind_report.get("semantic_review_complete") is not True:
        return False
    semantic_contract = contract.get("semantic_review", {})
    if semantic_contract.get("calibration_reviewer_required") is True:
        required_checks = {
            key: value
            for key, value in checks.items()
            if key != "human_review_complete"
        }
        if not required_checks or not all(required_checks.values()):
            return False
        calibration = blind_report.get(
            "calibration_review", blind_report.get("human_review")
        )
        return _calibration_reviewer_matches_contract(
            semantic_contract=semantic_contract,
            calibration=calibration,
        )
    if not all(checks.values()):
        return False
    if semantic_contract.get("independent_human_required") is not True:
        return True
    human = blind_report.get("human_review")
    return (
        isinstance(human, dict)
        and human.get("valid") is True
        and human.get("reviewer_type") == "human"
        and human.get("contract_authoring_involvement") is False
        and human.get("independence_confirmed") is True
    )


def _calibration_reviewer_matches_contract(
    *, semantic_contract: dict[str, Any], calibration: Any
) -> bool:
    if not isinstance(calibration, dict) or calibration.get("valid") is not True:
        return False
    reviewer_type = calibration.get("reviewer_type")
    if reviewer_type == "human":
        return (
            calibration.get("contract_authoring_involvement") is False
            and calibration.get("independence_confirmed") is True
        )
    if reviewer_type != "ai":
        return False
    policy = semantic_contract.get("calibration_reviewer_policy")
    if not isinstance(policy, dict) or "ai" not in policy.get(
        "allowed_reviewer_types", []
    ):
        return False
    authorization = policy.get("authorized_ai_reviewer")
    if not isinstance(authorization, dict):
        return False
    exact_fields = (
        "reviewer_id",
        "provider",
        "model_family",
        "model_id_or_version",
        "contract_authoring_involvement",
        "authorization_id",
        "authorization_scope",
        "authorized_at",
        "authorized_by",
    )
    if any(
        calibration.get(field) != authorization.get(field) for field in exact_fields
    ):
        return False
    return all(
        calibration.get(field) is True
        for field in (
            "user_authorized",
            "source_blind_confirmed",
            "forbidden_materials_not_accessed",
            "reviewer_output_independence_confirmed",
        )
    )


def _baseline_honest_terminal(row: dict[str, Any]) -> bool:
    if row.get("completion_verify_attempt_recorded") is True:
        return True
    return (
        row.get("status") == "failed"
        and isinstance(row.get("returncode"), int)
        and row["returncode"] != 0
        and bool(row.get("product_run_dir"))
    )


def build_report(
    *,
    contract: dict[str, Any],
    records: list[dict[str, Any]],
    semantic_review_complete: bool,
) -> dict[str, Any]:
    lanes = [lane for record in records for lane in record.get("lanes", {}).values()]
    valid = [lane for lane in lanes if lane.get("validation", {}).get("valid") is True]
    valid_before_repairs = [
        lane
        for lane in lanes
        if lane.get("validation", {}).get(
            "valid_before_host_repairs",
            lane.get("validation", {}).get("valid"),
        )
        is True
    ]
    host_repaired = [
        lane for lane in lanes if lane.get("validation", {}).get("host_repairs")
    ]
    evaluations = [
        row
        for lane in lanes
        for row in lane.get("execution", {}).get("evaluations", [])
    ]
    executable = [
        row for row in evaluations if row.get("classification") == "executable"
    ]
    additive = [
        lane["additive_comparison"]
        for lane in lanes
        if isinstance(lane.get("additive_comparison"), dict)
    ]
    baselines = [record.get("baseline", {}) for record in records]
    unverifiable_claims = [
        claim
        for lane in valid
        for claim in lane.get("validation", {}).get("unverifiable_claims", [])
    ]
    baseline_completion_required = (
        contract.get("baseline", {}).get("completion_verify_result_required") is True
    )
    baseline_task_contract_required = (
        contract.get("baseline", {}).get("task_contract_bound_required") is True
        or baseline_completion_required
    )
    baseline_run_required = (
        contract.get("baseline", {}).get("product_run_discovered_required") is True
        or baseline_completion_required
    )
    baseline_terminal_required = (
        contract.get("baseline", {}).get("honest_terminal_required") is True
    )
    false_full = sum(
        row.get("shadow_verdict") == "pass"
        and any(
            claim.get("status") != "strong"
            for claim in row.get("combined_score", {}).get("claims", [])
        )
        for row in additive
    )
    expected_records = len(contract["selected_cells"]) * int(
        contract["samples_per_cell"]
    )
    checks = {
        "record_count": len(records) == expected_records,
        "schema_compliance": bool(lanes)
        and len(valid) / len(lanes)
        >= contract["preflight"]["schema_compliance_yield_floor"],
        "product_snapshot_recorded": all(
            record.get("snapshot_manifests", {})
            .get("product", {})
            .get("snapshot_sha256")
            for record in records
        ),
        "same_snapshot": all(
            lane.get("execution", {}).get("same_snapshot") is True for lane in valid
        ),
        "reference_fallback_zero": sum(
            int(lane.get("execution", {}).get("reference_fallback_count", 0))
            for lane in lanes
        )
        == 0,
        "gold_used_for_execution_zero": sum(
            int(lane.get("execution", {}).get("gold_used_for_execution_count", 0))
            for lane in lanes
        )
        == 0,
        "executable_oracle_attempts_recorded": all(
            row.get("execution_attempt_recorded") is True for row in executable
        ),
        "baseline_failure_not_overridden": all(
            row.get("baseline_failure_overridden") is False for row in additive
        ),
        "shadow_false_full_zero": false_full == 0,
        "semantic_review_complete": semantic_review_complete,
        "baseline_task_contract_bound": not baseline_task_contract_required
        or all(row.get("completion_contract_bound") is True for row in baselines),
        "baseline_run_discovered": not baseline_run_required
        or all(bool(row.get("product_run_dir")) for row in baselines),
        "baseline_completion_verify_attempted": not baseline_completion_required
        or all(
            row.get("completion_verify_attempt_recorded") is True for row in baselines
        ),
        "baseline_honest_terminal_recorded": not baseline_terminal_required
        or all(_baseline_honest_terminal(row) for row in baselines),
    }
    return {
        "schema_version": "commandagent.goal_verify.phase6_preflight_report.v4",
        "contract_id": contract["contract_id"],
        "counts": {
            "records": len(records),
            "lanes": len(lanes),
            "schema_valid": len(valid),
            "schema_valid_before_host_repairs": len(valid_before_repairs),
            "host_repaired_lanes": len(host_repaired),
            "host_repairs": sum(
                len(lane.get("validation", {}).get("host_repairs", []))
                for lane in lanes
            ),
            "candidate_oracles": len(evaluations),
            "executable_candidate_oracles": len(executable),
            "explicit_unverifiable_claims": len(unverifiable_claims),
            "baseline_product_runs": sum(
                bool(row.get("product_run_dir")) for row in baselines
            ),
            "baseline_completion_verify_attempts": sum(
                row.get("completion_verify_attempt_recorded") is True
                for row in baselines
            ),
            "baseline_honest_early_failures": sum(
                row.get("completion_verify_attempt_recorded") is not True
                and _baseline_honest_terminal(row)
                for row in baselines
            ),
            "baseline_observations": sum(
                len(row.get("observations", [])) for row in baselines
            ),
            "shadow_false_full": false_full,
        },
        "paired_delta": {
            "required_claim_recall": [
                row["paired_delta"]["required_claim_recall"] for row in additive
            ],
            "strong_binding": [
                row["paired_delta"]["strong_binding"] for row in additive
            ],
            "unverified_rate": [
                row["paired_delta"]["unverified_rate"] for row in additive
            ],
        },
        "checks": checks,
        "ready_for_full_experiment_design": all(checks.values()),
    }
