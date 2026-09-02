from __future__ import annotations

from collections import Counter, defaultdict
from typing import Any


def workspace_case_id(case: dict[str, Any]) -> str:
    value = case.get("workspace_case_id", case.get("case_id"))
    if not isinstance(value, str) or not value:
        raise ValueError("workspace_case_id must be a non-empty string")
    return value


def product_stage(case: dict[str, Any]) -> str:
    configured = case.get("product_stage")
    if configured is not None:
        if configured not in {"initial", "before"}:
            raise ValueError(
                f"product_stage invalid:{case.get('case_id')}:{configured}"
            )
        return configured
    intent = case.get("intent")
    if intent == "create":
        return "initial"
    if intent == "fix":
        return "before"
    if intent == "investigate":
        return "initial"
    raise ValueError(f"intent invalid:{case.get('case_id')}:{intent}")


def cluster_metadata(case: dict[str, Any]) -> dict[str, str]:
    cell_id = case.get("cell_id")
    source_task_id = case.get("source_task_id")
    for field, value in (
        ("cell_id", cell_id),
        ("source_task_id", source_task_id),
    ):
        if not isinstance(value, str) or not value:
            raise ValueError(f"{field} missing:{case.get('case_id')}")
    return {"cell_id": cell_id, "source_task_id": source_task_id}


def main_design_errors(
    *, corpus: dict[str, Any], contract: dict[str, Any], matrix: dict[str, Any]
) -> list[str]:
    """Validate the frozen 12 x 10 source-task x 3 run design."""
    errors: list[str] = []
    cases = corpus.get("cases", [])
    selected_rows = contract.get("selected_cells", [])
    if not isinstance(cases, list) or not isinstance(selected_rows, list):
        return ["main_corpus_or_selection_invalid"]
    case_ids = [row.get("case_id") for row in cases if isinstance(row, dict)]
    if len(case_ids) != len(cases) or len(set(case_ids)) != len(case_ids):
        errors.append("main_corpus_case_ids_invalid")
    by_case = {
        row["case_id"]: row
        for row in cases
        if isinstance(row, dict) and row.get("case_id")
    }
    selected_ids = [
        row.get("case_id") for row in selected_rows if isinstance(row, dict)
    ]
    if len(selected_ids) != len(selected_rows) or len(set(selected_ids)) != len(
        selected_ids
    ):
        errors.append("main_selected_case_ids_invalid")
    if set(selected_ids) != set(by_case):
        errors.append("main_selection_must_equal_corpus")

    full = contract.get("full_experiment", {})
    expected_cells = int(full.get("cells", 0))
    minimum_tasks = int(full.get("minimum_distinct_source_tasks_per_cell", 0))
    minimum_runs = int(full.get("minimum_runs_per_source_task", 0))
    samples = int(contract.get("samples_per_cell", 0))
    if samples != minimum_runs:
        errors.append("main_samples_per_task_mismatch")
    expected_pairs = expected_cells * minimum_tasks * minimum_runs
    if expected_pairs != int(full.get("minimum_total_pairs", 0)):
        errors.append("main_total_pair_contract_mismatch")
    if len(selected_rows) * samples != expected_pairs:
        errors.append("main_selected_pair_count_mismatch")

    dimensions = matrix.get("dimensions", [])
    matrix_cells = matrix.get("cells", [])
    expected_by_cell = {
        f"cell-{index:02d}": tuple(row.get(field) for field in dimensions)
        for index, row in enumerate(matrix_cells, 1)
    }
    grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
    task_ids: list[str] = []
    for selected in selected_rows:
        if not isinstance(selected, dict):
            continue
        case = by_case.get(selected.get("case_id"))
        if case is None:
            continue
        try:
            metadata = cluster_metadata(case)
            workspace_case_id(case)
            product_stage(case)
        except ValueError as error:
            errors.append(str(error))
            continue
        cell_id = metadata["cell_id"]
        source_task_id = metadata["source_task_id"]
        grouped[cell_id].append(case)
        task_ids.append(source_task_id)
        if selected.get("cell_id") != cell_id:
            errors.append(f"main_selected_cell_mismatch:{case['case_id']}")
        if selected.get("source_task_id") != source_task_id:
            errors.append(f"main_selected_task_mismatch:{case['case_id']}")
        expected_dimensions = expected_by_cell.get(cell_id)
        actual_dimensions = tuple(case.get(field) for field in dimensions)
        if expected_dimensions is None:
            errors.append(f"main_unknown_cell:{cell_id}")
        elif actual_dimensions != expected_dimensions:
            errors.append(f"main_cell_dimensions_mismatch:{case['case_id']}")
    if len(set(task_ids)) != len(task_ids):
        duplicates = sorted(
            task_id for task_id, count in Counter(task_ids).items() if count > 1
        )
        errors.append("main_source_task_ids_duplicate:" + "+".join(duplicates))
    if set(grouped) != set(expected_by_cell):
        errors.append("main_cell_set_mismatch")
    for cell_id in sorted(expected_by_cell):
        rows = grouped.get(cell_id, [])
        if len(rows) != minimum_tasks:
            errors.append(f"main_cell_task_count:{cell_id}:{len(rows)}")
        goals = [row.get("goal") for row in rows]
        if len(set(goals)) != len(goals):
            errors.append(f"main_cell_goals_not_distinct:{cell_id}")
        task_variants = [row.get("task_variant") for row in rows]
        if any(not isinstance(value, dict) or not value for value in task_variants):
            errors.append(f"main_task_variant_missing:{cell_id}")
        fingerprints = [
            tuple(sorted(value.items())) if isinstance(value, dict) else ()
            for value in task_variants
        ]
        if len(set(fingerprints)) != len(fingerprints):
            errors.append(f"main_task_variants_not_distinct:{cell_id}")
    return errors
