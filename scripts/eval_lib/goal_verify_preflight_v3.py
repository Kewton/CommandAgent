from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from eval_lib.goal_verify_sandbox import sandbox_backend_status
from eval_lib.goal_verify_workspaces_v3 import (
    validate_provisioning,
    validate_workspace_registry,
)


def load_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise TypeError(f"expected JSON object: {path}")
    return value


def exact_sha_ci_evidence_errors(
    *, root: Path, contract: dict[str, Any]
) -> list[str]:
    evidence_value = contract.get("exact_sha_ci_evidence")
    if not isinstance(evidence_value, str) or not evidence_value:
        return ["exact_sha_ci_evidence_missing"]
    evidence_path = root / evidence_value
    if not evidence_path.is_file():
        return ["exact_sha_ci_evidence_missing"]
    try:
        evidence = load_json(evidence_path)
    except (OSError, TypeError, json.JSONDecodeError):
        return ["exact_sha_ci_evidence_invalid"]

    code_sha = contract.get("code_sha")
    if not isinstance(code_sha, str) or not code_sha:
        return []
    errors: list[str] = []
    if evidence.get("head_sha") != code_sha:
        errors.append("exact_sha_ci_evidence_sha_mismatch")
    workflows = evidence.get("workflows")
    if not isinstance(workflows, list):
        return [*errors, "exact_sha_ci_evidence_workflows_invalid"]
    by_name = {
        row.get("name"): row
        for row in workflows
        if isinstance(row, dict) and isinstance(row.get("name"), str)
    }
    for name in ("CI", "acceptance"):
        row = by_name.get(name)
        if row is None:
            errors.append(f"exact_sha_ci_workflow_missing:{name}")
            continue
        if row.get("status") != "completed" or row.get("conclusion") != "success":
            errors.append(f"exact_sha_ci_workflow_not_successful:{name}")
        workflow_sha = row.get("head_sha")
        if workflow_sha is not None and workflow_sha != code_sha:
            errors.append(f"exact_sha_ci_workflow_sha_mismatch:{name}")
    return errors


def cross_source_errors(*, root: Path, contract: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    generation = contract.get("generation", {})
    prompt = root / str(generation.get("prompt", ""))
    if not prompt.is_file():
        errors.append("prompt_missing")
    else:
        text = prompt.read_text(encoding="utf-8")
        for requirement in (
            "http_status",
            "JSON integer",
            "file",
            "one oracle",
            "shell command",
            "expected_polarity",
            "Host code computes UTF-8 origins",
        ):
            if requirement not in text:
                errors.append(f"prompt_rule_missing:{requirement}")
    budget = load_json(root / contract["resource_budget_config"])
    expected = budget["resource_budget_registration"]["values"]
    actual = {
        key: value
        for key, value in contract["resource_budgets"].items()
        if key != "preflight_use"
    }
    if actual != expected:
        errors.append("resource_budget_mismatch")
    corpus = load_json(root / "eval/goal_verify/v0/corpus.json")
    adapters = load_json(root / contract["oracle_execution"]["adapter_registry"])
    capabilities = load_json(
        root / "eval/goal_verify/v0/phase6-execution-capabilities-v3.json"
    )
    selected = {row["case_id"] for row in contract["selected_cells"]}
    corpus_by_case = {row["case_id"]: row for row in corpus["cases"]}
    if selected - set(corpus_by_case):
        errors.append("selected_case_missing_from_corpus")
    adapter_claims = {(row["case_id"], row["claim_id"]) for row in adapters["adapters"]}
    capability_claims = {
        (case["case_id"], claim["claim_id"])
        for case in capabilities["cases"]
        for claim in case["claims"]
    }
    corpus_claims = {
        (case_id, claim["id"])
        for case_id in selected
        for claim in corpus_by_case[case_id]["required_claims"]
    }
    if adapter_claims != corpus_claims:
        errors.append("adapter_corpus_claim_set_mismatch")
    if capability_claims != corpus_claims:
        errors.append("capability_corpus_claim_set_mismatch")
    capability_status = {
        (case["case_id"], claim["claim_id"]): claim["executor_status"]
        for case in capabilities["cases"]
        for claim in case["claims"]
    }
    for adapter in adapters["adapters"]:
        key = (adapter["case_id"], adapter["claim_id"])
        adapter_status = str(adapter["executor"].get("executor_status", "")).split()[0]
        if adapter_status != capability_status.get(key):
            errors.append(f"executor_status_mismatch:{adapter['adapter_id']}")
    workspaces = load_json(root / contract["oracle_execution"]["workspace_registry"])
    errors.extend(validate_workspace_registry(root=root, registry=workspaces))
    if any(
        row["executor"].get("kind", "").startswith("snapshot")
        for row in adapters["adapters"]
    ):
        errors.append("primary_snapshot_executor_present")
    contract_ids = {
        contract["contract_id"],
        adapters["contract_id"],
        capabilities["contract_id"],
        workspaces["contract_id"],
    }
    if len(contract_ids) != 1:
        errors.append("contract_id_mismatch")
    return errors


def readiness_report(
    *, root: Path, contract_path: Path, execution_root: Path | None = None
) -> dict[str, Any]:
    contract = load_json(contract_path)
    errors = cross_source_errors(root=root, contract=contract)
    blockers = list(errors)
    if contract.get("status") != "frozen":
        blockers.append("contract_not_frozen")
    if not contract.get("code_sha"):
        blockers.append("exact_code_sha_missing")
    blockers.extend(exact_sha_ci_evidence_errors(root=root, contract=contract))
    if not contract.get("authorization", {}).get("approved_live"):
        blockers.append("live_preflight_not_authorized")
    if contract.get("generation", {}).get("seed_base") is None:
        blockers.append("seed_base_missing")
    blind = load_json(root / contract["semantic_blind_review_contract"])
    if blind.get("contract_id") != contract.get("contract_id"):
        blockers.append("blind_contract_id_mismatch")
    if blind.get("status") != "frozen":
        blockers.append("blind_contract_not_frozen")
    planned = blind.get("reviewers", {}).get("model", {}).get(
        "planned_reviewers", []
    )
    if len(planned) < 2 or len({row.get("model_family") for row in planned}) < 2:
        blockers.append("blind_model_reviewers_not_fixed")
    human = blind.get("reviewers", {}).get("human", {})
    if not human.get("assigned_reviewer_id"):
        blockers.append("independent_human_reviewer_missing")
    if human.get("independence_confirmed") is not True:
        blockers.append("human_reviewer_independence_unconfirmed")
    adapters = load_json(root / contract["oracle_execution"]["adapter_registry"])
    pending = sorted(
        row["adapter_id"]
        for row in adapters["adapters"]
        if str(row.get("executor", {}).get("executor_status", "")).startswith(
            "adaptation_required"
        )
    )
    blockers.extend(f"executor_adaptation_pending:{adapter_id}" for adapter_id in pending)
    workspace_registry = load_json(
        root / contract["oracle_execution"]["workspace_registry"]
    )
    blockers.extend(
        validate_workspace_registry(
            root=root, registry=workspace_registry, require_frozen=True
        )
    )
    blockers.extend(
        validate_provisioning(
            workspace_registry,
            execution_root / "provisioned" if execution_root is not None else None,
        )
    )
    for workspace in workspace_registry["workspaces"]:
        if workspace.get("status") != "frozen":
            blockers.append(f"workspace_not_frozen:{workspace['case_id']}")
        if not workspace.get("frozen_file_sha256"):
            blockers.append(f"workspace_hash_missing:{workspace['case_id']}")
    sandbox = sandbox_backend_status()
    if not sandbox["available"]:
        blockers.append(f"sandbox_backend_unavailable:{sandbox['reason']}")
    return {
        "contract_id": contract.get("contract_id"),
        "status": contract.get("status"),
        "cross_source_equivalence": not errors,
        "errors": errors,
        "sandbox": sandbox,
        "pending_executor_adapters": pending,
        "blockers": sorted(set(blockers)),
        "ready": not blockers,
    }
