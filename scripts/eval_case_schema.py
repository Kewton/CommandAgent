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
    "expected_semantic_failure_kind",
    "expected_semantic_cluster_source_of_truth",
    "expected_repair_action",
    "expected_repair_brief_status",
    "expected_action_envelope_status",
    "expected_allowed_change_kind",
    "expected_allowed_tool_category",
    "expected_target_confidence",
    "expected_disallowed_actions",
    "expected_repair_plan_rejection_reason",
    "expected_target_role",
    "expected_target_path",
    "expected_selected_target",
    "expected_target_rejection_reasons",
    "expected_rejected_target_reason",
    "expected_observed_expected",
    "expected_affected_cases",
    "expected_candidate_artifacts",
    "expected_unknown_diagnostic_count",
    "expected_task_contract_kind",
    "expected_task_contract_status",
    "expected_task_contract_lifecycle",
    "expected_task_contract_request_signals",
    "expected_task_contract_constraints",
    "expected_task_contract_completion_evidence",
    "expected_behavior_obligation_codes",
    "expected_behavior_obligation_status",
    "expected_behavior_obligation_owners",
    "expected_behavior_obligation_paths",
    "expected_artifact_role_projection_status",
    "expected_target_source_of_truth",
    "expected_target_ownership_source",
    "expected_target_evidence_freshness",
    "expected_focused_edit_status",
    "expected_target_conflict_reason",
    "expected_runtime_job_kind",
    "expected_runtime_job_outcome",
    "expected_setup_state",
    "expected_setup_manifest_path",
    "expected_setup_artifact_validation_status",
    "expected_dev_server_state",
    "expected_completion_authority_status",
    "expected_evidence_runner_status",
    "expected_evidence_runner_kind",
    "expected_freshness_status",
    "expected_evidence_binding_status",
    "expected_evidence_binding_kind",
    "expected_completion_evidence_status",
    "expected_completion_source_of_truth",
    "expected_missing_evidence",
    "expected_failed_evidence",
    "expected_failed_bindings",
    "expected_stale_evidence",
    "expected_artifact_ledger_status",
    "expected_artifact_ledger_entries",
    "expected_artifact_ledger_summary",
    "expected_artifact_ledger_sources",
    "expected_required_paths",
    "expected_read_paths",
    "expected_changed_paths",
    "expected_created_paths",
    "expected_verifier_mentioned_paths",
    "expected_scaffold_created_paths",
    "expected_setup_created_paths",
    "expected_out_of_scope_paths",
    "expected_workspace_scope_kind",
    "expected_workspace_scope_roots",
    "expected_artifact_ownership",
    "expected_artifact_source_of_truth",
    "expected_deliverable_obligation_kind",
    "expected_deliverable_obligation_path",
    "expected_deliverable_obligation",
    "expected_attempt_outcome",
    "expected_verifier_rerun_result",
    "expected_no_progress_strategy",
    "expected_repair_state_status",
    "expected_explicit_stop_reason",
    "expected_safe_stop_payload",
    "expected_patch_validation_status",
    "expected_tool_protocol_status",
    "expected_tool_protocol_source",
    "expected_tool_protocol_action",
    "expected_tool_protocol_failed_tool",
    "expected_tool_protocol_missing_field",
    "expected_tool_protocol_required_fields",
    "expected_tool_protocol_correction_spent",
    "expected_tool_protocol_correction_exhausted",
    "expected_lifecycle_stage",
    "expected_active_owner",
    "expected_selected_action",
    "expected_target_admission_status",
    "expected_repair_action_plan_status",
    "expected_completion_source",
]

ASSERTION_FIELD_NAMES = [
    "expected_assertion_status",
    "expected_assertion_count",
    "expected_assertion_failures",
]

MATRIX_FIELD_NAMES = [
    "matrix_row",
    "proof_mode",
]

CASE_METADATA_KEYS = {
    "id",
    "title",
    "profile",
    "style",
    "intent",
    "prompt",
    "mode",
    "fixture",
    "matrix_row",
    "proof_mode",
    "fixture_reason",
    "fixture_success",
    "fixture_rc",
    "fixture_stdout",
    "fixture_stderr",
}

PROOF_MODES = {
    "real_llm",
    "deterministic_fixture",
    "report_fixture",
}


def iter_case_paths(cases_dir: str | Path) -> list[Path]:
    return sorted(Path(cases_dir).rglob("*.yaml"))


def read_eval_case(path: str | Path) -> dict:
    data = {
        "expected_artifacts": [],
        "verify": [],
        "mode": "plan-run",
        "fixture": None,
        "matrix_row": "",
        "proof_mode": "real_llm",
        "fixture_fields": {},
        "success_check": {
            "required_paths": [],
            "must_include": {},
        },
        "expected_fields": {},
    }
    current_list = None
    in_success_check = False
    in_must_include = False
    in_fixture_fields = False
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
                in_fixture_fields = key == "fixture_fields"
                in_must_include = False
                current_must_include_path = None
                if key in CASE_METADATA_KEYS:
                    data[key] = value
                    current_list = None
                elif key in {"expected_artifacts", "verify"}:
                    current_list = key
                elif key == "fixture_fields":
                    current_list = None
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
            elif in_fixture_fields and indent == 2 and ":" in stripped:
                key, value = stripped.split(":", 1)
                data["fixture_fields"][key.strip()] = unquote(value.strip())
            elif current_list and stripped.startswith("- "):
                data[current_list].append(unquote(stripped[2:].strip()))
    for required in ["id", "profile", "style", "prompt"]:
        if required not in data:
            raise SystemExit(f"{path}: missing required field {required}")
    if data["mode"] not in {"plan-run", "ultra-plan-run"}:
        raise SystemExit(f"{path}: unsupported mode {data['mode']}")
    if not data["matrix_row"]:
        data["matrix_row"] = data["id"]
    if data["proof_mode"] not in PROOF_MODES:
        raise SystemExit(f"{path}: unsupported proof_mode {data['proof_mode']}")
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
        if not expected_field_matches(field, expected, observed, recheck=recheck):
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


def expected_field_matches(
    field: str,
    expected: str,
    observed: str,
    *,
    recheck: bool = False,
) -> bool:
    if observed == expected:
        return True
    if not recheck:
        return False
    if field == "lifecycle_stage" and observed == "rechecking":
        return bool(expected)
    if field == "completion_source":
        if expected == "runtime_success" and observed == "recheck_success":
            return True
        if expected == "none" and observed == "recheck_failure":
            return True
    if field == "contract_layer" and expected != "ok" and observed != "ok":
        return True
    return False
