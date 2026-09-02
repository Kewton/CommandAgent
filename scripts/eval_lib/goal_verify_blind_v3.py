from __future__ import annotations

import copy
import hashlib
import json
import random
from collections import Counter
from typing import Any


def _canonical(value: Any) -> bytes:
    return json.dumps(
        value, ensure_ascii=False, separators=(",", ":"), sort_keys=True
    ).encode()


def prepare_blind_items(
    *, records: list[dict[str, Any]], contract_sha256: str, lane: str
) -> tuple[list[dict[str, Any]], dict[str, str]]:
    if lane not in {"semantic_hidden", "execution_visible"}:
        raise ValueError("unknown blind lane")
    seed = int(contract_sha256[:16], 16)
    rng = random.Random(seed + (0 if lane == "semantic_hidden" else 1))
    items = []
    mapping = {}
    for record in records:
        pair_id = record["pair_id"]
        arms = [
            ("baseline", _card(record["baseline_card"], lane)),
            ("candidate", _card(record["candidate_card"], lane)),
        ]
        rng.shuffle(arms)
        labels = ("variant_a", "variant_b")
        item = {
            "item_id": f"{lane}:{pair_id}",
            "pair_id": pair_id,
            "lane": lane,
            "goal": record["goal"],
            "intent": record["intent"],
            "profile": record["profile"],
            "required_claims": [{"id": row["id"]} for row in record["required_claims"]],
        }
        for label, (arm, card) in zip(labels, arms, strict=True):
            item[label] = card
            mapping[f"{item['item_id']}:{label}"] = arm
        item["item_sha256"] = hashlib.sha256(_canonical(item)).hexdigest()
        items.append(item)
    rng.shuffle(items)
    return items, mapping


def _card(card: dict[str, Any], lane: str) -> dict[str, Any]:
    result = copy.deepcopy(card)
    for key in (
        "arm",
        "provider",
        "model",
        "request_id",
        "token_counts",
        "timing",
        "record_path",
        "lineage_hashes",
        "machine_verdict",
    ):
        result.pop(key, None)
    if lane == "semantic_hidden":
        result.pop("execution_results", None)
        result.pop("canonical_answer_key", None)
    return result


def human_sample(items: list[dict[str, Any]], primary_cases: list[str]) -> list[str]:
    by_pair = {item["pair_id"]: item["item_id"] for item in items}
    selected = [f"{case}--pair-01" for case in primary_cases]
    selected.extend(f"{case}--pair-02" for case in sorted(primary_cases)[:3])
    return [by_pair[pair_id] for pair_id in selected]


def records_to_blind_inputs(records: list[dict[str, Any]]) -> list[dict[str, Any]]:
    converted = []
    for record in records:
        lane = record["lanes"]["held_out_synthesis"]
        spec, parse_status = _raw_candidate_proposal(lane)
        candidate = {
            "claims": copy.deepcopy((spec or {}).get("claims", [])),
            "oracles": copy.deepcopy((spec or {}).get("oracles", [])),
            "parse_status": parse_status,
            "execution_results": copy.deepcopy(
                lane.get("execution", {}).get("evaluations", [])
            ),
        }
        baseline = {
            "claims": copy.deepcopy(
                record.get("baseline", {}).get("coverage", {}).get("claims", [])
            ),
            "oracles": copy.deepcopy(
                record.get("baseline", {}).get("observations", [])
            ),
            "execution_results": copy.deepcopy(
                record.get("baseline", {}).get("evaluations", [])
            ),
        }
        converted.append(
            {
                "pair_id": record["pair_id"],
                "goal": record["goal"],
                "intent": record["intent"],
                "profile": record["profile"],
                "required_claims": record["required_claims"],
                "baseline_card": baseline,
                "candidate_card": candidate,
            }
        )
    return converted


def _raw_candidate_proposal(
    lane: dict[str, Any],
) -> tuple[dict[str, Any] | None, str]:
    """Return provider proposal semantics before host canonicalization."""
    attempts = lane.get("attempts", [])
    if not attempts:
        return None, "missing_attempt"
    response = attempts[-1].get("response", {})
    if response.get("status") != "completed":
        return None, "provider_error"
    raw = response.get("response", {}).get("response")
    if not isinstance(raw, str):
        return None, "missing_response"
    try:
        proposal = json.loads(raw)
    except json.JSONDecodeError:
        return None, "malformed_json"
    if not isinstance(proposal, dict):
        return None, "non_object"
    return proposal, "parsed"


def cohen_kappa(left: list[str], right: list[str]) -> dict[str, float]:
    if len(left) != len(right) or not left:
        raise ValueError("kappa requires two non-empty equally sized ratings")
    labels = sorted(set(left) | set(right))
    observed = sum(a == b for a, b in zip(left, right, strict=True)) / len(left)
    left_counts = Counter(left)
    right_counts = Counter(right)
    expected = sum(
        left_counts[label] * right_counts[label] for label in labels
    ) / len(left) ** 2
    kappa = (observed - expected) / (1 - expected) if expected < 1 else 1.0
    return {"agreement": observed, "expected_agreement": expected, "kappa": kappa}


def build_blind_review_report(
    *,
    items: list[dict[str, Any]],
    mapping: dict[str, str],
    model_reviews: list[dict[str, Any]],
    human_review: dict[str, Any],
    required_human_ids: list[str],
) -> dict[str, Any]:
    item_ids = {item["item_id"] for item in items}
    model_errors: list[str] = []
    model_oriented: list[dict[str, str]] = []
    families = set()
    expected_items_sha256 = hashlib.sha256(_canonical(items)).hexdigest()
    required_provenance = {
        "provider",
        "model_id_or_version",
        "invoked_at",
        "items_sha256",
        "raw_response",
        "parsed_reviews",
        "invocation_script_sha256",
        "independent",
    }
    for index, review in enumerate(model_reviews):
        missing = sorted(required_provenance - review.keys())
        if missing:
            model_errors.append(f"model_{index + 1}_provenance_missing:{','.join(missing)}")
        if review.get("independent") is not True:
            model_errors.append(f"model_{index + 1}_not_independent")
        if review.get("items_sha256") != expected_items_sha256:
            model_errors.append(f"model_{index + 1}_items_hash_mismatch")
        families.add(review.get("model_family"))
        parsed = review.get("parsed_reviews", [])
        oriented, errors = _orient_reviews(parsed, item_ids, mapping)
        model_oriented.append(oriented)
        model_errors.extend(f"model_{index + 1}:{error}" for error in errors)
    if len(model_reviews) < 2:
        model_errors.append("model_reviewer_count_below_2")
    if None in families or len(families) < 2:
        model_errors.append("distinct_model_families_below_2")
    if not any(review.get("provider") == "ollama" for review in model_reviews):
        model_errors.append("local_ollama_reviewer_missing")

    pairwise = []
    if len(model_oriented) >= 2:
        shared = sorted(set(model_oriented[0]) & set(model_oriented[1]))
        if set(shared) != item_ids:
            model_errors.append("model_pair_does_not_cover_all_items")
        elif shared:
            pairwise.append(
                cohen_kappa(
                    [model_oriented[0][item] for item in shared],
                    [model_oriented[1][item] for item in shared],
                )
            )

    human_rows = human_review.get("reviews", [])
    human, human_errors = _orient_reviews(human_rows, set(required_human_ids), mapping)
    for row in human_rows:
        item_id = row.get("item_id")
        for field in ("reviewer_id", "reviewed_at", "reason_codes", "rationale"):
            if not row.get(field):
                human_errors.append(f"human_field_missing:{item_id}:{field}")
    if set(human) != set(required_human_ids):
        human_errors.append("human_fixed_sample_incomplete")
    consensus = {}
    if len(model_oriented) >= 2:
        for item_id in item_ids:
            values = [row.get(item_id) for row in model_oriented]
            consensus[item_id] = values[0] if len(set(values)) == 1 else "disagree"
    shared = [item_id for item_id in required_human_ids if item_id in human]
    human_agreement = (
        sum(human[item_id] == consensus.get(item_id) for item_id in shared) / len(shared)
        if shared
        else 0.0
    )
    model_gate = bool(pairwise) and pairwise[0]["kappa"] >= 0.4
    human_gate = len(shared) == len(required_human_ids) and human_agreement >= 0.7
    complete = not model_errors and not human_errors and model_gate and human_gate
    return {
        "schema_version": "commandagent.goal_verify.semantic_blind_report.v3",
        "semantic_blind_review_complete": complete,
        "model_errors": sorted(set(model_errors)),
        "human_errors": sorted(set(human_errors)),
        "model_model": pairwise,
        "model_model_gate": model_gate,
        "human_vs_model_consensus_agreement": human_agreement,
        "human_vs_model_gate": human_gate,
        "reviewed_model_count": len(model_reviews),
        "reviewed_human_items": len(shared),
    }


def _orient_reviews(
    reviews: list[dict[str, Any]],
    expected_ids: set[str],
    mapping: dict[str, str],
) -> tuple[dict[str, str], list[str]]:
    oriented = {}
    errors = []
    for review in reviews:
        item_id = review.get("item_id")
        preferred = review.get("preferred")
        if item_id not in expected_ids:
            errors.append(f"unexpected_item:{item_id}")
            continue
        if item_id in oriented:
            errors.append(f"duplicate_item:{item_id}")
            continue
        if preferred in {"tie", "both_unusable"}:
            oriented[item_id] = "tie"
            continue
        if preferred not in {"variant_a", "variant_b"}:
            errors.append(f"invalid_preference:{item_id}")
            continue
        arm = mapping.get(f"{item_id}:{preferred}")
        if arm not in {"baseline", "candidate"}:
            errors.append(f"mapping_missing:{item_id}:{preferred}")
            continue
        oriented[item_id] = arm
    missing = expected_ids - set(oriented)
    errors.extend(f"missing_item:{item_id}" for item_id in sorted(missing))
    return oriented, errors
