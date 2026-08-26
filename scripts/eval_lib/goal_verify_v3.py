from __future__ import annotations

import copy
import hashlib
import json
import re
from pathlib import Path
from typing import Any

from eval_lib.goal_verify_v2 import (
    CLAIM_KINDS,
    INPUT_KINDS,
    OBSERVATION_KINDS,
    STRATEGIES,
    _binding_hash,
    _evidence_registry,
    _oracle_id,
)

LANES = ("contract_conformance", "held_out_synthesis")
_SAFE_ID_PART = re.compile(r"[^A-Za-z0-9_.-]+")


def load_prompt_from_contract(
    *, root: Path, contract: dict[str, Any], cli_prompt: Path | None = None
) -> tuple[Path, str]:
    configured = (root / contract["generation"]["prompt"]).resolve()
    if cli_prompt is not None and cli_prompt.resolve() != configured:
        raise ValueError("CLI prompt differs from contract.generation.prompt")
    return configured, configured.read_text(encoding="utf-8")


def _vocabulary() -> dict[str, tuple[str, ...]]:
    return {
        "claim.kind": CLAIM_KINDS,
        "oracle.strategy": STRATEGIES,
        "oracle.input.kind": INPUT_KINDS,
        "oracle.observation.kind": OBSERVATION_KINDS,
    }


def _generation(request_id: str) -> dict[str, str]:
    return {
        "provider": "ollama-local",
        "model": "set-by-caller",
        "request_id": request_id,
        "raw_response_sha256": "",
    }


def _case_adapters(
    case: dict[str, Any], adapters: list[dict[str, Any]]
) -> list[dict[str, Any]]:
    return [row for row in adapters if row["case_id"] == case["case_id"]]


def build_conformance_prompt(
    base_prompt: str,
    case: dict[str, Any],
    request_id: str,
    shape_example: str,
    *,
    adapters: list[dict[str, Any]],
) -> str:
    rows = _case_adapters(case, adapters)
    by_claim: dict[str, list[dict[str, Any]]] = {}
    for row in rows:
        by_claim.setdefault(row["claim_id"], []).append(row)
    required = []
    for claim in case["required_claims"]:
        entries = by_claim.get(claim["id"], [])
        required.append(
            {
                **copy.deepcopy(claim),
                "expected_observations": [
                    {
                        "adapter_id": row["adapter_id"],
                        "strategies": row["proposal"]["strategies"],
                        "polarities": row["proposal"]["polarities"],
                        "observation_kinds": row["proposal"]["observation_kinds"],
                    }
                    for row in entries
                ],
            }
        )
    request: dict[str, Any] = {
        "lane": "contract_conformance",
        "goal": case["goal"],
        "intent": case["intent"],
        "profile": case["profile"],
        "required_claims": required,
        "closed_vocabulary": _vocabulary(),
        "generation": _generation(request_id),
    }
    registry = _evidence_registry(case)
    if registry:
        request["existing_evidence_registry"] = registry
    return _finish_prompt(base_prompt, shape_example, request)


def build_held_out_prompt(
    base_prompt: str,
    case: dict[str, Any],
    request_id: str,
    shape_example: str,
    *,
    capabilities: dict[str, Any],
) -> str:
    strategies = sorted(
        {
            strategy
            for case_row in capabilities.get("cases", [])
            for row in case_row.get("claims", [])
            if row.get("executor_status") != "unavailable"
            for strategy in row.get("strategies", [])
        }
    )
    request: dict[str, Any] = {
        "lane": "held_out_synthesis",
        "goal": case["goal"],
        "intent": case["intent"],
        "profile": case["profile"],
        "executor_capabilities": strategies,
        "closed_vocabulary": _vocabulary(),
        "generation": _generation(request_id),
    }
    registry = _evidence_registry(case)
    if registry:
        request["existing_evidence_registry"] = [
            {key: value for key, value in row.items() if key != "claim_id"}
            for row in registry
        ]
    return _finish_prompt(base_prompt, shape_example, request)


def _finish_prompt(
    base_prompt: str, shape_example: str, request: dict[str, Any]
) -> str:
    return (
        f"{base_prompt.rstrip()}\n\n"
        "The following object is a shape example only. Copy its structure, not "
        "its values.\n"
        f"SHAPE EXAMPLE:\n{shape_example.rstrip()}\n\n"
        "Return JSON only.\n"
        f"INPUT JSON:\n{json.dumps(request, ensure_ascii=False, sort_keys=True)}\n"
    )


def effective_prompt_sha256(prompt: str) -> str:
    return hashlib.sha256(prompt.encode("utf-8")).hexdigest()


def _stable_claim_id(claim: dict[str, Any], index: int) -> str:
    requirement = str(claim.get("normalized_requirement", "claim"))
    prefix = _SAFE_ID_PART.sub("-", requirement.lower()).strip("-.")[:32]
    digest = hashlib.sha256(requirement.encode("utf-8")).hexdigest()[:8]
    return f"held-{prefix or 'claim'}-{index:02d}-{digest}"[:64]


def canonicalize_held_out_proposal(
    raw: str, *, case: dict[str, Any], model: str, request_id: str
) -> str:
    value = json.loads(raw)
    if not isinstance(value, dict):
        raise TypeError("provider proposal must be an object")
    claims = value.get("claims")
    oracles = value.get("oracles")
    if not isinstance(claims, list) or not claims or not isinstance(oracles, list):
        raise TypeError("claims and oracles must be non-empty arrays")
    old_ids = [claim.get("id") for claim in claims if isinstance(claim, dict)]
    if len(old_ids) != len(claims) or any(
        not isinstance(item, str) for item in old_ids
    ):
        raise ValueError("held-out claims require temporary provider IDs")
    if len(set(old_ids)) != len(old_ids):
        raise ValueError("held-out provider claim IDs must be unique")
    canonical = copy.deepcopy(value)
    mapping: dict[str, str] = {}
    for index, claim in enumerate(canonical["claims"], 1):
        new_id = _stable_claim_id(claim, index)
        mapping[claim["id"]] = new_id
        claim["id"] = new_id
        claim["required"] = True
        claim["origin"] = {
            "source_kind": "goal",
            "start_byte": 0,
            "end_byte": len(case["goal"].encode("utf-8")),
        }
        claim["oracle_ids"] = []
    by_id = {claim["id"]: claim for claim in canonical["claims"]}
    counts: dict[str, int] = {}
    ordered = []
    for oracle in canonical["oracles"]:
        if not isinstance(oracle, dict) or oracle.get("claim_id") not in mapping:
            raise ValueError("held-out oracle references an unknown provider claim")
        claim_id = mapping[oracle["claim_id"]]
        counts[claim_id] = counts.get(claim_id, 0) + 1
        oracle["claim_id"] = claim_id
        oracle["id"] = _oracle_id(claim_id, counts[claim_id])
        oracle["observed_strength"] = None
        oracle["lifecycle"] = "proposed"
        oracle["result"] = "unverified"
        digest = _binding_hash(oracle)
        oracle["lineage"] = {
            "proposed_binding_sha256": digest,
            "concretized_binding_sha256": digest,
            "semantic_equivalence": True,
            "repair_kind": None,
        }
        by_id[claim_id]["oracle_ids"].append(oracle["id"])
        ordered.append(oracle)
    if any(not claim["oracle_ids"] for claim in canonical["claims"]):
        raise ValueError("every held-out claim must have at least one oracle")
    canonical["goal"] = case["goal"]
    canonical["intent"] = case["intent"]
    canonical["profile"] = case["profile"]
    canonical["oracles"] = ordered
    canonical["generation"] = {
        "provider": "ollama-local",
        "model": model,
        "request_id": request_id,
        "raw_response_sha256": hashlib.sha256(raw.encode("utf-8")).hexdigest(),
    }
    return json.dumps(canonical, ensure_ascii=False, separators=(",", ":"))


def should_regenerate(validation: dict[str, Any], attempt: int) -> bool:
    if attempt != 1 or validation.get("valid"):
        return False
    errors = validation.get("errors", [])
    return bool(errors) and all(
        isinstance(error, str)
        and not error.startswith(("provider_error", "policy_rejected"))
        for error in errors
    )


def regeneration_seed(base_seed: int, pair_index: int, lane: str, attempt: int) -> int:
    if lane not in LANES or attempt not in (1, 2):
        raise ValueError("invalid lane or regeneration attempt")
    lane_offset = 0 if lane == "contract_conformance" else 1000
    return base_seed + pair_index + lane_offset + (attempt - 1)
