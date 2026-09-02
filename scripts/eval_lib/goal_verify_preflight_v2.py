from __future__ import annotations

import json
from pathlib import Path
from typing import Any


def _load(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise TypeError(f"expected JSON object: {path}")
    return value


def assess_v2_readiness(*, root: Path, contract_path: Path) -> dict[str, Any]:
    contract = _load(contract_path)
    blockers = []
    if contract.get("status") != "frozen":
        blockers.append("contract_not_frozen")
    code_sha = contract.get("code_sha")
    if not isinstance(code_sha, str) or len(code_sha) != 40:
        blockers.append("exact_code_sha_missing")
    ci_reference = contract.get("exact_sha_ci_evidence")
    if not isinstance(ci_reference, str) or not (root / ci_reference).is_file():
        blockers.append("exact_sha_ci_evidence_missing")
    if contract.get("authorization", {}).get("approved_live") is not True:
        blockers.append("live_preflight_not_authorized")

    proposal = contract.get("proposal_contract", {})
    selected_intents = proposal.get("selected_intents", [])
    corpus = _load(root / "eval/goal_verify/v0/corpus.json")
    selected_cases = [
        case for case in corpus["cases"] if case.get("intent") in selected_intents
    ]
    expected_pairs = len(selected_cases) * contract.get("samples_per_cell", 0)
    acceptance_pairs = (
        contract.get("acceptance", {})
        .get("schema_compliance", {})
        .get("denominator_pairs")
    )
    if expected_pairs != acceptance_pairs:
        blockers.append("scope_pair_count_mismatch")

    production = contract.get("artifact_production", {})
    if production.get("enabled") is not True:
        blockers.append("candidate_artifact_production_missing")
    task_manifest = production.get("task_manifest")
    if not isinstance(task_manifest, str) or not (root / task_manifest).is_file():
        blockers.append("task_workspace_manifest_missing")
    if not production.get("same_scope_baseline_raw"):
        blockers.append("same_scope_raw_baseline_missing")

    execution = contract.get("oracle_execution", {})
    if execution.get("enabled") is not True:
        blockers.append("candidate_oracle_execution_disabled")
    adapter_reference = execution.get("adapter_registry")
    if (
        not isinstance(adapter_reference, str)
        or not (root / adapter_reference).is_file()
    ):
        blockers.append("adapter_registry_missing")
        adapter_count = 0
    else:
        adapters = _load(root / adapter_reference).get("adapters", [])
        adapter_count = len(adapters) if isinstance(adapters, list) else 0
        if adapter_count == 0:
            blockers.append("registered_command_adapters_empty")

    blind_reference = contract.get("semantic_blind_review_contract")
    if not isinstance(blind_reference, str) or not (root / blind_reference).is_file():
        blockers.append("semantic_blind_review_contract_missing")

    return {
        "schema_version": "commandagent.goal_verify.preflight_readiness.v2",
        "ready": not blockers,
        "contract_id": contract.get("contract_id"),
        "selected_case_count": len(selected_cases),
        "expected_pair_count": expected_pairs,
        "registered_adapter_count": adapter_count,
        "blockers": blockers,
    }
