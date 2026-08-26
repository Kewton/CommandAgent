from __future__ import annotations

import copy
from collections.abc import Callable
from pathlib import Path
from typing import Any

from eval_lib.goal_verify_executors_v3 import execute_registered


def classify_candidate_oracle(
    oracle: dict[str, Any], adapter: dict[str, Any] | None
) -> str:
    if adapter is None:
        return "concretization_failure"
    executor = adapter.get("executor", {})
    if executor.get("kind") == "unavailable":
        return "executor_unavailable"
    if _unsafe_provider_input(oracle):
        return "policy_rejected"
    return "executable"


def _unsafe_provider_input(oracle: dict[str, Any]) -> bool:
    input_value = oracle.get("input")
    if not isinstance(input_value, dict):
        return False
    serialized = str(input_value)
    return "../" in serialized or "bash -c" in serialized or "sh -c" in serialized


def proposal_matches_adapter(
    oracle: dict[str, Any], adapter: dict[str, Any], *, compare_claim_id: bool
) -> bool:
    proposal = adapter["proposal"]
    observation = oracle.get("observation", {})
    if compare_claim_id and oracle.get("claim_id") != adapter["claim_id"]:
        return False
    if oracle.get("strategy") not in proposal["strategies"]:
        return False
    if oracle.get("expected_polarity") not in proposal["polarities"]:
        return False
    if observation.get("kind") not in proposal["observation_kinds"]:
        return False
    expected_values = proposal.get("expected_values")
    if expected_values is not None and str(observation.get("expected")) not in {
        str(item) for item in expected_values
    }:
        return False
    expected_contains = proposal.get("expected_contains")
    if expected_contains is not None:
        text = str(observation)
        if not all(fragment in text for fragment in expected_contains):
            return False
    return True


def select_adapter(
    oracle: dict[str, Any],
    adapters: list[dict[str, Any]],
    *,
    case_id: str,
    compare_claim_id: bool,
) -> tuple[dict[str, Any] | None, str | None]:
    matches = [
        row
        for row in adapters
        if row["case_id"] == case_id
        and proposal_matches_adapter(oracle, row, compare_claim_id=compare_claim_id)
    ]
    if len(matches) == 1:
        return matches[0], None
    return None, "adapter_missing" if not matches else "adapter_ambiguous"


def evaluate_candidate_spec(
    *,
    case_id: str,
    spec: dict[str, Any],
    adapters: list[dict[str, Any]],
    workspaces: dict[tuple[str, str], Path],
    lane: str,
    executor: Callable[..., dict[str, Any]] = execute_registered,
) -> dict[str, Any]:
    compare_claim_id = lane == "contract_conformance"
    evaluations = []
    matched_adapter_ids = set()
    for oracle in spec.get("oracles", []):
        adapter, selection_error = select_adapter(
            oracle,
            adapters,
            case_id=case_id,
            compare_claim_id=compare_claim_id,
        )
        classification = classify_candidate_oracle(oracle, adapter)
        base = {
            "oracle_id": oracle.get("id"),
            "claim_id": oracle.get("claim_id"),
            "classification": classification,
            "adapter_id": adapter.get("adapter_id") if adapter else None,
            "executor_kind": adapter.get("executor", {}).get("kind")
            if adapter
            else None,
            "claim_id_compared": compare_claim_id,
        }
        if classification != "executable":
            evaluations.append(
                {
                    **base,
                    "executed": False,
                    "result": "unverified",
                    "reason": selection_error or classification,
                    "observation_match": False,
                }
            )
            continue
        matched_adapter_ids.add(adapter["adapter_id"])
        workspace_key = (case_id, adapter["executor"]["stage"])
        workspace = workspaces.get(workspace_key)
        if workspace is None:
            evaluations.append(
                {
                    **base,
                    "classification": "concretization_failure",
                    "executed": False,
                    "result": "unverified",
                    "reason": "workspace_stage_missing",
                    "observation_match": False,
                }
            )
            continue
        outcome = executor(copy.deepcopy(adapter["executor"]), workspace=workspace)
        evaluations.append(
            {
                **base,
                **outcome,
                "observation_match": outcome.get("result") == "pass",
                "observed_strength": adapter["executor"].get("observed_strength")
                if outcome.get("result") == "pass"
                else None,
            }
        )
    expected_ids = {row["adapter_id"] for row in adapters if row["case_id"] == case_id}
    return {
        "lane": lane,
        "case_id": case_id,
        "evaluations": evaluations,
        "scoring_coverage": all(
            row["classification"]
            in {
                "executable",
                "executor_unavailable",
                "policy_rejected",
                "concretization_failure",
            }
            for row in evaluations
        ),
        "unmatched_registered_adapter_ids": sorted(expected_ids - matched_adapter_ids),
        "extra_candidate_oracle_count": sum(
            1 for row in evaluations if row["adapter_id"] is None
        ),
    }


def score_claim_coverage(
    *,
    case: dict[str, Any],
    adapters: list[dict[str, Any]],
    evaluations: list[dict[str, Any]],
) -> dict[str, Any]:
    passed = {
        row["adapter_id"]: row
        for row in evaluations
        if row.get("observation_match") and row.get("adapter_id")
    }
    strengths = {"weak": 1, "deterministic": 2, "runtime": 3}
    rows = []
    for claim in case["required_claims"]:
        entries = [
            row
            for row in adapters
            if row["case_id"] == case["case_id"] and row["claim_id"] == claim["id"]
        ]
        matched = [
            passed[row["adapter_id"]] for row in entries if row["adapter_id"] in passed
        ]
        minimum = strengths[claim["min_strength"]]
        strong = (
            bool(entries)
            and len(matched) == len(entries)
            and all(
                strengths.get(row.get("observed_strength"), 0) >= minimum
                for row in matched
            )
        )
        weak = bool(matched) and not strong
        rows.append(
            {
                "claim_id": claim["id"],
                "status": "strong" if strong else "weak" if weak else "unverified",
                "matched_adapter_ids": sorted(row["adapter_id"] for row in matched),
                "required_adapter_ids": sorted(row["adapter_id"] for row in entries),
            }
        )
    return {
        "claims": rows,
        "required_claim_observation_recall": (
            sum(row["status"] != "unverified" for row in rows) / len(rows)
            if rows
            else 0.0
        ),
        "strong_binding_by_observation": (
            sum(row["status"] == "strong" for row in rows) / len(rows) if rows else 0.0
        ),
    }
