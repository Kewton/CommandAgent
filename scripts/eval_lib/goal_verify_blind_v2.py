from __future__ import annotations

import copy
import json
import random
from typing import Any


def _raw_proposal(record: dict[str, Any]) -> dict[str, Any]:
    response = record.get("response", {})
    if response.get("status") != "completed":
        return {"parse_status": "provider_error", "claims": [], "oracles": []}
    raw = response.get("response", {}).get("response", "")
    try:
        proposal = json.loads(raw)
    except (TypeError, json.JSONDecodeError):
        return {"parse_status": "malformed_json", "claims": [], "oracles": []}
    if not isinstance(proposal, dict):
        return {"parse_status": "non_object", "claims": [], "oracles": []}
    claims = proposal.get("claims")
    oracles = proposal.get("oracles")
    return {
        "parse_status": "parsed",
        "goal": proposal.get("goal"),
        "intent": proposal.get("intent"),
        "claims": copy.deepcopy(claims) if isinstance(claims, list) else [],
        "oracles": copy.deepcopy(oracles) if isinstance(oracles, list) else [],
    }


def semantic_proposal_card(record: dict[str, Any]) -> dict[str, Any]:
    """Expose raw proposal semantics while dropping provider and run identity."""
    card = _raw_proposal(record)
    for claim in card["claims"]:
        if isinstance(claim, dict):
            origin = claim.get("origin")
            if isinstance(origin, dict):
                origin.pop("lineage", None)
                if "artifact_path" in origin:
                    origin["artifact_path"] = "<evidence-artifact>"
    for oracle in card["oracles"]:
        if isinstance(oracle, dict):
            oracle.pop("lineage", None)
            oracle.pop("lifecycle", None)
            oracle.pop("result", None)
            oracle.pop("observed_strength", None)
    card["execution_results"] = [
        {
            key: evaluation.get(key)
            for key in (
                "oracle_id",
                "executed",
                "result",
                "observed_strength",
                "reason",
                "actual",
            )
            if key in evaluation
        }
        for evaluation in record.get("oracle_evaluations", [])
        if isinstance(evaluation, dict)
    ]
    return card


def prepare_semantic_items(
    left_records: dict[str, dict[str, Any]],
    right_records: dict[str, dict[str, Any]],
    *,
    cases_by_pair_id: dict[str, dict[str, Any]],
    seed: int,
) -> tuple[list[dict[str, Any]], dict[str, dict[str, str]]]:
    """Build anonymous A/B items from two same-scope raw proposal arms."""
    if (
        left_records.keys() != right_records.keys()
        or left_records.keys() != cases_by_pair_id.keys()
    ):
        raise ValueError("semantic blind review requires identical paired record IDs")
    rng = random.Random(seed)
    items = []
    mapping = {}
    for pair_id in sorted(left_records):
        left = semantic_proposal_card(left_records[pair_id])
        right = semantic_proposal_card(right_records[pair_id])
        swapped = bool(rng.getrandbits(1))
        variants = [right, left] if swapped else [left, right]
        mapping[pair_id] = {
            "A": "right" if swapped else "left",
            "B": "left" if swapped else "right",
        }
        case = cases_by_pair_id[pair_id]
        items.append(
            {
                "pair_id": pair_id,
                "goal": case["goal"],
                "intent": case["intent"],
                "required_claims": copy.deepcopy(case["required_claims"]),
                "variant_A": variants[0],
                "variant_B": variants[1],
            }
        )
    return items, mapping


def semantic_arms_from_paired_records(
    records_by_pair_id: dict[str, dict[str, Any]],
) -> tuple[dict[str, dict[str, Any]], dict[str, dict[str, Any]]]:
    """Split runner records into same-boundary deterministic and candidate raw arms."""
    baseline = {}
    candidate = {}
    for pair_id, record in records_by_pair_id.items():
        baseline_spec = record.get("baseline_spec")
        if not isinstance(baseline_spec, dict):
            raise TypeError(f"paired record lacks baseline spec: {pair_id}")
        baseline[pair_id] = {
            "response": {
                "status": "completed",
                "response": {"response": json.dumps(baseline_spec, ensure_ascii=False)},
            },
            "oracle_evaluations": copy.deepcopy(
                record.get("baseline_oracle_evaluations", [])
            ),
        }
        candidate[pair_id] = record
    return baseline, candidate
