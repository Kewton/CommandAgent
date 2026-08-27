from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path
from typing import Any


def load_task_contract_registry(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict) or not isinstance(value.get("cases"), list):
        raise TypeError("invalid v4 task contract registry")
    rows = value["cases"]
    case_ids = [row.get("case_id") for row in rows if isinstance(row, dict)]
    if len(case_ids) != len(rows) or len(set(case_ids)) != len(case_ids):
        raise ValueError("task contract case IDs must be present and unique")
    return value


def bind_task_contract(
    case: dict[str, Any], registry: dict[str, Any]
) -> dict[str, Any]:
    rows = {row["case_id"]: row for row in registry["cases"]}
    row = rows.get(case["case_id"])
    if row is None:
        raise ValueError(f"task contract missing:{case['case_id']}")
    expected = hashlib.sha256(case["goal"].encode("utf-8")).hexdigest()
    if row.get("source_goal_sha256") != expected:
        raise ValueError(f"task contract source goal mismatch:{case['case_id']}")
    execution_goal = row.get("execution_goal")
    completion_contract = row.get("completion_contract")
    if not isinstance(execution_goal, str) or not execution_goal.strip():
        raise ValueError(f"task execution goal missing:{case['case_id']}")
    if not isinstance(completion_contract, dict):
        raise TypeError(f"completion contract missing:{case['case_id']}")
    bound = copy.deepcopy(case)
    bound["source_goal"] = case["goal"]
    bound["goal"] = execution_goal
    bound["task_contract"] = {
        "schema_version": registry.get("schema_version"),
        "completion_contract": copy.deepcopy(completion_contract),
        "offline_dependencies": copy.deepcopy(row.get("offline_dependencies", [])),
    }
    return bound


def selected_task_contract_errors(
    *, corpus: dict[str, Any], contract: dict[str, Any], registry: dict[str, Any]
) -> list[str]:
    by_case = {row.get("case_id"): row for row in corpus.get("cases", [])}
    errors = []
    for selected in contract.get("selected_cells", []):
        case_id = selected.get("case_id")
        case = by_case.get(case_id)
        if case is None:
            errors.append(f"task_contract_corpus_case_missing:{case_id}")
            continue
        try:
            bind_task_contract(case, registry)
        except (KeyError, TypeError, ValueError) as error:
            errors.append(str(error))
    return errors


def bind_existing_evidence_registry(
    case: dict[str, Any], workspace: Path
) -> dict[str, Any]:
    if case.get("intent") not in {"fix", "investigate"}:
        return case
    requirement_by_claim = {
        "before-after": "after_passes",
        "regressions": "no_regression",
        "exact-reproducer": "before_fails",
        "after-executed": "after_passes",
        "full-regression-set": "no_regression",
        "bug-reproducer": "after_passes",
        "reproducer-defect": "reproducer_fails",
        "location-exists": "diagnosis_bound",
        "causal-intervention": "diagnosis_bound",
        "intent-unresolved": "diagnosis_bound",
        "timeout-boundary": "reproducer_fails",
    }
    documents = _evidence_documents(workspace)
    registry = []
    for claim in case.get("required_claims", []):
        claim_id = claim["id"]
        requirement_id = requirement_by_claim.get(claim_id)
        match = _matching_evidence_document(documents, requirement_id)
        if match is None:
            continue
        path, value = match
        if case["intent"] == "fix":
            row = _fix_registry_row(claim_id, requirement_id, path, value)
        else:
            row = _investigation_registry_row(
                claim_id, requirement_id, path, value
            )
        if row is not None:
            registry.append(row)
    bound = copy.deepcopy(case)
    bound["existing_evidence_registry"] = registry
    return bound


def _evidence_documents(workspace: Path) -> list[tuple[str, dict[str, Any]]]:
    evidence_root = workspace / "evidence"
    if not evidence_root.is_dir():
        return []
    documents = []
    for path in sorted(evidence_root.glob("*.json")):
        try:
            value = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, UnicodeDecodeError, json.JSONDecodeError):
            continue
        if isinstance(value, dict):
            documents.append((path.relative_to(workspace).as_posix(), value))
    return documents


def _matching_evidence_document(documents, requirement_id):
    direct = [
        row
        for row in documents
        if row[1].get("requirement_id") == requirement_id
    ]
    if direct:
        return direct[-1]
    adjudication = []
    for row in documents:
        value = row[1].get("adjudication")
        statuses = value.get("requirement_statuses") if isinstance(value, dict) else None
        if isinstance(statuses, dict) and statuses.get(requirement_id) is not None:
            adjudication.append(row)
    return adjudication[-1] if adjudication else None


def _fix_registry_row(claim_id, requirement_id, path, value):
    if value.get("requirement_id") == requirement_id:
        stage = value.get("stage")
        expected = value.get("expected")
        lineage = value.get("lineage")
        epoch = value.get("epoch")
    else:
        evidence = value.get("evidence", {})
        if not isinstance(evidence, dict):
            return None
        run_id = value.get("run_id") or evidence.get("run_id")
        stage = "after"
        expected = "success"
        lineage = f"adjudication:{run_id}:{requirement_id}"
        epochs = [
            row.get("epoch")
            for row in evidence.get("regressions", [])
            if isinstance(row, dict) and isinstance(row.get("epoch"), int)
        ]
        epoch = max(epochs, default=1)
    if (
        stage not in {"before", "after"}
        or expected not in {"success", "failure"}
        or not isinstance(lineage, str)
        or not lineage
        or not isinstance(epoch, int)
        or epoch < 1
    ):
        return None
    return {
        "claim_id": claim_id,
        "artifact_path": path,
        "requirement_id": requirement_id,
        "stage": stage,
        "expected_polarity": expected,
        "lineage": lineage,
        "epoch": epoch,
    }


def _investigation_registry_row(claim_id, requirement_id, path, value):
    binding_id = value.get("binding_id")
    stage = value.get("stage")
    lineage = value.get("lineage")
    epoch = value.get("epoch")
    if (
        not isinstance(binding_id, str)
        or not binding_id
        or stage not in {"reproduce", "diagnosis"}
        or not isinstance(lineage, str)
        or not lineage
        or not isinstance(epoch, int)
        or epoch < 1
    ):
        return None
    return {
        "claim_id": claim_id,
        "artifact_path": path,
        "requirement_id": requirement_id,
        "binding_id": binding_id,
        "stage": stage,
        "lineage": lineage,
        "epoch": epoch,
    }
