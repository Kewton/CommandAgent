#!/usr/bin/env python3
"""Normalize CommandAgent eval failure observations.

This module is intentionally deterministic and side-effect free. It classifies
already observed run data for eval reporting; it must not decide repair actions
or change runtime behavior.
"""

from __future__ import annotations

import csv
import re
from pathlib import Path
from typing import Any


OBSERVATION_FIELD_NAMES = [
    "terminal_state",
    "failure_class",
    "violated_contract",
    "source",
    "source_of_truth",
    "diagnostic_code",
    "failure_signature",
    "producer",
    "guard",
    "actionability",
    "completion_authority_status",
    "completion_source_of_truth",
    "evidence_runner_status",
    "evidence_runner_kind",
    "artifact_ledger_status",
    "freshness_status",
    "missing_evidence",
    "failed_evidence",
    "failed_bindings",
    "stale_evidence",
    "evidence_binding_kind",
    "workspace_scope_kind",
    "workspace_scope_roots",
    "artifact_ledger_entries",
    "artifact_ledger_summary",
    "artifact_ownership",
    "artifact_ownership_reason",
    "artifact_source_of_truth",
    "rejected_target_reason",
    "read_paths",
    "changed_paths",
    "created_paths",
    "verifier_mentioned_paths",
    "scaffold_created_paths",
    "setup_created_paths",
    "out_of_scope_paths",
    "command",
    "setup_state",
    "port",
]


def load_taxonomy() -> dict[str, dict[str, str]]:
    path = Path(__file__).with_name("failure_observation_taxonomy.tsv")
    with open(path, encoding="utf-8", newline="") as handle:
        return {
            row["terminal_state"]: row
            for row in csv.DictReader(handle, delimiter="\t")
        }


TERMINAL_STATE_TAXONOMY = load_taxonomy()
TERMINAL_STATE_TO_CATEGORY = {
    state: row["failure_class"] for state, row in TERMINAL_STATE_TAXONOMY.items()
}
TERMINAL_STATE_TO_CONTRACT_LAYER = {
    state: row["contract_layer"] for state, row in TERMINAL_STATE_TAXONOMY.items()
}
TERMINAL_STATE_TO_VIOLATED_CONTRACT = {
    state: row["violated_contract"] for state, row in TERMINAL_STATE_TAXONOMY.items()
}
TERMINAL_STATE_TO_SOURCE = {
    state: row["source"] for state, row in TERMINAL_STATE_TAXONOMY.items()
}
TERMINAL_STATE_TO_SOURCE_OF_TRUTH = {
    state: row["source_of_truth"] for state, row in TERMINAL_STATE_TAXONOMY.items()
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
    contract_layer = clean(raw.get("contract_layer")) or contract_layer_for_terminal_state(
        terminal_state, failure_class
    )
    diagnostic_code = (
        clean(raw.get("diagnostic_code"))
        or contract_value(evidence, "diagnostic_code")
        or diagnostic_code_from_reason(reason, terminal_state)
    )
    failure_signature = clean(raw.get("failure_signature")) or contract_value(
        evidence, "failure_signature"
    )
    producer = clean(raw.get("producer")) or contract_value(evidence, "producer") or producer_for_terminal_state(
        terminal_state
    )
    guard = clean(raw.get("guard")) or contract_value(evidence, "guard")
    actionability = (
        clean(raw.get("actionability"))
        or contract_value(evidence, "actionability")
        or actionability_for_terminal_state(terminal_state)
    )
    evidence_runner_status = (
        clean(raw.get("evidence_runner_status"))
        or contract_value(evidence, "evidence_runner_status")
        or evidence_runner_status_for_terminal_state(terminal_state)
    )
    completion_authority_status = (
        clean(raw.get("completion_authority_status"))
        or contract_value(evidence, "completion_authority_status")
        or terminal_state
    )
    completion_source_of_truth = (
        clean(raw.get("completion_source_of_truth"))
        or contract_value(evidence, "completion_source_of_truth")
        or contract_value(evidence, "source_of_truth")
    )
    freshness_status = (
        clean(raw.get("freshness_status"))
        or contract_value(evidence, "freshness_status")
        or freshness_status_for_terminal_state(terminal_state)
    )
    artifact_ledger_status = (
        clean(raw.get("artifact_ledger_status"))
        or contract_value(evidence, "artifact_ledger_status")
        or artifact_ledger_status_for_terminal_state(terminal_state)
    )
    source = clean(raw.get("source")) or source_for_terminal_state(terminal_state, reason)
    source_of_truth = (
        clean(raw.get("source_of_truth"))
        or contract_value(evidence, "source_of_truth")
        or source_of_truth_for_terminal_state(terminal_state)
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
        "failure_signature": failure_signature,
        "producer": producer,
        "guard": guard,
        "actionability": actionability,
        "completion_authority_status": completion_authority_status,
        "completion_source_of_truth": completion_source_of_truth,
        "evidence_runner_status": evidence_runner_status,
        "evidence_runner_kind": clean(raw.get("evidence_runner_kind"))
        or contract_value(evidence, "evidence_runner_kind"),
        "artifact_ledger_status": artifact_ledger_status,
        "freshness_status": freshness_status,
        "missing_evidence": clean(raw.get("missing_evidence"))
        or contract_value(evidence, "missing_evidence"),
        "failed_evidence": clean(raw.get("failed_evidence"))
        or contract_value(evidence, "failed_evidence"),
        "failed_bindings": clean(raw.get("failed_bindings"))
        or contract_value(evidence, "failed_bindings"),
        "stale_evidence": clean(raw.get("stale_evidence"))
        or contract_value(evidence, "stale_evidence"),
        "evidence_binding_kind": clean(raw.get("evidence_binding_kind"))
        or contract_value(evidence, "evidence_binding_kind"),
        "workspace_scope_kind": clean(raw.get("workspace_scope_kind"))
        or contract_value(evidence, "workspace_scope_kind"),
        "workspace_scope_roots": clean(raw.get("workspace_scope_roots"))
        or contract_value(evidence, "workspace_scope_roots"),
        "artifact_ledger_entries": clean(raw.get("artifact_ledger_entries"))
        or contract_value(evidence, "artifact_ledger_entries"),
        "artifact_ledger_summary": clean(raw.get("artifact_ledger_summary"))
        or contract_value(evidence, "artifact_ledger_summary"),
        "artifact_ownership": clean(raw.get("artifact_ownership"))
        or contract_value(evidence, "artifact_ownership"),
        "artifact_ownership_reason": clean(raw.get("artifact_ownership_reason"))
        or contract_value(evidence, "artifact_ownership_reason"),
        "artifact_source_of_truth": clean(raw.get("artifact_source_of_truth"))
        or contract_value(evidence, "artifact_source_of_truth"),
        "rejected_target_reason": clean(raw.get("rejected_target_reason"))
        or contract_value(evidence, "rejected_target_reason"),
        "read_paths": clean(raw.get("read_paths")) or contract_value(evidence, "read_paths"),
        "changed_paths": clean(raw.get("changed_paths"))
        or contract_value(evidence, "changed_paths"),
        "created_paths": clean(raw.get("created_paths"))
        or contract_value(evidence, "created_paths"),
        "verifier_mentioned_paths": clean(raw.get("verifier_mentioned_paths"))
        or contract_value(evidence, "verifier_mentioned_paths"),
        "scaffold_created_paths": clean(raw.get("scaffold_created_paths"))
        or contract_value(evidence, "scaffold_created_paths"),
        "setup_created_paths": clean(raw.get("setup_created_paths"))
        or contract_value(evidence, "setup_created_paths"),
        "out_of_scope_paths": clean(raw.get("out_of_scope_paths"))
        or contract_value(evidence, "out_of_scope_paths"),
        "command": clean(raw.get("command")) or contract_value(evidence, "command"),
        "setup_state": setup_state,
        "port": port,
        "explicit_stop_reason": clean(raw.get("explicit_stop_reason"))
        or contract_value(evidence, "explicit_stop_reason"),
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
    if "xml syntax error" in combined and (
        "function" in combined or "tool" in combined or "ollama" in combined
    ):
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
    if reason_lc.startswith("semantic_mismatch:"):
        return "eval_assertion_failed"
    if reason_lc.startswith("quality:") or reason_lc.startswith("app_quality:"):
        return "eval_assertion_failed"
    completion_status = clean(raw.get("completion_evidence_status")).casefold()
    binding_status = clean(raw.get("evidence_binding_status")).casefold()
    ledger_status = clean(raw.get("artifact_ledger_status")).casefold()
    runner_status = clean(raw.get("evidence_runner_status")).casefold()
    freshness_status = clean(raw.get("freshness_status")).casefold()
    if ledger_status == "missing_required" or "artifact_ledger_status=missing_required" in combined:
        return "missing_deliverable"
    if (
        completion_status == "stale"
        or freshness_status == "stale"
        or "completion_evidence_status=stale" in combined
        or "freshness_status=stale" in combined
        or "stale_evidence" in combined
    ):
        return "stale_evidence"
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
    terminal_state = terminal_state_from_reason(reason)
    return contract_layer_for_terminal_state(
        terminal_state,
        category_for_terminal_state(terminal_state, reason),
    )


def contract_layer_for_category(category: str) -> str:
    return CATEGORY_TO_CONTRACT_LAYER.get(category, "unknown_contract")


def contract_layer_for_terminal_state(terminal_state: str, category: str) -> str:
    return TERMINAL_STATE_TO_CONTRACT_LAYER.get(
        terminal_state, contract_layer_for_category(category)
    )


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
        "provider_transport:",
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
    if terminal_state in TERMINAL_STATE_TO_SOURCE:
        return TERMINAL_STATE_TO_SOURCE[terminal_state]
    if reason:
        return "reason"
    return "unknown"


def source_of_truth_for_terminal_state(terminal_state: str) -> str:
    return TERMINAL_STATE_TO_SOURCE_OF_TRUTH.get(terminal_state, "unknown")


def producer_for_terminal_state(terminal_state: str) -> str:
    if terminal_state == "ok":
        return "process_result"
    if terminal_state == "plan_parse_failed":
        return "plan_parser"
    if terminal_state == "plan_schema_failed":
        return "plan_schema"
    if terminal_state == "plan_lint_failed":
        return "plan_lint"
    if terminal_state == "provider_transport_failed":
        return "provider_transport"
    if terminal_state == "provider_parse_failed":
        return "provider_parser"
    if terminal_state == "tool_protocol_failed":
        return "tool_protocol"
    if terminal_state == "step_policy_failed":
        return "step_policy"
    if terminal_state == "profile_contract_failed":
        return "profile_verification"
    if terminal_state == "verifier_command_failed":
        return "verifier"
    if terminal_state in {"dependency_missing", "setup_failed"}:
        return "setup_runtime"
    if terminal_state == "port_in_use":
        return "dev_server"
    if terminal_state in {"missing_deliverable", "eval_assertion_failed"}:
        return "eval_success"
    if terminal_state in {"missing_evidence", "completion_evidence_failed"}:
        return "completion_evidence"
    if terminal_state == "stale_evidence":
        return "completion_evidence"
    if terminal_state == "evidence_binding_failed":
        return "evidence_binding"
    if terminal_state in {"repair_exhausted", "explicit_stop"}:
        return "recovery_loop"
    return "unknown"


def actionability_for_terminal_state(terminal_state: str) -> str:
    if terminal_state == "ok":
        return "not_applicable"
    if terminal_state in {"explicit_stop", "repair_exhausted"}:
        return "explicit_stop"
    if terminal_state == "unknown":
        return "unknown"
    return "actionable"


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
    if terminal_state in {"completion_evidence_failed", "evidence_binding_failed", "stale_evidence"}:
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
        "stale_evidence",
        "eval_assertion_failed",
    }:
        return "complete"
    return ""


def freshness_status_for_terminal_state(terminal_state: str) -> str:
    if terminal_state == "ok":
        return "fresh"
    if terminal_state == "stale_evidence":
        return "stale"
    if terminal_state in {
        "missing_deliverable",
        "missing_evidence",
        "completion_evidence_failed",
        "evidence_binding_failed",
    }:
        return "unknown"
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
