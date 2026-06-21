#!/usr/bin/env python3
"""Normalize CommandAgent eval failure observations.

This module is intentionally deterministic and side-effect free. It classifies
already observed run data for eval reporting; it must not decide repair actions
or change runtime behavior.
"""

from __future__ import annotations

import re
from typing import Any


OBSERVATION_FIELD_NAMES = [
    "terminal_state",
    "failure_class",
    "violated_contract",
    "source",
    "source_of_truth",
    "diagnostic_code",
    "evidence_runner_status",
    "artifact_ledger_status",
    "command",
    "setup_state",
    "port",
]


TERMINAL_STATE_TO_CATEGORY = {
    "ok": "ok",
    "plan_parse_failed": "planning",
    "plan_schema_failed": "planning",
    "plan_lint_failed": "planning",
    "provider_transport_failed": "provider_transport",
    "provider_parse_failed": "provider_transport",
    "tool_protocol_failed": "tool_protocol",
    "step_policy_failed": "step_policy",
    "profile_contract_failed": "profile",
    "verifier_command_failed": "verifier",
    "dependency_missing": "setup",
    "setup_failed": "setup",
    "port_in_use": "setup",
    "missing_deliverable": "planning",
    "missing_evidence": "quality",
    "evidence_binding_failed": "quality",
    "completion_evidence_failed": "quality",
    "eval_assertion_failed": "quality",
    "repair_exhausted": "planning",
    "explicit_stop": "unknown",
    "unknown": "unknown",
}


CATEGORY_TO_CONTRACT_LAYER = {
    "ok": "ok",
    "planning": "planning_contract",
    "provider_transport": "execution_contract",
    "tool_protocol": "execution_contract",
    "step_policy": "execution_contract",
    "profile": "profile_contract",
    "setup": "setup_bootstrap_contract",
    "verifier": "verification_contract",
    "quality": "eval_success_contract",
    "unknown": "unknown_contract",
}


TERMINAL_STATE_TO_VIOLATED_CONTRACT = {
    "ok": "none",
    "plan_parse_failed": "plan_file_parse_contract",
    "plan_schema_failed": "plan_file_schema_contract",
    "plan_lint_failed": "planning_contract",
    "provider_transport_failed": "provider_transport_contract",
    "provider_parse_failed": "provider_tool_call_parse_contract",
    "tool_protocol_failed": "tool_protocol_contract",
    "step_policy_failed": "step_execution_policy_contract",
    "profile_contract_failed": "profile_contract",
    "verifier_command_failed": "verification_contract",
    "dependency_missing": "setup_bootstrap_contract",
    "setup_failed": "setup_bootstrap_contract",
    "port_in_use": "dev_server_port_contract",
    "missing_deliverable": "eval_success_contract",
    "missing_evidence": "evidence_contract",
    "evidence_binding_failed": "evidence_binding_contract",
    "completion_evidence_failed": "completion_evidence_contract",
    "eval_assertion_failed": "eval_success_contract",
    "repair_exhausted": "bounded_repair_contract",
    "explicit_stop": "explicit_stop_contract",
    "unknown": "unknown_contract",
}


def normalize_observation(raw: dict[str, Any]) -> dict[str, str]:
    reason = clean(raw.get("reason") or raw.get("success_check_reason"))
    success = boolish(raw.get("success"))
    if not reason:
        reason = "ok" if success else "unknown"
    evidence = evidence_text(raw)
    terminal_state = clean(raw.get("terminal_state")) or terminal_state_from_reason(
        reason, evidence, raw
    )
    failure_class = clean(raw.get("failure_class")) or category_for_terminal_state(
        terminal_state, reason
    )
    contract_layer = clean(raw.get("contract_layer")) or contract_layer_for_category(
        failure_class
    )
    diagnostic_code = clean(raw.get("diagnostic_code")) or diagnostic_code_from_reason(
        reason, terminal_state
    )
    evidence_runner_status = (
        clean(raw.get("evidence_runner_status"))
        or contract_value(evidence, "evidence_runner_status")
        or evidence_runner_status_for_terminal_state(terminal_state)
    )
    artifact_ledger_status = (
        clean(raw.get("artifact_ledger_status"))
        or contract_value(evidence, "artifact_ledger_status")
        or artifact_ledger_status_for_terminal_state(terminal_state)
    )
    source = clean(raw.get("source")) or source_for_terminal_state(terminal_state, reason)
    source_of_truth = clean(raw.get("source_of_truth")) or source_of_truth_for_terminal_state(
        terminal_state
    )
    setup_state = clean(raw.get("setup_state")) or setup_state_for_terminal_state(
        terminal_state
    )
    port = ""
    if terminal_state == "port_in_use":
        port = clean(raw.get("port")) or port_from_text(evidence)

    return {
        "terminal_state": terminal_state,
        "failure_class": failure_class,
        "failure_category": failure_class,
        "contract_layer": contract_layer,
        "violated_contract": clean(raw.get("violated_contract"))
        or TERMINAL_STATE_TO_VIOLATED_CONTRACT.get(terminal_state, "unknown_contract"),
        "source": source,
        "source_of_truth": source_of_truth,
        "diagnostic_code": diagnostic_code,
        "evidence_runner_status": evidence_runner_status,
        "artifact_ledger_status": artifact_ledger_status,
        "command": clean(raw.get("command")),
        "setup_state": setup_state,
        "port": port,
    }


def terminal_state_from_reason(reason: str, evidence: str = "", raw: dict[str, Any] | None = None) -> str:
    raw = raw or {}
    reason_lc = reason.casefold()
    evidence_lc = evidence.casefold()
    combined = "\n".join([reason, evidence]).casefold()

    if boolish(raw.get("success")) or reason == "ok":
        return "ok"
    if reason_lc == "port_in_use":
        return "port_in_use"
    if "eaddrinuse" in combined or "address already in use" in combined:
        return "port_in_use"
    if "invalid plan yaml" in combined or "unsupported block scalar" in combined:
        return "plan_parse_failed"
    if (
        "plan schema" in combined
        or "missing required plan" in combined
        or "invalid plan schema" in combined
    ):
        return "plan_schema_failed"
    if (
        reason_lc.startswith("planning:")
        or reason_lc.startswith("plan_lint")
        or "plan lint" in combined
        or "invalid ultra plan" in combined
        or "invalid step plan" in combined
    ):
        return "plan_lint_failed"
    if (
        "bounded repair exhausted" in combined
        or "bounded_plan_correction_exhausted" in combined
        or "plan_correction_no_progress_or_exhausted" in combined
    ):
        return "repair_exhausted"
    if reason_lc.startswith("provider_parse") or "tool call is missing a tool name" in combined:
        return "provider_parse_failed"
    if "json parse failed" in combined and "tool" in combined:
        return "provider_parse_failed"
    if (
        reason_lc.startswith("provider_transport")
        or "transport failed" in combined
        or "error sending request for url" in combined
        or "connection refused" in combined
        or "http" in combined and "provider" in combined
        or "api key" in combined
        or "authentication" in combined
        or "rate limit" in combined
        or "timed out" in combined and "provider" in combined
    ):
        return "provider_transport_failed"
    if (
        reason_lc.startswith("tool_args_")
        or reason_lc.startswith("tool_protocol")
        or "invalid tool arguments" in combined
        or "arguments are not valid json" in combined
    ):
        return "tool_protocol_failed"
    if reason_lc.startswith("step_policy:") or reason == "read_only_step_mutation":
        return "step_policy_failed"
    if "read_only_step_mutation" in combined or "read-only step" in combined:
        return "step_policy_failed"
    if reason_lc.startswith("profile_verification:") or "profile verification failed" in combined:
        return "profile_contract_failed"
    if "dependency_missing" in combined:
        return "dependency_missing"
    if (
        "module_not_found" in combined
        or "cannot find module" in combined
        or "node_modules/.bin" in combined
    ):
        return "dependency_missing"
    if (
        reason_lc.startswith("setup:")
        or reason_lc.startswith("dependency_setup:")
        or "npm err! eresolve" in combined
    ):
        return "setup_failed"
    if reason_lc.startswith("missing:") or reason_lc.startswith("semantic_missing:"):
        return "missing_deliverable"
    completion_status = clean(raw.get("completion_evidence_status")).casefold()
    binding_status = clean(raw.get("evidence_binding_status")).casefold()
    ledger_status = clean(raw.get("artifact_ledger_status")).casefold()
    runner_status = clean(raw.get("evidence_runner_status")).casefold()
    if ledger_status == "missing_required" or "artifact_ledger_status=missing_required" in combined:
        return "missing_deliverable"
    if (
        completion_status == "missing"
        or runner_status == "missing"
        or "completion_evidence_status=missing" in combined
        or "evidence_runner_status=missing" in combined
    ):
        return "missing_evidence"
    if completion_status == "failed" or "completion_evidence_status=failed" in combined:
        return "completion_evidence_failed"
    if binding_status in {"missing", "failed", "unbound"} or any(
        f"evidence_binding_status={status}" in combined
        for status in ["missing", "failed", "unbound"]
    ):
        return "evidence_binding_failed"
    if "missing_evidence" in combined or "missing evidence" in combined:
        return "missing_evidence"
    if "evidence_binding" in combined and "failed" in combined:
        return "evidence_binding_failed"
    if "completion_evidence" in combined and "failed" in combined:
        return "completion_evidence_failed"
    if reason_lc.startswith("semantic_mismatch:"):
        return "eval_assertion_failed"
    if reason_lc.startswith("quality:") or reason_lc.startswith("app_quality:"):
        return "eval_assertion_failed"
    if reason_lc.startswith("explicit_stop") or "explicit_stop" in combined:
        return "explicit_stop"
    if reason_lc.startswith("rc:") or reason_lc.startswith("command_failed:") or reason_lc.startswith("blocked:"):
        return "verifier_command_failed"
    if not reason or reason == "unknown":
        return "unknown"
    return "unknown"


def category_for_terminal_state(terminal_state: str, reason: str = "") -> str:
    if terminal_state in TERMINAL_STATE_TO_CATEGORY:
        return TERMINAL_STATE_TO_CATEGORY[terminal_state]
    return category_for_reason(reason)


def category_for_reason(reason: str) -> str:
    return category_for_terminal_state(terminal_state_from_reason(reason), reason)


def contract_layer_for_reason(reason: str) -> str:
    return contract_layer_for_category(category_for_reason(reason))


def contract_layer_for_category(category: str) -> str:
    return CATEGORY_TO_CONTRACT_LAYER.get(category, "unknown_contract")


def diagnostic_code_from_reason(reason: str, terminal_state: str) -> str:
    if reason == "ok":
        return "ok"
    for prefix in [
        "profile_verification:",
        "tool_args_missing_required_field:",
        "tool_protocol_failure:",
        "plan_lint.",
        "semantic_missing:",
        "semantic_mismatch:",
        "missing:",
        "setup:",
        "dependency_setup:",
    ]:
        if reason.startswith(prefix):
            value = reason[len(prefix) :].split(",", 1)[0]
            return sanitize_code(prefix.rstrip(":.")) + ":" + sanitize_code(value)
    if reason.startswith("rc:"):
        return "rc_" + sanitize_code(reason.split(":", 1)[1])
    if reason.startswith("command_failed:"):
        return "command_failed_" + sanitize_code(reason.split(":", 1)[1])
    if terminal_state != "unknown":
        return terminal_state
    return "unknown"


def source_for_terminal_state(terminal_state: str, reason: str) -> str:
    if terminal_state == "ok":
        return "process_result"
    if terminal_state.startswith("plan_"):
        return "plan_contract"
    if terminal_state.startswith("provider_"):
        return "provider_response"
    if terminal_state in {"tool_protocol_failed", "step_policy_failed"}:
        return "execution_guard"
    if terminal_state == "profile_contract_failed":
        return "profile_verifier"
    if terminal_state in {"dependency_missing", "setup_failed", "port_in_use"}:
        return "environment_verifier"
    if terminal_state in {
        "missing_deliverable",
        "missing_evidence",
        "evidence_binding_failed",
        "completion_evidence_failed",
        "eval_assertion_failed",
    }:
        return "eval_success_check"
    if terminal_state == "repair_exhausted":
        return "bounded_repair"
    if terminal_state == "verifier_command_failed":
        return "verifier"
    if reason:
        return "reason"
    return "unknown"


def source_of_truth_for_terminal_state(terminal_state: str) -> str:
    if terminal_state == "ok":
        return "process_exit_and_eval_checks"
    if terminal_state in {
        "missing_deliverable",
        "missing_evidence",
        "evidence_binding_failed",
        "completion_evidence_failed",
        "eval_assertion_failed",
    }:
        return "eval_success_contract"
    if terminal_state in {"dependency_missing", "setup_failed", "port_in_use"}:
        return "verifier_output"
    if terminal_state == "unknown":
        return "unknown"
    return "runtime_evidence"


def setup_state_for_terminal_state(terminal_state: str) -> str:
    if terminal_state == "dependency_missing":
        return "dependency_missing"
    if terminal_state == "setup_failed":
        return "setup_failed"
    if terminal_state == "port_in_use":
        return "port_in_use"
    return ""


def evidence_runner_status_for_terminal_state(terminal_state: str) -> str:
    if terminal_state == "ok":
        return "executed"
    if terminal_state == "missing_evidence":
        return "missing"
    if terminal_state in {"completion_evidence_failed", "evidence_binding_failed"}:
        return "executed"
    return ""


def artifact_ledger_status_for_terminal_state(terminal_state: str) -> str:
    if terminal_state == "ok":
        return "complete"
    if terminal_state == "missing_deliverable":
        return "missing_required"
    if terminal_state in {
        "missing_evidence",
        "completion_evidence_failed",
        "evidence_binding_failed",
        "eval_assertion_failed",
    }:
        return "complete"
    return ""


def port_from_text(text: str) -> str:
    patterns = [
        r"\bport\s*[:=]\s*(\d{2,5})\b",
        r"\blocalhost:(\d{2,5})\b",
        r"\b127\.0\.0\.1:(\d{2,5})\b",
        r":::(\d{2,5})\b",
        r"\s-p\s+(\d{2,5})\b",
    ]
    for pattern in patterns:
        match = re.search(pattern, text, flags=re.IGNORECASE)
        if match:
            return match.group(1)
    return ""


def evidence_text(raw: dict[str, Any]) -> str:
    parts = [
        raw.get("reason"),
        raw.get("evidence"),
        raw.get("stdout"),
        raw.get("stderr"),
        raw.get("diagnostic"),
        raw.get("diagnostic_excerpt"),
        raw.get("explicit_stop_reason"),
    ]
    return "\n".join(clean(part) for part in parts if clean(part))


def contract_value(text: str, key: str) -> str:
    patterns = [
        rf"^- {re.escape(key)}:\s*(.+)$",
        rf"^- {re.escape(key)}=([^\s,]+)",
        rf"\b{re.escape(key)}=([^\s,]+)",
    ]
    for pattern in patterns:
        match = re.search(pattern, text, flags=re.MULTILINE)
        if match:
            return match.group(1).strip()
    return ""


def boolish(value: Any) -> bool:
    if isinstance(value, bool):
        return value
    if isinstance(value, str):
        return value.strip().casefold() == "true"
    return False


def clean(value: Any) -> str:
    if value is None:
        return ""
    return str(value).strip()


def sanitize_code(value: str) -> str:
    value = value.strip()
    if not value:
        return "unknown"
    return re.sub(r"[^a-zA-Z0-9_./:-]+", "_", value).strip("_") or "unknown"
