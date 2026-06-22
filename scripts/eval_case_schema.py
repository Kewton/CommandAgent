#!/usr/bin/env python3
"""Shared eval case parsing and focused assertion helpers.

This module is eval-only. Expected fields describe what the report should
observe after a run; they must not be passed into runtime prompts or commands.
"""

from __future__ import annotations

from pathlib import Path


EXPECTED_FIELD_NAMES = [
    "expected_terminal_state",
    "expected_contract_layer",
    "expected_failure_class",
    "expected_violated_contract",
    "expected_diagnostic_code",
    "expected_source_of_truth",
    "expected_producer",
    "expected_guard",
    "expected_actionability",
    "expected_active_job",
    "expected_recovery_owner",
    "expected_dispatch_status",
    "expected_diagnostic_failure_kind",
    "expected_semantic_cluster_source_of_truth",
    "expected_repair_action",
    "expected_target_role",
    "expected_observed_expected",
    "expected_affected_cases",
    "expected_candidate_artifacts",
    "expected_unknown_diagnostic_count",
    "expected_target_source_of_truth",
    "expected_target_ownership_source",
    "expected_target_evidence_freshness",
    "expected_focused_edit_status",
    "expected_target_conflict_reason",
    "expected_runtime_job_kind",
    "expected_runtime_job_outcome",
    "expected_setup_state",
    "expected_dev_server_state",
    "expected_completion_authority_status",
    "expected_freshness_status",
    "expected_evidence_binding_status",
    "expected_completion_evidence_status",
    "expected_attempt_outcome",
    "expected_explicit_stop_reason",
]

ASSERTION_FIELD_NAMES = [
    "expected_assertion_status",
    "expected_assertion_count",
    "expected_assertion_failures",
]


def iter_case_paths(cases_dir: str | Path) -> list[Path]:
    return sorted(Path(cases_dir).rglob("*.yaml"))


def read_eval_case(path: str | Path) -> dict:
    data = {
        "expected_artifacts": [],
        "verify": [],
        "mode": "plan-run",
        "fixture": None,
        "success_check": {
            "required_paths": [],
            "must_include": {},
        },
        "expected_fields": {},
    }
    current_list = None
    in_success_check = False
    in_must_include = False
    current_must_include_path = None
    with open(path, encoding="utf-8") as handle:
        for raw in handle:
            line = raw.rstrip("\n")
            stripped = line.strip()
            if not stripped or stripped.startswith("#"):
                continue
            indent = len(line) - len(line.lstrip(" "))
            if not line.startswith(" ") and ":" in line:
                key, value = line.split(":", 1)
                key = key.strip()
                value = unquote(value.strip())
                in_success_check = key == "success_check"
                in_must_include = False
                current_must_include_path = None
                if key in {
                    "id",
                    "title",
                    "profile",
                    "style",
                    "intent",
                    "prompt",
                    "mode",
                    "fixture",
                }:
                    data[key] = value
                    current_list = None
                elif key in {"expected_artifacts", "verify"}:
                    current_list = key
                elif key in EXPECTED_FIELD_NAMES:
                    data["expected_fields"][key.removeprefix("expected_")] = value
                    current_list = None
                else:
                    current_list = None
            elif in_success_check:
                if indent == 2 and stripped.startswith("type:"):
                    data["success_check"]["type"] = unquote(
                        stripped.split(":", 1)[1].strip()
                    )
                    current_list = None
                    in_must_include = False
                elif indent == 2 and stripped == "required_paths:":
                    current_list = "success_required_paths"
                    in_must_include = False
                elif indent == 2 and stripped == "must_include:":
                    current_list = None
                    in_must_include = True
                    current_must_include_path = None
                elif current_list == "success_required_paths" and stripped.startswith("- "):
                    data["success_check"]["required_paths"].append(
                        unquote(stripped[2:].strip())
                    )
                elif in_must_include and indent == 4 and stripped.endswith(":"):
                    current_must_include_path = unquote(stripped[:-1].strip())
                    data["success_check"]["must_include"].setdefault(
                        current_must_include_path, []
                    )
                elif (
                    in_must_include
                    and indent >= 6
                    and stripped.startswith("- ")
                    and current_must_include_path
                ):
                    data["success_check"]["must_include"][
                        current_must_include_path
                    ].append(unquote(stripped[2:].strip()))
            elif current_list and stripped.startswith("- "):
                data[current_list].append(unquote(stripped[2:].strip()))
    for required in ["id", "profile", "style", "prompt"]:
        if required not in data:
            raise SystemExit(f"{path}: missing required field {required}")
    if data["mode"] not in {"plan-run", "ultra-plan-run"}:
        raise SystemExit(f"{path}: unsupported mode {data['mode']}")
    return data


def unquote(value: str) -> str:
    if len(value) >= 2 and value[0] == value[-1] and value[0] in {'"', "'"}:
        return value[1:-1]
    return value


def focused_assertions(
    expected_fields: dict[str, str],
    observed_fields: dict[str, str],
    *,
    dry_run: bool = False,
    recheck: bool = False,
) -> dict[str, str]:
    if not expected_fields:
        return {
            "expected_assertion_status": "not_configured",
            "expected_assertion_count": "0",
            "expected_assertion_failures": "",
        }
    if dry_run:
        return {
            "expected_assertion_status": "skipped_dry_run",
            "expected_assertion_count": str(len(expected_fields)),
            "expected_assertion_failures": "",
        }

    failures = []
    for field, expected in sorted(expected_fields.items()):
        expected = str(expected).strip()
        if not expected:
            continue
        observed = str(observed_fields.get(field, "")).strip()
        if observed != expected:
            failures.append(f"{field}:expected={expected};observed={observed or '<blank>'}")

    if failures:
        status = "failed_recheck" if recheck else "failed"
    else:
        status = "passed_recheck" if recheck else "passed"
    return {
        "expected_assertion_status": status,
        "expected_assertion_count": str(len(expected_fields)),
        "expected_assertion_failures": " | ".join(failures),
    }
