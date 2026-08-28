from __future__ import annotations

import copy
import hashlib
import json
import random
import re
from typing import Any

AXES = (
    "requirement_clear",
    "input_observation_expected_specific",
    "executable_from_visible_information",
    "false_positive_or_overconstraint_risk_acceptable",
    "semantic_duplication_absent",
)
VERDICTS = ("acceptable", "needs_revision", "unusable")


def canonical_sha256(value: Any) -> str:
    payload = json.dumps(
        value, ensure_ascii=False, separators=(",", ":"), sort_keys=True
    ).encode()
    return hashlib.sha256(payload).hexdigest()


def prepare_semantic_items(
    *, records: list[dict[str, Any]], contract_sha256: str
) -> tuple[list[dict[str, Any]], dict[str, dict[str, Any]]]:
    seed = int(contract_sha256[:16], 16)
    rng = random.Random(seed)
    items = []
    mapping = {}
    for record in records:
        for lane_name, lane in sorted(record["lanes"].items()):
            proposal = _raw_proposal(lane)
            claims = proposal.get("claims", [])
            oracles = proposal.get("oracles", [])
            if not isinstance(claims, list) or not isinstance(oracles, list):
                raise TypeError(
                    f"raw claims/oracles are not lists:{record['pair_id']}:{lane_name}"
                )
            claimed_oracle_indexes = set()
            for index, claim in enumerate(claims, 1):
                if not isinstance(claim, dict):
                    raise TypeError(
                        f"raw claim is not an object:{record['pair_id']}:{lane_name}"
                    )
                claim_id = claim.get("id")
                linked_indexes = [
                    oracle_index
                    for oracle_index, oracle in enumerate(oracles)
                    if isinstance(oracle, dict) and oracle.get("claim_id") == claim_id
                ]
                linked = [oracles[oracle_index] for oracle_index in linked_indexes]
                claimed_oracle_indexes.update(linked_indexes)
                _append_item(
                    items=items,
                    mapping=mapping,
                    contract_sha256=contract_sha256,
                    record=record,
                    lane_name=lane_name,
                    source_index=index,
                    raw_claim=claim,
                    raw_oracles=linked,
                    source_oracle_indexes=linked_indexes,
                    group_kind="claim_group",
                )
            orphan_index = len(claims)
            for oracle_index, oracle in enumerate(oracles):
                if not isinstance(oracle, dict):
                    raise TypeError(
                        f"raw oracle is not an object:{record['pair_id']}:{lane_name}"
                    )
                if oracle_index in claimed_oracle_indexes:
                    continue
                orphan_index += 1
                _append_item(
                    items=items,
                    mapping=mapping,
                    contract_sha256=contract_sha256,
                    record=record,
                    lane_name=lane_name,
                    source_index=orphan_index,
                    raw_claim=None,
                    raw_oracles=[oracle],
                    source_oracle_indexes=[oracle_index],
                    group_kind="orphan_oracle",
                )
            if not claims and not oracles:
                _append_item(
                    items=items,
                    mapping=mapping,
                    contract_sha256=contract_sha256,
                    record=record,
                    lane_name=lane_name,
                    source_index=1,
                    raw_claim=None,
                    raw_oracles=[],
                    source_oracle_indexes=[],
                    group_kind="empty_proposal",
                )
    rng.shuffle(items)
    return items, mapping


def human_sample(
    *,
    items: list[dict[str, Any]],
    mapping: dict[str, dict[str, Any]],
    sample_spec: dict[str, Any] | None = None,
) -> list[str]:
    if sample_spec is not None:
        return _stratified_main_sample(
            items=items,
            mapping=mapping,
            sample_spec=sample_spec,
        )
    by_case: dict[str, list[str]] = {}
    for item in items:
        item_id = item["item_id"]
        case_id = mapping[item_id]["source_case_id"]
        by_case.setdefault(case_id, []).append(item_id)
    selected = [min(values) for _, values in sorted(by_case.items())]
    for _, values in sorted(by_case.items()):
        remaining = [item_id for item_id in sorted(values) if item_id not in selected]
        if remaining:
            selected.append(remaining[0])
        if len(selected) == 10:
            break
    if len(selected) != 10:
        raise ValueError(f"human sample requires 10 items, found {len(selected)}")
    order = {item["item_id"]: index for index, item in enumerate(items)}
    return sorted(selected, key=order.__getitem__)


def _stratified_main_sample(
    *,
    items: list[dict[str, Any]],
    mapping: dict[str, dict[str, Any]],
    sample_spec: dict[str, Any],
) -> list[str]:
    sample_size = sample_spec.get("size")
    per_cell = sample_spec.get("items_per_cell")
    source_lane = sample_spec.get("source_lane")
    if (
        not isinstance(sample_size, int)
        or isinstance(sample_size, bool)
        or sample_size < 1
        or not isinstance(per_cell, int)
        or isinstance(per_cell, bool)
        or per_cell < 1
        or not isinstance(source_lane, str)
        or not source_lane
    ):
        raise ValueError("main semantic review sample spec is invalid")
    by_cell_task: dict[str, dict[str, list[str]]] = {}
    for item in items:
        item_id = item["item_id"]
        source = mapping[item_id]
        if source.get("source_lane") != source_lane:
            continue
        cell_id = source.get("cell_id")
        task_id = source.get("source_task_id")
        if not isinstance(cell_id, str) or not isinstance(task_id, str):
            raise TypeError(f"main semantic item lacks cluster metadata:{item_id}")
        by_cell_task.setdefault(cell_id, {}).setdefault(task_id, []).append(item_id)
    selected = []
    for cell_id, tasks in sorted(by_cell_task.items()):
        task_selections = [min(item_ids) for _, item_ids in sorted(tasks.items())]
        if len(task_selections) < per_cell:
            raise ValueError(
                f"main semantic sample lacks tasks:{cell_id}:{len(task_selections)}"
            )
        selected.extend(sorted(task_selections)[:per_cell])
    if len(selected) != sample_size:
        raise ValueError(
            f"main semantic sample requires {sample_size} items, found {len(selected)}"
        )
    order = {item["item_id"]: index for index, item in enumerate(items)}
    return sorted(selected, key=order.__getitem__)


def independent_human_template(
    *, items_sha256: str, human_items: list[dict[str, Any]]
) -> dict[str, Any]:
    return {
        "items_sha256": items_sha256,
        "human_items_sha256": canonical_sha256(human_items),
        "reviewer_id": "",
        "reviewer_type": "human",
        "contract_authoring_involvement": None,
        "independence_confirmed": False,
        "item_ids": [item["item_id"] for item in human_items],
        "reviews": [blank_review_row(item["item_id"]) for item in human_items],
    }


def authorized_ai_reviewer_template(
    *,
    items_sha256: str,
    human_items: list[dict[str, Any]],
    reviewer_policy: dict[str, Any],
) -> dict[str, Any]:
    authorization = reviewer_policy.get("authorized_ai_reviewer")
    if not isinstance(authorization, dict):
        raise TypeError("reviewer policy has no authorized AI reviewer")
    required = (
        "authorization_id",
        "authorization_scope",
        "authorized_at",
        "authorized_by",
        "reviewer_id",
        "provider",
        "model_family",
        "model_id_or_version",
        "contract_authoring_involvement",
    )
    missing = [field for field in required if field not in authorization]
    if missing:
        raise ValueError(f"authorized AI reviewer policy is incomplete:{missing}")
    return {
        "items_sha256": items_sha256,
        "human_items_sha256": canonical_sha256(human_items),
        "reviewer_id": authorization["reviewer_id"],
        "reviewer_type": "ai",
        "provider": authorization["provider"],
        "model_family": authorization["model_family"],
        "model_id_or_version": authorization["model_id_or_version"],
        "contract_authoring_involvement": authorization[
            "contract_authoring_involvement"
        ],
        "user_authorized": True,
        "authorization_id": authorization["authorization_id"],
        "authorization_scope": authorization["authorization_scope"],
        "authorized_at": authorization["authorized_at"],
        "authorized_by": authorization["authorized_by"],
        "source_blind_confirmed": False,
        "forbidden_materials_not_accessed": False,
        "reviewer_output_independence_confirmed": False,
        "invoked_at": "",
        "item_ids": [item["item_id"] for item in human_items],
        "reviews": [blank_review_row(item["item_id"]) for item in human_items],
    }


def blank_review_row(item_id: str) -> dict[str, Any]:
    return {
        "item_id": item_id,
        "verdict": "",
        "axes": {axis: None for axis in AXES},
        "reason_codes": [],
        "rationale": "",
    }


def build_blind_report(
    *,
    items: list[dict[str, Any]],
    model_documents: list[dict[str, Any]],
    human_document: dict[str, Any],
    human_items: list[dict[str, Any]],
    reviewer_policy: dict[str, Any] | None = None,
    review_contract: dict[str, Any] | None = None,
) -> dict[str, Any]:
    item_ids = [item["item_id"] for item in items]
    items_sha256 = canonical_sha256(items)
    expected_human_ids = [item["item_id"] for item in human_items]
    master_by_id = {item["item_id"]: item for item in items}
    sample_spec = (
        review_contract.get("main_sample")
        if isinstance(review_contract, dict)
        else None
    )
    expected_sample_size = (
        sample_spec.get("size") if isinstance(sample_spec, dict) else 10
    )
    human_items_match_master = (
        len(human_items) == expected_sample_size
        and len(expected_human_ids) == len(set(expected_human_ids))
        and all(master_by_id.get(item["item_id"]) == item for item in human_items)
    )
    models = [
        validate_model_review(
            document=document,
            expected_item_ids=item_ids,
            items_sha256=items_sha256,
        )
        for document in model_documents
    ]
    valid_models = [model for model in models if model["valid"]]
    families = sorted({model["model_family"] for model in valid_models})
    human = validate_human_review(
        document=human_document,
        expected_item_ids=expected_human_ids,
        items_sha256=items_sha256,
        human_items_sha256=canonical_sha256(human_items),
        reviewer_policy=reviewer_policy,
    )
    agreement = _model_agreement(valid_models, item_ids)
    public_models = [
        {key: value for key, value in model.items() if key != "reviews"}
        for model in models
    ]
    calibration_review_complete = human["valid"]
    model_reviews_required = not (
        isinstance(review_contract, dict)
        and review_contract.get("model_reviews_required") is False
    )
    checks = {
        "all_items_have_stable_ids": len(item_ids) == len(set(item_ids)),
        "all_model_reviews_valid": len(valid_models) == len(models),
        "at_least_two_model_reviews": not model_reviews_required
        or len(valid_models) >= 2,
        "distinct_model_families": not model_reviews_required or len(families) >= 2,
        "human_sample_is_ten": sample_spec is not None or len(expected_human_ids) == 10,
        "human_sample_size_matches_contract": len(expected_human_ids)
        == expected_sample_size,
        "human_items_match_master": human_items_match_master,
        "human_review_complete": (
            calibration_review_complete and human["reviewer_type"] == "human"
        ),
        "calibration_review_complete": calibration_review_complete,
    }
    semantic_checks = {
        key: value for key, value in checks.items() if key != "human_review_complete"
    }
    return {
        "schema_version": "commandagent.goal_verify.semantic_blind_report.v4",
        "items_sha256": items_sha256,
        "item_count": len(items),
        "review_sample_size": expected_sample_size,
        "model_reviews_required": model_reviews_required,
        "model_reviews": public_models,
        "distinct_model_families": families,
        "human_review": human,
        "calibration_review": human,
        "agreement": agreement,
        "checks": checks,
        "semantic_review_complete": all(semantic_checks.values()),
    }


def validate_model_review(
    *,
    document: dict[str, Any],
    expected_item_ids: list[str],
    items_sha256: str,
) -> dict[str, Any]:
    reviewer = document.get("reviewer")
    if not isinstance(reviewer, dict):
        reviewer = {
            "provider": document.get("provider"),
            "model_id_or_version": document.get("model_id_or_version"),
            "model_family": document.get("model_family"),
            "invoked_at": document.get("invoked_at"),
            "independent": document.get("independent"),
        }
    errors = []
    if document.get("items_sha256") != items_sha256:
        errors.append("items_sha256_mismatch")
    for field in ("provider", "model_id_or_version", "model_family", "invoked_at"):
        if not isinstance(reviewer.get(field), str) or not reviewer[field]:
            errors.append(f"reviewer_{field}_missing")
    if reviewer.get("independent") is not True:
        errors.append("reviewer_independence_not_confirmed")
    invocation_script_sha256 = document.get("invocation_script_sha256")
    agent_without_script = (
        reviewer.get("provider") == "openai-codex-agent"
        and invocation_script_sha256 == "not_applicable_agent_review"
    )
    if not _is_sha256(invocation_script_sha256) and not agent_without_script:
        errors.append("invocation_script_sha256_invalid")
    rows = _review_rows(document)
    row_errors = _review_row_errors(rows, expected_item_ids)
    errors.extend(row_errors)
    counts = {verdict: 0 for verdict in VERDICTS}
    axis_pass_counts = {axis: 0 for axis in AXES}
    if not row_errors:
        for row in rows:
            counts[row["verdict"]] += 1
            for axis in AXES:
                axis_pass_counts[axis] += int(row["axes"][axis])
    total = len(rows)
    return {
        "provider": reviewer.get("provider"),
        "model_id_or_version": reviewer.get("model_id_or_version"),
        "model_family": reviewer.get("model_family"),
        "invoked_at": reviewer.get("invoked_at"),
        "independent": reviewer.get("independent"),
        "document_sha256": canonical_sha256(document),
        "invocation_script_sha256": invocation_script_sha256,
        "invocation_script_sha256_history": document.get(
            "invocation_script_sha256_history", []
        ),
        "review_count": total,
        "verdict_counts": counts,
        "axis_pass_counts": axis_pass_counts,
        "valid": not errors,
        "errors": errors,
        "reviews": rows if not row_errors else [],
    }


def validate_human_review(
    *,
    document: dict[str, Any],
    expected_item_ids: list[str],
    items_sha256: str,
    human_items_sha256: str,
    reviewer_policy: dict[str, Any] | None = None,
) -> dict[str, Any]:
    errors = []
    if document.get("items_sha256") != items_sha256:
        errors.append("items_sha256_mismatch")
    if document.get("human_items_sha256") != human_items_sha256:
        errors.append("human_items_sha256_mismatch")
    if not isinstance(document.get("reviewer_id"), str) or not document["reviewer_id"]:
        errors.append("reviewer_id_missing")
    reviewer_type = document.get("reviewer_type")
    allowed_types = (
        reviewer_policy.get("allowed_reviewer_types", ["human"])
        if isinstance(reviewer_policy, dict)
        else ["human"]
    )
    if reviewer_type not in allowed_types:
        if allowed_types == ["human"]:
            errors.append("reviewer_type_must_be_human")
            if document.get("contract_authoring_involvement") is not False:
                errors.append("contract_authoring_involvement_must_be_false")
            if document.get("independence_confirmed") is not True:
                errors.append("reviewer_independence_not_confirmed")
        else:
            errors.append("reviewer_type_not_allowed")
    elif reviewer_type == "human":
        if document.get("contract_authoring_involvement") is not False:
            errors.append("contract_authoring_involvement_must_be_false")
        if document.get("independence_confirmed") is not True:
            errors.append("reviewer_independence_not_confirmed")
    elif reviewer_type == "ai":
        errors.extend(
            _authorized_ai_reviewer_errors(
                document=document,
                reviewer_policy=reviewer_policy,
            )
        )
    if document.get("item_ids") != expected_item_ids:
        errors.append("human_sample_ids_mismatch")
    rows = document.get("reviews", [])
    if not isinstance(rows, list):
        rows = []
        errors.append("reviews_not_array")
    errors.extend(_review_row_errors(rows, expected_item_ids))
    counts = {verdict: 0 for verdict in VERDICTS}
    axis_pass_counts = {axis: 0 for axis in AXES}
    if not errors:
        for row in rows:
            counts[row["verdict"]] += 1
            for axis in AXES:
                axis_pass_counts[axis] += int(row["axes"][axis])
    return {
        "reviewer_id": document.get("reviewer_id"),
        "reviewer_type": document.get("reviewer_type"),
        "provider": document.get("provider"),
        "model_family": document.get("model_family"),
        "model_id_or_version": document.get("model_id_or_version"),
        "contract_authoring_involvement": document.get(
            "contract_authoring_involvement"
        ),
        "independence_confirmed": document.get("independence_confirmed"),
        "user_authorized": document.get("user_authorized"),
        "authorization_id": document.get("authorization_id"),
        "authorization_scope": document.get("authorization_scope"),
        "authorized_at": document.get("authorized_at"),
        "authorized_by": document.get("authorized_by"),
        "source_blind_confirmed": document.get("source_blind_confirmed"),
        "forbidden_materials_not_accessed": document.get(
            "forbidden_materials_not_accessed"
        ),
        "reviewer_output_independence_confirmed": document.get(
            "reviewer_output_independence_confirmed"
        ),
        "invoked_at": document.get("invoked_at"),
        "document_sha256": canonical_sha256(document),
        "review_count": len(rows),
        "verdict_counts": counts,
        "axis_pass_counts": axis_pass_counts,
        "valid": not errors,
        "errors": errors,
    }


def _authorized_ai_reviewer_errors(
    *, document: dict[str, Any], reviewer_policy: dict[str, Any] | None
) -> list[str]:
    if not isinstance(reviewer_policy, dict):
        return ["authorized_ai_reviewer_policy_missing"]
    authorization = reviewer_policy.get("authorized_ai_reviewer")
    if not isinstance(authorization, dict):
        return ["authorized_ai_reviewer_policy_missing"]
    errors = []
    expected_fields = (
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
    for field in expected_fields:
        if document.get(field) != authorization.get(field):
            errors.append(f"authorized_ai_{field}_mismatch")
    if document.get("user_authorized") is not True:
        errors.append("authorized_ai_user_authorization_missing")
    if document.get("source_blind_confirmed") is not True:
        errors.append("authorized_ai_source_blind_not_confirmed")
    if document.get("forbidden_materials_not_accessed") is not True:
        errors.append("authorized_ai_forbidden_materials_boundary_not_confirmed")
    if document.get("reviewer_output_independence_confirmed") is not True:
        errors.append("authorized_ai_output_independence_not_confirmed")
    if not isinstance(document.get("invoked_at"), str) or not document["invoked_at"]:
        errors.append("authorized_ai_invoked_at_missing")
    return errors


def _review_rows(document: dict[str, Any]) -> list[dict[str, Any]]:
    rows = document.get("parsed_reviews", document.get("reviews", []))
    if isinstance(rows, dict):
        rows = rows.get("reviews", [])
    return rows if isinstance(rows, list) else []


def _review_row_errors(
    rows: list[dict[str, Any]], expected_item_ids: list[str]
) -> list[str]:
    errors = []
    actual_ids = [row.get("item_id") for row in rows if isinstance(row, dict)]
    if len(rows) != len(expected_item_ids):
        errors.append(
            f"review_count_mismatch:expected={len(expected_item_ids)}:actual={len(rows)}"
        )
    if len(actual_ids) != len(rows) or set(actual_ids) != set(expected_item_ids):
        errors.append("review_item_ids_mismatch")
    if len(actual_ids) != len(set(actual_ids)):
        errors.append("review_item_ids_duplicated")
    for row in rows:
        if not isinstance(row, dict):
            errors.append("review_row_not_object")
            continue
        item_id = row.get("item_id", "unknown")
        if row.get("verdict") not in VERDICTS:
            errors.append(f"invalid_verdict:{item_id}")
        axes = row.get("axes")
        if not isinstance(axes, dict) or set(axes) != set(AXES):
            errors.append(f"invalid_axes:{item_id}")
        elif not all(isinstance(axes[axis], bool) for axis in AXES):
            errors.append(f"non_boolean_axis:{item_id}")
        if not isinstance(row.get("reason_codes"), list):
            errors.append(f"reason_codes_not_array:{item_id}")
        if not isinstance(row.get("rationale"), str) or not row["rationale"].strip():
            errors.append(f"rationale_missing:{item_id}")
    return errors


def _is_sha256(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _model_agreement(
    valid_models: list[dict[str, Any]], item_ids: list[str]
) -> dict[str, Any] | None:
    if len(valid_models) != 2:
        return None
    left = {row["item_id"]: row for row in valid_models[0]["reviews"]}
    right = {row["item_id"]: row for row in valid_models[1]["reviews"]}
    exact = sum(
        left[item_id]["verdict"] == right[item_id]["verdict"] for item_id in item_ids
    )
    observed = exact / len(item_ids) if item_ids else 0.0
    left_rates = {
        verdict: sum(left[item_id]["verdict"] == verdict for item_id in item_ids)
        / len(item_ids)
        for verdict in VERDICTS
    }
    right_rates = {
        verdict: sum(right[item_id]["verdict"] == verdict for item_id in item_ids)
        / len(item_ids)
        for verdict in VERDICTS
    }
    expected = sum(left_rates[value] * right_rates[value] for value in VERDICTS)
    kappa = (observed - expected) / (1.0 - expected) if expected < 1.0 else 1.0
    axis_exact = {
        axis: sum(
            left[item_id]["axes"][axis] == right[item_id]["axes"][axis]
            for item_id in item_ids
        )
        / len(item_ids)
        for axis in AXES
    }
    return {
        "model_ids": [model["model_id_or_version"] for model in valid_models],
        "verdict_exact_count": exact,
        "verdict_exact_rate": observed,
        "cohen_kappa": kappa,
        "axis_exact_rates": axis_exact,
    }


def _raw_proposal(lane: dict[str, Any]) -> dict[str, Any]:
    attempts = lane.get("attempts", [])
    if not attempts:
        return {}
    response = attempts[-1].get("response", {})
    if response.get("status") != "completed":
        return {}
    raw = response.get("response", {}).get("response")
    if not isinstance(raw, str):
        return {}
    try:
        proposal = json.loads(raw)
    except json.JSONDecodeError:
        return {}
    if not isinstance(proposal, dict):
        return {}
    if not isinstance(proposal.get("claims", []), list) or not isinstance(
        proposal.get("oracles", []), list
    ):
        return {}
    return proposal


def _append_item(
    *,
    items: list[dict[str, Any]],
    mapping: dict[str, dict[str, Any]],
    contract_sha256: str,
    record: dict[str, Any],
    lane_name: str,
    source_index: int,
    raw_claim: dict[str, Any] | None,
    raw_oracles: list[dict[str, Any]],
    source_oracle_indexes: list[int],
    group_kind: str,
) -> None:
    source = f"{contract_sha256}:{record['pair_id']}:{lane_name}:{source_index}"
    item_id = f"item-{hashlib.sha256(source.encode()).hexdigest()[:20]}"
    item = {
        "item_id": item_id,
        "goal": record["goal"],
        "intent": record["intent"],
        "profile": record["profile"],
        "required_claims": [
            {"id": claim["id"]} for claim in record.get("required_claims", [])
        ],
        "group_kind": group_kind,
        "raw_claim": copy.deepcopy(raw_claim),
        "raw_oracles": copy.deepcopy(raw_oracles),
    }
    item["item_sha256"] = canonical_sha256(item)
    items.append(item)
    mapping[item_id] = {
        "pair_id": record["pair_id"],
        "source_case_id": record["source_case_id"],
        "cell_id": record.get("cell_id"),
        "source_task_id": record.get("source_task_id"),
        "source_lane": lane_name,
        "source_index": source_index,
        "source_oracle_indexes": source_oracle_indexes,
        "group_kind": group_kind,
    }
