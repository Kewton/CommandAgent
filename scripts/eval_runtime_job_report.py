#!/usr/bin/env python3
"""Project eval/runtime facts into a job-level reporting record.

This module is intentionally side-effect free. It is a reporting boundary for
eval artifacts and human-readable reports; it must not select repair actions,
run commands, or mutate runtime behavior.
"""

from __future__ import annotations

from typing import Any


RUNTIME_JOB_REPORT_FIELD_NAMES = [
    "lifecycle_stage",
    "active_owner",
    "selected_action",
    "target_admission_status",
    "repair_action_plan_status",
    "completion_source",
    "large_disposition",
    "large_disposition_reason",
    "large_disposition_owner_action_status",
    "large_disposition_evidence",
]

RUNTIME_JOB_REPORT_FULL_FIELD_NAMES = [
    *RUNTIME_JOB_REPORT_FIELD_NAMES,
    "attempt_outcome",
    "evidence_runner_status",
    "verifier_rerun_result",
    "explicit_stop_reason",
]


def build_runtime_job_report(
    raw: dict[str, Any],
    *,
    dry_run: bool | None = None,
    recheck: bool = False,
) -> dict[str, str]:
    """Derive the Phase 14 report projection from already observed fields."""

    success = boolish(raw.get("success"))
    dry_run = boolish(raw.get("dry_run")) if dry_run is None else dry_run
    reason = clean(raw.get("reason") or raw.get("success_check_reason"))
    active_job = clean(raw.get("active_job") or raw.get("runtime_job_kind"))
    recovery_owner = clean(raw.get("recovery_owner"))
    repair_action = clean(raw.get("repair_action"))
    loop_control_action = clean(raw.get("loop_control_action"))
    runtime_job_outcome = clean(raw.get("runtime_job_outcome"))
    terminal_state = clean(raw.get("terminal_state"))
    failure_category = clean(raw.get("failure_category") or raw.get("failure_class"))
    diagnostic_code = clean(raw.get("diagnostic_code"))
    setup_job_state = clean(raw.get("setup_job_state") or raw.get("setup_state"))
    evidence_runner_status = clean(raw.get("evidence_runner_status"))
    verifier_rerun_result = clean(raw.get("verifier_rerun_result"))
    explicit_stop_reason = clean(raw.get("explicit_stop_reason"))

    projection = large_failure_projection(
        raw,
        success=success,
        reason=reason,
        terminal_state=terminal_state,
        failure_category=failure_category,
        diagnostic_code=diagnostic_code,
    )
    active_job = projection.get("active_job", active_job)
    recovery_owner = projection.get("recovery_owner", recovery_owner)
    repair_action = projection.get("repair_action", repair_action)
    explicit_stop_reason = projection.get(
        "explicit_stop_reason", explicit_stop_reason
    )
    runtime_job_outcome = projection.get("runtime_job_outcome", runtime_job_outcome)

    report = {
        "lifecycle_stage": lifecycle_stage(
            success=success,
            dry_run=dry_run,
            recheck=recheck,
            reason=reason,
            terminal_state=terminal_state,
            active_job=active_job,
            setup_job_state=setup_job_state,
            runtime_job_outcome=runtime_job_outcome,
            evidence_runner_status=evidence_runner_status,
            verifier_rerun_result=verifier_rerun_result,
            explicit_stop_reason=explicit_stop_reason,
        ),
        "active_owner": active_owner(recovery_owner, active_job, success),
        "selected_action": selected_action(
            repair_action, loop_control_action, active_job, runtime_job_outcome, success
        ),
        "target_admission_status": target_admission_status(raw, active_job, success),
        "repair_action_plan_status": repair_action_plan_status(raw, success),
        "completion_source": completion_source(
            success=success,
            dry_run=dry_run,
            recheck=recheck,
            raw=raw,
        ),
    }
    report.update(projection)
    for name in RUNTIME_JOB_REPORT_FIELD_NAMES:
        report.setdefault(name, "")
    return report


def large_failure_projection(
    raw: dict[str, Any],
    *,
    success: bool,
    reason: str,
    terminal_state: str,
    failure_category: str,
    diagnostic_code: str,
) -> dict[str, str]:
    """Project deterministic failed-row ownership/evidence for eval reports.

    This is a reporting projection only. It does not run repair, choose new
    runtime behavior, or hide failure. It makes already observed failures
    attributable enough for broad sign-off.
    """

    if success:
        return {}

    if is_provider_boundary_failure(
        reason=reason,
        terminal_state=terminal_state,
        failure_category=failure_category,
        diagnostic_code=diagnostic_code,
        raw=raw,
    ):
        stop_reason = (
            "provider_transport_timeout"
            if "timeout" in diagnostic_code.casefold()
            or "timeout" in reason.casefold()
            else "provider_transport_failure"
        )
        projection = {
            "active_job": "provider_transport_blocker",
            "recovery_owner": "provider_transport",
            "repair_action": "stop_for_provider_timeout"
            if stop_reason == "provider_transport_timeout"
            else "stop_for_provider_transport_failure",
            "target_path": "not_applicable",
            "target_role": "not_applicable",
            "selected_target": "not_applicable",
            "selected_target_role": "not_applicable",
            "target_admission_status": "not_applicable",
            "target_source_of_truth": "provider_transport",
            "target_ownership_source": "provider_transport_boundary",
            "evidence_binding_status": "not_applicable",
            "completion_evidence_status": "not_applicable",
            "attempt_outcome": "blocked_external",
            "explicit_stop_reason": stop_reason,
            "runtime_job_kind": "provider_transport_blocker",
            "runtime_job_outcome": "blocked_external",
        }
        if is_large_eval_row(raw):
            projection.update(
                {
                    "large_disposition": "accepted_external_limitation",
                    "large_disposition_reason": stop_reason,
                    "large_disposition_owner_action_status": "consistent",
                    "large_disposition_evidence": (
                        "owner=provider_transport;attempt=blocked_external"
                    ),
                }
            )
        return projection

    if is_profile_manifest_dependency_conflict(
        reason=reason,
        terminal_state=terminal_state,
        diagnostic_code=diagnostic_code,
    ):
        projection = {
            "active_job": "manifest_repair",
            "recovery_owner": "manifest",
            "repair_action": "resolve_manifest_conflict",
            "target_path": "package.json",
            "target_role": "setup_manifest",
            "selected_target": "package.json",
            "selected_target_role": "setup_manifest",
            "target_admission_status": "admitted",
            "target_source_of_truth": "profile_verification",
            "target_ownership_source": "profile_diagnostic",
            "evidence_binding_status": "bound",
            "completion_evidence_status": "failed",
            "attempt_outcome": "failed",
            "runtime_job_kind": "manifest_repair",
            "runtime_job_outcome": "failed",
        }
        if is_large_eval_row(raw):
            projection.update(
                {
                    "large_disposition": "closed_owned_failure",
                    "large_disposition_reason": "owned_manifest_conflict",
                    "large_disposition_owner_action_status": "consistent",
                    "large_disposition_evidence": (
                        "owner=manifest;action=resolve_manifest_conflict;"
                        "target=package.json;evidence=bound/failed"
                    ),
                }
            )
        return projection

    projection: dict[str, str] = {}
    if is_tool_protocol_failure(
        raw=raw,
        terminal_state=terminal_state,
        failure_category=failure_category,
        diagnostic_code=diagnostic_code,
        reason=reason,
    ) and not has_rejected_repair_plan(raw):
        projection.update(
            {
                "active_job": "tool_protocol_correction",
                "recovery_owner": "tool_protocol",
                "repair_action": "correct_tool_protocol",
                "runtime_job_kind": "tool_protocol_correction",
                "tool_protocol_status": clean(raw.get("tool_protocol_status"))
                or "admitted",
                "tool_protocol_source": clean(raw.get("tool_protocol_source"))
                or tool_protocol_source_for(raw, diagnostic_code),
                "tool_protocol_action": clean(raw.get("tool_protocol_action"))
                or tool_protocol_action_for(diagnostic_code),
                "tool_protocol_failed_tool": clean(
                    raw.get("tool_protocol_failed_tool")
                )
                or failed_tool_for(raw),
                "tool_protocol_missing_field": clean(
                    raw.get("tool_protocol_missing_field")
                )
                or missing_tool_field_for(raw, reason),
            }
        )
    elif diagnostic_code == "read_only_step_mutation":
        projection.update(
            {
                "active_job": "explicit_stop",
                "recovery_owner": "explicit_stop",
                "repair_action": "stop_with_structured_evidence",
                "runtime_job_kind": "explicit_stop",
                "runtime_job_outcome": "failed",
                "explicit_stop_reason": "read_only_step_mutation",
            }
        )

    if is_deterministic_failure_evidence(
        terminal_state=terminal_state,
        failure_category=failure_category,
        diagnostic_code=diagnostic_code,
        reason=reason,
    ):
        if not is_meaningful_status(raw.get("evidence_binding_status")):
            projection["evidence_binding_status"] = "bound"
        if not is_meaningful_status(raw.get("completion_evidence_status")):
            projection["completion_evidence_status"] = "failed"
        if not is_meaningful_status(raw.get("attempt_outcome")):
            projection["attempt_outcome"] = "failed"
        if not is_meaningful_status(raw.get("runtime_job_outcome")):
            projection["runtime_job_outcome"] = "failed"
    if not is_meaningful_status(raw.get("evidence_binding_status")):
        projection.setdefault("evidence_binding_status", "missing")
    if not is_meaningful_status(raw.get("completion_evidence_status")):
        projection.setdefault("completion_evidence_status", "missing")
    if not is_meaningful_status(raw.get("attempt_outcome")):
        projection.setdefault("attempt_outcome", "failed")
    projected = {**raw, **projection}
    projection.update(
        large_disposition_projection(
            projected,
            success=success,
            terminal_state=terminal_state,
            failure_category=failure_category,
            diagnostic_code=diagnostic_code,
            reason=reason,
        )
    )
    return projection


def is_tool_protocol_failure(
    *,
    raw: dict[str, Any],
    terminal_state: str,
    failure_category: str,
    diagnostic_code: str,
    reason: str,
) -> bool:
    text = " ".join(
        [
            terminal_state,
            failure_category,
            diagnostic_code,
            reason,
            clean(raw.get("producer")),
            clean(raw.get("source_of_truth")),
            clean(raw.get("failure_signature")),
            clean(raw.get("active_job") or raw.get("runtime_job_kind")),
            clean(raw.get("allowed_change_kind")),
        ]
    ).casefold()
    if terminal_state == "tool_protocol_failed" or failure_category == "tool_protocol":
        return True
    if diagnostic_code.startswith("tool_args_"):
        return True
    if diagnostic_code == "edit_target_not_found" and (
        "tool_protocol" in text
        or "tool_schema_contract" in text
        or "tool_call_shape_only" in text
    ):
        return True
    return False


def has_rejected_repair_plan(raw: dict[str, Any]) -> bool:
    rejection = clean(raw.get("repair_plan_rejection_reason"))
    if rejection and rejection != "none":
        return True
    return clean(raw.get("action_envelope_status")) == "rejected" or clean(
        raw.get("repair_action_plan_status")
    ) == "rejected"


def failed_tool_for(raw: dict[str, Any]) -> str:
    signature = clean(raw.get("failure_signature"))
    parts = signature.split("|")
    if len(parts) >= 3 and parts[0] in {"tool_protocol", "step_policy"}:
        tool = parts[2].strip()
        if tool and tool not in {"unknown", "none"}:
            return tool
    text = " ".join(
        [
            clean(raw.get("reason")),
            clean(raw.get("success_check_reason")),
            clean(raw.get("diagnostic_code")),
        ]
    )
    for marker in ["Write", "Edit", "Read", "Bash"]:
        if marker in text:
            return marker
    return "unknown"


def missing_tool_field_for(raw: dict[str, Any], reason: str) -> str:
    text = " ".join(
        [
            reason,
            clean(raw.get("success_check_reason")),
            clean(raw.get("failure_signature")),
            clean(raw.get("diagnostic_failure_kind")),
            clean(raw.get("observed_expected")),
        ]
    )
    marker = "tool_args_missing_required_field:"
    if marker in text:
        return text.split(marker, 1)[1].split()[0].split("|")[0].split(",")[0]
    if "required string field `" in text:
        return text.split("required string field `", 1)[1].split("`", 1)[0]
    return "not_applicable"


def tool_protocol_source_for(raw: dict[str, Any], diagnostic_code: str) -> str:
    if diagnostic_code == "edit_target_not_found":
        return "tool_target_resolution"
    return "tool_argument_schema"


def tool_protocol_action_for(diagnostic_code: str) -> str:
    if diagnostic_code == "edit_target_not_found":
        return "emit_tool_call_with_existing_target"
    return "emit_same_tool_with_required_fields"


def large_disposition_projection(
    raw: dict[str, Any],
    *,
    success: bool,
    terminal_state: str,
    failure_category: str,
    diagnostic_code: str,
    reason: str,
) -> dict[str, str]:
    existing = clean(raw.get("large_disposition"))
    if existing:
        return {
            "large_disposition": existing,
            "large_disposition_reason": clean(raw.get("large_disposition_reason")),
            "large_disposition_owner_action_status": clean(
                raw.get("large_disposition_owner_action_status")
            )
            or owner_action_status(raw),
            "large_disposition_evidence": clean(
                raw.get("large_disposition_evidence")
            )
            or disposition_evidence(raw, diagnostic_code),
        }
    if not is_large_eval_row(raw):
        return {
            "large_disposition": "",
            "large_disposition_reason": "",
            "large_disposition_owner_action_status": "",
            "large_disposition_evidence": "",
        }
    if success:
        return {
            "large_disposition": "not_applicable",
            "large_disposition_reason": "success",
            "large_disposition_owner_action_status": "not_applicable",
            "large_disposition_evidence": "success=true",
        }
    if is_provider_boundary_failure(
        reason=reason,
        terminal_state=terminal_state,
        failure_category=failure_category,
        diagnostic_code=diagnostic_code,
        raw=raw,
    ):
        return {
            "large_disposition": "accepted_external_limitation",
            "large_disposition_reason": "provider_transport_failure",
            "large_disposition_owner_action_status": "consistent",
            "large_disposition_evidence": disposition_evidence(raw, diagnostic_code),
        }

    missing = missing_disposition_inputs(raw)
    status = owner_action_status(raw)
    if missing or status != "consistent":
        reason_parts = []
        if missing:
            reason_parts.append("missing=" + ",".join(missing))
        if status != "consistent":
            reason_parts.append("owner_action=" + status)
        return {
            "large_disposition": "implementation_blocker",
            "large_disposition_reason": ";".join(reason_parts),
            "large_disposition_owner_action_status": status,
            "large_disposition_evidence": disposition_evidence(raw, diagnostic_code),
        }

    if clean(raw.get("recovery_owner")) == "explicit_stop":
        reason_code = clean(raw.get("explicit_stop_reason")) or diagnostic_code
        return {
            "large_disposition": "closed_owned_failure",
            "large_disposition_reason": "owned_explicit_stop:" + reason_code,
            "large_disposition_owner_action_status": status,
            "large_disposition_evidence": disposition_evidence(raw, diagnostic_code),
        }
    if diagnostic_code == "unknown_verifier_failure":
        reason_code = "owned_weak_verifier_failure_with_command"
    elif diagnostic_code == "blocked_bash_command_policy":
        reason_code = "owned_tool_policy_failure"
    elif clean(raw.get("recovery_owner")) == "tool_protocol":
        reason_code = "owned_tool_protocol_failure"
    else:
        reason_code = "owned_" + (failure_category or terminal_state or "failure")
    return {
        "large_disposition": "closed_owned_failure",
        "large_disposition_reason": reason_code,
        "large_disposition_owner_action_status": status,
        "large_disposition_evidence": disposition_evidence(raw, diagnostic_code),
    }


def missing_disposition_inputs(raw: dict[str, Any]) -> list[str]:
    missing: list[str] = []
    required = {
        "active_job": clean(raw.get("active_job") or raw.get("runtime_job_kind")),
        "owner": clean(raw.get("recovery_owner") or raw.get("active_owner")),
        "action": clean(raw.get("repair_action") or raw.get("selected_action")),
        "evidence_binding": clean(raw.get("evidence_binding_status")),
        "completion_evidence": clean(raw.get("completion_evidence_status")),
        "attempt_outcome": clean(raw.get("attempt_outcome")),
    }
    if not target_optional(raw):
        required["target"] = clean(raw.get("target_path") or raw.get("selected_target"))
    for key, value in required.items():
        if value.casefold() in {"", "unknown", "none"}:
            missing.append(key)
        elif value.casefold() == "not_applicable" and key not in {
            "target",
            "evidence_binding",
            "completion_evidence",
        }:
            missing.append(key)
    return missing


def owner_action_status(raw: dict[str, Any]) -> str:
    active_job = clean(raw.get("active_job") or raw.get("runtime_job_kind"))
    owner = clean(raw.get("recovery_owner") or raw.get("active_owner"))
    action = clean(raw.get("repair_action") or raw.get("selected_action"))
    if owner == "source" and action == "correct_tool_protocol":
        return "inconsistent_source_tool_protocol_action"
    if active_job == "tool_protocol_correction" and owner != "tool_protocol":
        return "inconsistent_tool_protocol_job_owner"
    if owner == "tool_protocol" and action != "correct_tool_protocol":
        return "inconsistent_tool_protocol_action"
    if owner == "explicit_stop" and action != "stop_with_structured_evidence":
        return "inconsistent_explicit_stop_action"
    if owner == "provider_transport" and clean(raw.get("attempt_outcome")) not in {
        "blocked_external",
        "stopped_external",
    }:
        return "inconsistent_provider_transport_attempt"
    return "consistent"


def target_optional(raw: dict[str, Any]) -> bool:
    terminal_state = clean(raw.get("terminal_state"))
    failure_category = clean(raw.get("failure_category") or raw.get("failure_class"))
    owner = clean(raw.get("recovery_owner") or raw.get("active_owner"))
    return (
        terminal_state in {"explicit_stop", "provider_transport_failed", "provider_parse_failed"}
        or failure_category == "provider_transport"
        or owner == "provider_transport"
    )


def disposition_evidence(raw: dict[str, Any], diagnostic_code: str) -> str:
    fields = {
        "owner": clean(raw.get("recovery_owner") or raw.get("active_owner")),
        "action": clean(raw.get("repair_action") or raw.get("selected_action")),
        "target": clean(raw.get("target_path") or raw.get("selected_target")),
        "evidence": clean(raw.get("evidence_binding_status"))
        + "/"
        + clean(raw.get("completion_evidence_status")),
        "attempt": clean(raw.get("attempt_outcome")),
        "diagnostic": diagnostic_code,
    }
    command = clean(raw.get("affected_cases") or raw.get("success_check"))
    if command:
        fields["verifier"] = command
    failed_tool = clean(raw.get("failed_tool") or raw.get("tool_protocol_failed_tool"))
    if failed_tool:
        fields["failed_tool"] = failed_tool
    blocked_command = clean(raw.get("blocked_command"))
    if blocked_command:
        fields["blocked_command"] = blocked_command
    command_class = clean(raw.get("command_class"))
    if command_class:
        fields["command_class"] = command_class
    classification_reason = clean(raw.get("command_classification_reason"))
    if classification_reason:
        fields["command_reason"] = classification_reason
    first_divergence = clean(raw.get("first_actionable_divergence"))
    if first_divergence:
        fields["first_divergence"] = first_divergence
    missing_field = clean(raw.get("tool_protocol_missing_field"))
    if missing_field:
        fields["missing_field"] = missing_field
    return ";".join(f"{key}={value or 'unknown'}" for key, value in fields.items())


def is_large_eval_row(raw: dict[str, Any]) -> bool:
    if clean(raw.get("eval_family") or raw.get("family")) == "large":
        return True
    for field in ["case_id", "matrix_row"]:
        value = clean(raw.get(field))
        if value.startswith("large-"):
            return True
    return False


def is_deterministic_failure_evidence(
    *,
    terminal_state: str,
    failure_category: str,
    diagnostic_code: str,
    reason: str,
) -> bool:
    if terminal_state in {
        "verifier_command_failed",
        "tool_protocol_failed",
        "step_policy_failed",
        "profile_contract_failed",
        "plan_lint_failed",
        "missing_deliverable",
        "eval_assertion_failed",
        "completion_evidence_failed",
        "evidence_binding_failed",
        "stale_evidence",
    }:
        return True
    if failure_category in {"verifier", "tool_protocol", "step_policy", "profile", "planning", "quality"}:
        return True
    if diagnostic_code and diagnostic_code not in {"unknown", "ok"} and not diagnostic_code.startswith("rc_"):
        return True
    if reason.startswith(("semantic_missing:", "semantic_mismatch:", "missing:", "tool_args_")):
        return True
    return False


def is_provider_boundary_failure(
    *,
    reason: str,
    terminal_state: str,
    failure_category: str,
    diagnostic_code: str,
    raw: dict[str, Any],
) -> bool:
    if terminal_state in {"provider_transport_failed", "provider_parse_failed"}:
        return True
    if failure_category == "provider_transport":
        return True
    if diagnostic_code.startswith("provider_transport:"):
        return True
    text = " ".join(
        [
            reason,
            diagnostic_code,
            clean(raw.get("stderr")),
            clean(raw.get("error")),
        ]
    ).casefold()
    return "provider_transport" in text or "eval command timed out" in text


def is_profile_manifest_dependency_conflict(
    *,
    reason: str,
    terminal_state: str,
    diagnostic_code: str,
) -> bool:
    text = " ".join([reason, terminal_state, diagnostic_code]).casefold()
    return (
        "profile_verification:nextjs_dependency_version_conflict" in text
        or "nextjs_dependency_version_conflict" in text
    )


def lifecycle_stage(
    *,
    success: bool,
    dry_run: bool,
    recheck: bool,
    reason: str,
    terminal_state: str,
    active_job: str,
    setup_job_state: str,
    runtime_job_outcome: str,
    evidence_runner_status: str,
    verifier_rerun_result: str,
    explicit_stop_reason: str,
) -> str:
    if recheck:
        return "rechecking"
    if dry_run:
        return "dry_run_placeholder"
    if (
        is_meaningful_value(explicit_stop_reason)
        or active_job == "explicit_stop"
        or terminal_state == "explicit_stop"
    ):
        return "explicit_stop"
    if success or reason == "ok" or terminal_state == "ok" or runtime_job_outcome == "passed":
        return "completed"
    if setup_job_state or active_job in {"setup_bootstrap", "manifest_repair", "dev_server_smoke"}:
        return "setup"
    if verifier_rerun_result or evidence_runner_status in {"executed", "failed", "missing"}:
        return "verifying"
    if active_job and active_job != "none":
        return "repairing"
    if terminal_state in {"plan_parse_failed", "plan_schema_failed", "plan_lint_failed"}:
        return "planning"
    if terminal_state in {"provider_transport_failed", "provider_parse_failed", "tool_protocol_failed"}:
        return "running"
    if terminal_state in {"repair_exhausted", "missing_evidence", "stale_evidence"}:
        return "blocked"
    return "failed"


def active_owner(recovery_owner: str, active_job: str, success: bool) -> str:
    if recovery_owner:
        return recovery_owner
    if active_job and active_job != "none":
        return active_job
    return "none" if success else "unknown"


def selected_action(
    repair_action: str,
    loop_control_action: str,
    active_job: str,
    runtime_job_outcome: str,
    success: bool,
) -> str:
    for candidate in [repair_action, loop_control_action, active_job]:
        if is_meaningful_value(candidate):
            return candidate
    if not success and is_meaningful_value(runtime_job_outcome):
        return runtime_job_outcome
    return "none" if success else "unknown"


def target_admission_status(raw: dict[str, Any], active_job: str, success: bool) -> str:
    existing = clean(raw.get("target_admission_status"))
    if existing:
        return existing
    rejection = clean(raw.get("target_rejection_reasons") or raw.get("rejected_target_reason"))
    if rejection:
        return "rejected"
    admitted_count = clean(raw.get("target_admitted_count"))
    rejected_count = clean(raw.get("target_rejected_count"))
    if is_positive_int(admitted_count):
        return "admitted"
    if is_positive_int(rejected_count):
        return "rejected"
    if clean(raw.get("selected_target") or raw.get("target_path")):
        return "admitted"
    if success or active_job == "none":
        return "not_applicable"
    return "unknown"


def repair_action_plan_status(raw: dict[str, Any], success: bool) -> str:
    existing = clean(raw.get("repair_action_plan_status"))
    if existing:
        return existing
    rejection = clean(raw.get("repair_plan_rejection_reason"))
    if rejection and rejection != "none":
        return "rejected"
    values = [
        clean(raw.get("repair_brief_status")),
        clean(raw.get("action_envelope_status")),
        clean(raw.get("repair_state_status")),
    ]
    if any(value == "rejected" for value in values):
        return "rejected"
    if any(value in {"admitted", "planned", "selected"} for value in values):
        return "planned"
    repair_action = clean(raw.get("repair_action"))
    if repair_action and repair_action != "none":
        return "planned"
    return "not_applicable" if success else "unknown"


def completion_source(
    *,
    success: bool,
    dry_run: bool,
    recheck: bool,
    raw: dict[str, Any],
) -> str:
    existing = clean(raw.get("completion_source"))
    if existing:
        return existing
    if recheck:
        return "recheck_success" if success else "recheck_failure"
    if dry_run:
        return "dry_run_placeholder_success"
    if success:
        source_of_truth = clean(raw.get("completion_source_of_truth"))
        attempt_outcome = clean(raw.get("attempt_outcome"))
        runtime_job_outcome = clean(raw.get("runtime_job_outcome"))
        if source_of_truth == "existing_workspace" or (
            attempt_outcome in {"not_attempted", ""}
            and runtime_job_outcome == ""
            and clean(raw.get("active_job")) in {"", "none"}
        ):
            return "existing_success"
        if source_of_truth in {"completion_evidence", "completion_evidence_freshness"}:
            return "evidence_only_success"
        return "runtime_success"
    return "none"


def clean(value: Any) -> str:
    if value is None:
        return ""
    return str(value).strip()


def is_meaningful_value(value: Any) -> bool:
    cleaned = clean(value).casefold()
    return cleaned not in {"", "none", "not_applicable"}


def is_meaningful_status(value: Any) -> bool:
    cleaned = clean(value).casefold()
    return cleaned not in {"", "unknown", "none"}


def boolish(value: Any) -> bool:
    if isinstance(value, bool):
        return value
    if value is None:
        return False
    return str(value).strip().casefold() in {"1", "true", "yes", "ok", "passed"}


def is_positive_int(value: str) -> bool:
    try:
        return int(value) > 0
    except (TypeError, ValueError):
        return False
