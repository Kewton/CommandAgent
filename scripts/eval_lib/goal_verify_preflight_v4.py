from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from eval_lib.goal_verify_preflight_v3 import exact_sha_ci_evidence_errors
from eval_lib.goal_verify_sandbox import sandbox_backend_status
from eval_lib.goal_verify_task_contracts_v4 import (
    load_task_contract_registry,
    selected_task_contract_errors,
)
from eval_lib.goal_verify_workspaces_v3 import (
    validate_provisioning,
    validate_workspace_registry,
)
from eval_lib.goal_verify_workspaces_v4 import (
    load_v4_workspace_registry,
    selected_product_workspace_errors,
)


def load_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise TypeError(f"expected object: {path}")
    return value


def design_errors(*, root: Path, contract: dict[str, Any]) -> list[str]:
    errors = []
    comparison = contract.get("comparison", {})
    pairing = contract.get("pairing", {})
    boundary = contract.get("information_boundary", {})
    execution = contract.get("execution", {})
    full = contract.get("full_experiment", {})
    if comparison.get("baseline") != "current_product_evidence":
        errors.append("baseline_not_current_product_evidence")
    if comparison.get("candidate") != "baseline_plus_candidate_evidence":
        errors.append("candidate_not_additive")
    if comparison.get("task_execution_count_per_pair") != 1:
        errors.append("task_execution_not_shared")
    if comparison.get("baseline_failure_override_allowed") is not False:
        errors.append("baseline_failure_override_not_forbidden")
    if pairing.get("candidate_execution_target") != "product_snapshot":
        errors.append("candidate_target_not_product_snapshot")
    if pairing.get("reference_or_after_fallback") != "forbidden":
        errors.append("reference_fallback_not_forbidden")
    if boundary.get("gold_use") != "scoring_only_after_candidate_execution":
        errors.append("gold_execution_leak")
    if execution.get("same_snapshot_hash_required") is not True:
        errors.append("same_snapshot_not_required")
    if execution.get("candidate_primary_reference_fallback_max_count") != 0:
        errors.append("reference_fallback_gate_missing")
    if execution.get("gold_used_for_execution_max_count") != 0:
        errors.append("gold_execution_gate_missing")
    if full.get("cells") != 12 or full.get("minimum_total_pairs") != 360:
        errors.append("full_experiment_sample_size_changed")
    if full.get("bootstrap_samples") != 2000:
        errors.append("bootstrap_count_changed")
    prompt = root / contract.get("generation", {}).get("prompt", "")
    schema = root / contract.get("generation", {}).get(
        "structured_output_schema", ""
    )
    if not schema.is_file():
        errors.append("structured_output_schema_missing")
    answer_key_path = root / contract.get("scoring", {}).get("answer_key", "")
    if not answer_key_path.is_file():
        errors.append("answer_key_missing")
    if not prompt.is_file():
        errors.append("prompt_missing")
    else:
        text = prompt.read_text(encoding="utf-8")
        for marker in (
            "baseline observation",
            "workspace_manifest",
            "without a shell",
            "Gold adapters",
            "scoring after execution",
        ):
            if marker not in text:
                errors.append(f"prompt_rule_missing:{marker}")
    task_registry = contract.get("task_contract_registry")
    corpus_path = contract.get("corpus")
    if task_registry or corpus_path:
        if not task_registry:
            errors.append("task_contract_registry_missing")
        if not corpus_path:
            errors.append("contract_corpus_missing")
        if task_registry and corpus_path:
            try:
                registry = load_task_contract_registry(root / task_registry)
                corpus = load_json(root / corpus_path)
            except (OSError, TypeError, ValueError) as error:
                errors.append(f"task_contract_registry_invalid:{error}")
            else:
                errors.extend(
                    selected_task_contract_errors(
                        corpus=corpus, contract=contract, registry=registry
                    )
                )
                try:
                    answer_key = load_json(answer_key_path)
                except (OSError, TypeError, ValueError) as error:
                    errors.append(f"answer_key_invalid:{error}")
                else:
                    adapter_rows = answer_key.get("adapters", [])
                    adapter_ids = [
                        row.get("adapter_id")
                        for row in adapter_rows
                        if isinstance(row, dict)
                    ]
                    if len(adapter_ids) != len(adapter_rows) or len(
                        set(adapter_ids)
                    ) != len(adapter_ids):
                        errors.append("answer_key_adapter_ids_invalid")
                    selected_ids = {
                        row.get("case_id") for row in contract.get("selected_cells", [])
                    }
                    expected_claims = {
                        (case["case_id"], claim["id"])
                        for case in corpus.get("cases", [])
                        if case.get("case_id") in selected_ids
                        for claim in case.get("required_claims", [])
                    }
                    actual_claims = {
                        (row.get("case_id"), row.get("claim_id"))
                        for row in adapter_rows
                        if row.get("case_id") in selected_ids
                    }
                    if actual_claims != expected_claims:
                        errors.append("answer_key_claim_set_mismatch")
    claim_policy = contract.get("claim_policy")
    if claim_policy is not None and (
        claim_policy.get("allow_unverifiable_reason") is not True
        or claim_policy.get("passing_credit") is not False
        or claim_policy.get("scoring")
        != "retain claim in denominator as unverified"
    ):
        errors.append("unverifiable_claim_policy_invalid")
    budget = load_json(root / contract["resource_budget_config"])
    expected = budget["resource_budget_registration"]["values"]
    actual = {
        key: value
        for key, value in contract["resource_budgets"].items()
        if key != "preflight_use"
    }
    if actual != expected:
        errors.append("resource_budget_mismatch")
    return errors


def readiness_report(
    *, root: Path, contract_path: Path, execution_root: Path | None = None
) -> dict[str, Any]:
    contract = load_json(contract_path)
    blockers = design_errors(root=root, contract=contract)
    if contract.get("status") != "frozen":
        blockers.append("contract_not_frozen")
    if not contract.get("code_sha"):
        blockers.append("exact_code_sha_missing")
    blockers.extend(exact_sha_ci_evidence_errors(root=root, contract=contract))
    if contract.get("authorization", {}).get("live_collection_authorized") is not True:
        blockers.append("live_collection_not_authorized")
    try:
        workspace_registry = load_v4_workspace_registry(root=root, contract=contract)
    except (KeyError, OSError, TypeError, ValueError) as error:
        blockers.append(f"workspace_registry_load_failed:{error}")
    else:
        blockers.extend(
            validate_workspace_registry(
                root=root, registry=workspace_registry, require_frozen=True
            )
        )
        blockers.extend(
            selected_product_workspace_errors(
                root=root, contract=contract, registry=workspace_registry
            )
        )
        blockers.extend(
            validate_provisioning(
                workspace_registry,
                execution_root / "provisioned"
                if execution_root is not None
                else None,
            )
        )
    sandbox = sandbox_backend_status()
    if not sandbox["available"]:
        blockers.append(f"sandbox_backend_unavailable:{sandbox['reason']}")
    return {
        "contract_id": contract.get("contract_id"),
        "design_errors": design_errors(root=root, contract=contract),
        "sandbox": sandbox,
        "blockers": sorted(set(blockers)),
        "ready": not blockers,
    }
