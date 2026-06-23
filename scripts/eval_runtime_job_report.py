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
        return {
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

    if is_profile_manifest_dependency_conflict(
        reason=reason,
        terminal_state=terminal_state,
        diagnostic_code=diagnostic_code,
    ):
        return {
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

    projection: dict[str, str] = {}
    if not is_meaningful_status(raw.get("evidence_binding_status")):
        projection["evidence_binding_status"] = "missing"
    if not is_meaningful_status(raw.get("completion_evidence_status")):
        projection["completion_evidence_status"] = "missing"
    if not is_meaningful_status(raw.get("attempt_outcome")):
        projection["attempt_outcome"] = "failed"
    return projection


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
