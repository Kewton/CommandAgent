from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

from eval_lib.goal_verify_main_design_v4 import product_stage, workspace_case_id
from eval_lib.goal_verify_workspaces_v3 import load_workspace_registry


def load_v4_workspace_registry(
    *, root: Path, contract: dict[str, Any]
) -> dict[str, Any]:
    base_relative = contract["workspace_registry"]
    base_path = root / base_relative
    base = load_workspace_registry(base_path)
    additions_relative = contract.get("workspace_registry_additions")
    if not additions_relative:
        return base

    additions = load_workspace_registry(root / additions_relative)
    frozen_base = additions.get("base_registry", {})
    if frozen_base.get("path") != base_relative:
        raise ValueError("v4 workspace additions base path mismatch")
    if frozen_base.get("sha256") != _sha256_file(base_path):
        raise ValueError("v4 workspace additions base hash mismatch")

    workspaces = list(base["workspaces"])
    seen = {row.get("case_id") for row in workspaces}
    for workspace in additions["workspaces"]:
        case_id = workspace.get("case_id")
        if case_id in seen:
            raise ValueError(f"duplicate v4 workspace addition:{case_id}")
        seen.add(case_id)
        workspaces.append(workspace)
    return {
        **base,
        "schema_version": additions.get(
            "resolved_schema_version",
            "commandagent.goal_verify.real_workspaces.v4",
        ),
        "status": additions.get("status", base.get("status")),
        "workspaces": workspaces,
    }


def selected_product_workspace_errors(
    *, root: Path, contract: dict[str, Any], registry: dict[str, Any]
) -> list[str]:
    errors = []
    by_case = {row.get("case_id"): row for row in registry.get("workspaces", [])}
    corpus_by_case = {}
    corpus_value = contract.get("corpus")
    if isinstance(corpus_value, str) and corpus_value:
        try:
            corpus = json.loads((root / corpus_value).read_text(encoding="utf-8"))
            corpus_by_case = {
                row.get("case_id"): row
                for row in corpus.get("cases", [])
                if isinstance(row, dict)
            }
        except (OSError, TypeError, ValueError) as error:
            errors.append(f"selected_product_corpus_invalid:{error}")
    for selected in contract.get("selected_cells", []):
        case_id = selected.get("case_id")
        intent = selected.get("intent")
        if intent not in {"create", "fix", "investigate"}:
            errors.append(f"selected_cell_intent_invalid:{case_id}:{intent}")
            continue
        try:
            case = corpus_by_case.get(case_id, selected)
            desired = product_stage(case)
            workspace_id = workspace_case_id(case)
        except (KeyError, TypeError, ValueError) as error:
            errors.append(f"selected_product_case_invalid:{case_id}:{error}")
            continue
        workspace = by_case.get(workspace_id)
        if workspace is None:
            suffix = f":{workspace_id}" if workspace_id != case_id else ""
            errors.append(f"selected_product_workspace_missing:{case_id}{suffix}")
            continue
        if workspace.get("intent") != intent:
            errors.append(f"selected_product_intent_mismatch:{case_id}")
        if desired not in workspace.get("stages", {}):
            errors.append(f"selected_product_stage_missing:{case_id}:{desired}")
        else:
            try:
                fixture_root = (root / workspace["root"]).resolve()
                stage_path = (fixture_root / desired).resolve()
            except (KeyError, OSError):
                errors.append(
                    f"selected_product_stage_directory_missing:{case_id}:{desired}"
                )
            else:
                if (
                    not stage_path.is_relative_to(fixture_root)
                    or not stage_path.is_dir()
                ):
                    errors.append(
                        f"selected_product_stage_directory_missing:{case_id}:{desired}"
                    )
        if workspace.get("product_run", {}).get("initial_stage") != desired:
            errors.append(
                f"selected_product_stage_contract_mismatch:{case_id}:{desired}"
            )
    return errors


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()
