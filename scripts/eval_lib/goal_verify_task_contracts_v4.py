from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path
from typing import Any

_A9_SCHEMA_VERSION = "commandagent.goal_verify.task_contracts.v4_a9"
_A9_ROOT_FIELDS = {
    "schema_version",
    "status",
    "supersedes",
    "policy",
    "validation",
    "cases",
}
_A9_CASE_FIELDS = {
    "case_id",
    "source_goal_sha256",
    "goal",
    "goal_role",
    "operational_constraints",
    "registered_claims",
    "offline_dependencies",
    "completion_contract",
    "dropped_from_a5_execution_goal",
    "decision",
}
_A9_CONSTRAINT_FIELDS = {
    "allowed_dependencies",
    "artifact",
    "do_not_convert_to",
    "do_not_substitute",
    "entry_paths",
    "extra_tools",
    "forbidden_dependencies",
    "framework",
    "frozen_regression_set",
    "install_from_network",
    "language",
    "network",
    "provisioning",
    "registered_reproducer_fixture",
    "reproducer",
    "reserved_port",
    "scored",
    "shell",
    "stdlib_only",
    "styling",
    "unavailable_dependencies",
}
_A9_COMPLETION_FIELDS = {
    "required_paths",
    "verify_commands",
    "profile",
    "goal",
    "required_capabilities",
    "deterministic_oracles",
    "required_evidence",
    "evidence_hint_tokens",
    "required_obligations",
    "deferred_verify_requirements",
    "verify_repair_cap",
}
_STRING_LIST_CONSTRAINTS = {
    "allowed_dependencies",
    "do_not_convert_to",
    "do_not_substitute",
    "entry_paths",
    "extra_tools",
    "forbidden_dependencies",
    "unavailable_dependencies",
}
_STRING_CONSTRAINTS = {
    "artifact",
    "framework",
    "install_from_network",
    "language",
    "network",
    "provisioning",
    "shell",
    "styling",
}
_SHELL_PROGRAMS = {"bash", "dash", "fish", "sh", "zsh"}


def load_task_contract_registry(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict) or not isinstance(value.get("cases"), list):
        raise TypeError("invalid v4 task contract registry")
    rows = value["cases"]
    case_ids = [row.get("case_id") for row in rows if isinstance(row, dict)]
    if len(case_ids) != len(rows) or len(set(case_ids)) != len(case_ids):
        raise ValueError("task contract case IDs must be present and unique")
    if value.get("schema_version") == _A9_SCHEMA_VERSION:
        errors = task_contract_registry_errors(value)
        if errors:
            raise ValueError("task contract a9 invalid:" + ",".join(errors))
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
    execution_goal = (
        row.get("goal")
        if registry.get("schema_version") == _A9_SCHEMA_VERSION
        else row.get("execution_goal")
    )
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
    if registry.get("schema_version") == _A9_SCHEMA_VERSION:
        expected_claims = [claim.get("id") for claim in case.get("required_claims", [])]
        if row.get("registered_claims") != expected_claims:
            raise ValueError(
                f"task contract registered claims mismatch:{case['case_id']}"
            )
        consistency_errors = _a9_case_binding_errors(case, row)
        if consistency_errors:
            raise ValueError(consistency_errors[0])
        bound["task_contract"]["operational_constraints"] = copy.deepcopy(
            row["operational_constraints"]
        )
    return bound


def task_contract_registry_errors(registry: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    unknown_root = sorted(set(registry) - _A9_ROOT_FIELDS)
    if unknown_root:
        errors.append("unknown_root_fields:" + "+".join(unknown_root))
    if registry.get("schema_version") != _A9_SCHEMA_VERSION:
        errors.append("schema_version_invalid")
    if registry.get("status") not in {"draft", "frozen"}:
        errors.append("status_invalid")
    if not _nonempty_string(registry.get("supersedes")):
        errors.append("supersedes_missing")
    if not _string_mapping(registry.get("policy")):
        errors.append("policy_missing")
    if not _string_mapping(registry.get("validation")):
        errors.append("validation_missing")
    for row in registry.get("cases", []):
        if not isinstance(row, dict):
            errors.append("case_not_object")
            continue
        case_id = row.get("case_id", "unknown")
        unknown = sorted(set(row) - _A9_CASE_FIELDS)
        if unknown:
            errors.append(f"unknown_case_fields:{case_id}:" + "+".join(unknown))
        if not _nonempty_string(row.get("goal")):
            errors.append(f"goal_missing:{case_id}")
        if not _nonempty_string(case_id):
            errors.append("case_id_invalid")
        if not _nonempty_string(row.get("goal_role")):
            errors.append(f"goal_role_missing:{case_id}")
        digest = row.get("source_goal_sha256")
        if (
            not isinstance(digest, str)
            or len(digest) != 64
            or any(char not in "0123456789abcdef" for char in digest)
        ):
            errors.append(f"source_goal_sha256_invalid:{case_id}")
        claims = row.get("registered_claims")
        if not _nonempty_string_list(claims) or len(claims) != len(set(claims)):
            errors.append(f"registered_claims_invalid:{case_id}")
        offline = row.get("offline_dependencies")
        if not isinstance(offline, list) or not all(
            _nonempty_string(item) for item in offline
        ):
            errors.append(f"offline_dependencies_invalid:{case_id}")
        dropped = row.get("dropped_from_a5_execution_goal")
        if not _nonempty_string_list(dropped, allow_empty=True):
            errors.append(f"dropped_requirements_invalid:{case_id}")
        if "decision" in row and not _nonempty_string(row["decision"]):
            errors.append(f"decision_invalid:{case_id}")
        errors.extend(
            _operational_constraint_errors(case_id, row.get("operational_constraints"))
        )
        errors.extend(
            _completion_contract_errors(case_id, row.get("completion_contract"))
        )
    return errors


def _operational_constraint_errors(case_id: str, value: Any) -> list[str]:
    if not isinstance(value, dict):
        return [f"operational_constraints_missing:{case_id}"]
    errors = []
    unknown = sorted(set(value) - _A9_CONSTRAINT_FIELDS)
    if unknown:
        errors.append(f"operational_constraints_unknown:{case_id}:" + "+".join(unknown))
    if value.get("scored") is not False:
        errors.append(f"operational_constraints_scored:{case_id}")
    for field in _STRING_LIST_CONSTRAINTS:
        if field in value and not _nonempty_string_list(value[field], allow_empty=True):
            errors.append(f"operational_constraints_type:{case_id}:{field}")
    for field in _STRING_CONSTRAINTS:
        if field in value and not _nonempty_string(value[field]):
            errors.append(f"operational_constraints_type:{case_id}:{field}")
    for field in ("stdlib_only",):
        if field in value and not isinstance(value[field], bool):
            errors.append(f"operational_constraints_type:{case_id}:{field}")
    port = value.get("reserved_port")
    if port is not None and (
        not isinstance(port, int) or isinstance(port, bool) or not 1 <= port <= 65535
    ):
        errors.append(f"operational_constraints_type:{case_id}:reserved_port")
    for path in value.get("entry_paths", []):
        if not _safe_relative(path):
            errors.append(f"operational_constraints_path:{case_id}:{path}")
    fixture = value.get("registered_reproducer_fixture")
    if fixture is not None and not _safe_relative(fixture):
        errors.append(
            f"operational_constraints_path:{case_id}:registered_reproducer_fixture"
        )
    allowed = set(value.get("allowed_dependencies", []))
    forbidden = set(value.get("forbidden_dependencies", []))
    if allowed & forbidden:
        errors.append(f"operational_constraints_dependency_overlap:{case_id}")
    reproducer = value.get("reproducer")
    if reproducer is not None:
        expected = {
            "argv",
            "expected_exit_before",
            "expected_exit_after",
            "stage_before",
        }
        if not isinstance(reproducer, dict) or set(reproducer) != expected:
            errors.append(f"operational_constraints_reproducer_shape:{case_id}")
        elif (
            _direct_argv_error(reproducer.get("argv"))
            or not all(
                isinstance(reproducer.get(field), int)
                and not isinstance(reproducer.get(field), bool)
                for field in ("expected_exit_before", "expected_exit_after")
            )
            or reproducer.get("stage_before") != "before"
        ):
            errors.append(f"operational_constraints_reproducer_invalid:{case_id}")
    regression = value.get("frozen_regression_set")
    if regression is not None:
        if not isinstance(regression, list) or not regression:
            errors.append(f"operational_constraints_regression_invalid:{case_id}")
        else:
            ids = []
            for row in regression:
                if not isinstance(row, dict) or set(row) != {"id", "argv"}:
                    errors.append(f"operational_constraints_regression_shape:{case_id}")
                    continue
                ids.append(row.get("id"))
                if not _nonempty_string(row.get("id")) or _direct_argv_error(
                    row.get("argv")
                ):
                    errors.append(
                        f"operational_constraints_regression_invalid:{case_id}"
                    )
            if len(ids) != len(set(ids)):
                errors.append(f"operational_constraints_regression_duplicate:{case_id}")
    return errors


def _completion_contract_errors(case_id: str, value: Any) -> list[str]:
    if not isinstance(value, dict):
        return [f"completion_contract_missing:{case_id}"]
    errors = []
    unknown = sorted(set(value) - _A9_COMPLETION_FIELDS)
    if unknown:
        errors.append(f"completion_contract_unknown:{case_id}:" + "+".join(unknown))
    string_list_fields = (
        "required_paths",
        "verify_commands",
        "required_capabilities",
        "deterministic_oracles",
        "required_evidence",
        "evidence_hint_tokens",
        "required_obligations",
    )
    for field in string_list_fields:
        if not _nonempty_string_list(value.get(field), allow_empty=True):
            errors.append(f"completion_contract_type:{case_id}:{field}")
    if not isinstance(value.get("deferred_verify_requirements"), list):
        errors.append(
            f"completion_contract_type:{case_id}:deferred_verify_requirements"
        )
    for path in value.get("required_paths", []):
        if not _safe_relative(path):
            errors.append(f"completion_contract_path:{case_id}:{path}")
    if not all(_nonempty_string(item) for item in value.get("verify_commands", [])):
        errors.append(f"completion_contract_verify_commands:{case_id}")
    if not _nonempty_string(value.get("profile")) or not _nonempty_string(
        value.get("goal")
    ):
        errors.append(f"completion_contract_identity:{case_id}")
    cap = value.get("verify_repair_cap")
    if not isinstance(cap, int) or isinstance(cap, bool) or cap < 0:
        errors.append(f"completion_contract_verify_repair_cap:{case_id}")
    return errors


def _a9_case_binding_errors(case: dict[str, Any], row: dict[str, Any]) -> list[str]:
    case_id = case["case_id"]
    completion = row["completion_contract"]
    constraints = row["operational_constraints"]
    errors = []
    if completion.get("profile") != case.get("profile"):
        errors.append(f"completion contract profile mismatch:{case_id}")
    required_paths = set(completion.get("required_paths", []))
    for path in constraints.get("entry_paths", []):
        if path not in required_paths:
            errors.append(f"completion contract entry path missing:{case_id}:{path}")
    fixture = constraints.get("registered_reproducer_fixture")
    if fixture is not None and fixture not in required_paths:
        errors.append(f"completion contract reproducer fixture missing:{case_id}")
    for path in constraints.get("do_not_substitute", []):
        if _safe_relative(path) and path not in required_paths:
            errors.append(
                f"completion contract protected path missing:{case_id}:{path}"
            )
    verify_commands = set(completion.get("verify_commands", []))
    reproducer = constraints.get("reproducer")
    if (
        isinstance(reproducer, dict)
        and " ".join(reproducer["argv"]) not in verify_commands
    ):
        errors.append(f"completion contract reproducer command missing:{case_id}")
    for regression in constraints.get("frozen_regression_set", []):
        command = " ".join(regression["argv"])
        if command not in verify_commands:
            errors.append(
                f"completion contract regression command missing:{case_id}:{regression['id']}"
            )
    allowed = set(constraints.get("allowed_dependencies", []))
    offline = set(row.get("offline_dependencies", []))
    if not allowed <= offline:
        errors.append(f"completion contract allowed dependency unavailable:{case_id}")
    return errors


def _direct_argv_error(argv: Any) -> str | None:
    if not isinstance(argv, list) or not argv:
        return "argv_missing"
    if any(not _nonempty_string(item) or "\x00" in item for item in argv):
        return "argv_invalid"
    if Path(argv[0]).name in _SHELL_PROGRAMS or Path(argv[0]).is_absolute():
        return "argv_program_unsafe"
    if any(
        any(operator in item for operator in ("&&", "||", "$(", "`", ";"))
        for item in argv
    ):
        return "argv_shell_syntax_unsafe"
    if any(Path(item).is_absolute() or ".." in Path(item).parts for item in argv[1:]):
        return "argv_path_unsafe"
    return None


def _safe_relative(value: Any) -> bool:
    return (
        _nonempty_string(value)
        and not Path(value).is_absolute()
        and ".." not in Path(value).parts
        and "\x00" not in value
    )


def _nonempty_string(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())


def _nonempty_string_list(value: Any, *, allow_empty: bool = False) -> bool:
    return (
        isinstance(value, list)
        and (allow_empty or bool(value))
        and all(_nonempty_string(item) for item in value)
    )


def _string_mapping(value: Any) -> bool:
    return (
        isinstance(value, dict)
        and bool(value)
        and all(
            _nonempty_string(key) and _nonempty_string(item)
            for key, item in value.items()
        )
    )


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
            row = _investigation_registry_row(claim_id, requirement_id, path, value)
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
        row for row in documents if row[1].get("requirement_id") == requirement_id
    ]
    if direct:
        return direct[-1]
    adjudication = []
    for row in documents:
        value = row[1].get("adjudication")
        statuses = (
            value.get("requirement_statuses") if isinstance(value, dict) else None
        )
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
