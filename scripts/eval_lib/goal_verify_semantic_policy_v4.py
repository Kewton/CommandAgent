from __future__ import annotations

import hashlib
import json
from typing import Any

SEMANTIC_POLICY = {
    "schema_version": "commandagent.goal_verify.semantic_policy.v4_a13",
    "adapter_availability": {
        "unavailable_kind": "unavailable",
        "unavailable_status": "unavailable",
        "effect": "excluded_from_expected_observations_and_scoring",
    },
    "semantic_admissibility": {
        "credit_requires": [
            "unique_available_adapter_match",
            "executed_observation_matches_adapter",
        ],
        "failure_requires": "unique_available_adapter_match",
        "observed_strength": (
            "derived_after_admissibility_from_observation_and_semantic_binding"
        ),
        "generic_investigation_binding": (
            "unavailable_without_explicit_supported_oracle_kind"
        ),
    },
    "failure_mode": "unverified",
}


def semantic_policy_sha256() -> str:
    encoded = json.dumps(
        SEMANTIC_POLICY,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def adapter_executor_available(adapter: dict[str, Any]) -> bool:
    executor = adapter.get("executor")
    if not isinstance(executor, dict):
        return False
    return not (
        executor.get("kind") == "unavailable"
        or executor.get("executor_status") == "unavailable"
    )


def adapter_semantic_admissibility(
    adapter: dict[str, Any], claim: dict[str, Any] | None
) -> tuple[bool, str | None]:
    if not adapter_executor_available(adapter):
        return False, "executor_capability_unavailable"
    if adapter.get("executor", {}).get("kind") != "existing_evidence_probe":
        return True, None
    if claim is None:
        return False, "semantic_claim_missing"
    oracle_kind = claim.get("oracle", {}).get("kind")
    supported = adapter.get("executor", {}).get("supported_oracle_kinds")
    if (
        not isinstance(oracle_kind, str)
        or not isinstance(supported, list)
        or oracle_kind not in supported
    ):
        return False, "semantic_capability_unavailable"
    return True, None


def available_adapters(
    adapters: list[dict[str, Any]], *, claim: dict[str, Any] | None = None
) -> list[dict[str, Any]]:
    return [
        adapter
        for adapter in adapters
        if adapter_semantic_admissibility(adapter, claim)[0]
    ]
