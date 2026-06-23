#!/usr/bin/env python3
import argparse
import csv
import json
import sys
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from eval_failure_observation import (  # noqa: E402
    OBSERVATION_FIELD_NAMES,
    category_for_reason,
    contract_value,
    contract_layer_for_reason,
    normalize_observation,
)
from eval_case_schema import (  # noqa: E402
    ASSERTION_FIELD_NAMES,
    MATRIX_FIELD_NAMES,
    focused_assertions,
    iter_case_paths,
    read_eval_case,
)
from eval_runtime_job_report import (  # noqa: E402
    RUNTIME_JOB_REPORT_FIELD_NAMES,
    build_runtime_job_report,
)


def parse_args():
    parser = argparse.ArgumentParser(description="Report or recheck CommandAgent eval roots")
    parser.add_argument("root")
    parser.add_argument("--cases-dir", default="eval/cases/smoke")
    parser.add_argument("--recheck", action="store_true")
    return parser.parse_args()


def read_cases(cases_dir):
    cases = {}
    for path in iter_case_paths(cases_dir):
        case = read_case(path)
        cases[case["id"]] = case
    return cases


def read_case(path):
    case = read_eval_case(path)
    check = case.get("success_check", {})
    return {
        "id": case["id"],
        "required_paths": check.get("required_paths", []),
        "must_include": check.get("must_include", {}),
        "type": check.get("type", "semantic"),
        "expected_fields": case.get("expected_fields", {}),
        "matrix_row": case.get("matrix_row", case["id"]),
        "proof_mode": case.get("proof_mode", "real_llm"),
    }


def unquote(value):
    if len(value) >= 2 and value[0] == value[-1] and value[0] in {'"', "'"}:
        return value[1:-1]
    return value


def read_summary(path):
    with open(path, encoding="utf-8", newline="") as handle:
        return list(csv.DictReader(handle, delimiter="\t"))


def failure_evidence(run_dir):
    parts = []
    for name in ["stdout.txt", "stderr.txt"]:
        path = run_dir / name
        if path.exists():
            parts.append(path.read_text(encoding="utf-8", errors="replace"))
    repairs_dir = run_dir / "workspace" / ".commandagent" / "repairs"
    if repairs_dir.is_dir():
        for path in sorted(repairs_dir.glob("*.md")):
            parts.append(path.read_text(encoding="utf-8", errors="replace"))
    return "\n".join(parts)


def write_summary(path, rows):
    fieldnames = [
        "case_id",
        "run",
        "rc",
        "elapsed_ms",
        "success",
        "reason",
        "failure_category",
        "contract_layer",
        "timeout_mode",
        "effective_timeout_secs",
        *MATRIX_FIELD_NAMES,
        *OBSERVATION_FIELD_NAMES,
        *RUNTIME_JOB_REPORT_FIELD_NAMES,
        "active_job",
        "active_job_lifecycle",
        "recovery_owner",
        "loop_control_action",
        "dispatch_status",
        "dispatch_reason",
        "candidate_jobs",
        "tie_break_reason",
        "target_path",
        "target_role",
        "target_candidate_count",
        "target_admitted_count",
        "target_rejected_count",
        "selected_target",
        "selected_target_role",
        "target_rejection_reasons",
        "target_source_of_truth",
        "target_ownership_source",
        "target_workspace_scope",
        "target_evidence_freshness",
        "focused_edit_status",
        "current_excerpt_available",
        "target_priority_components",
        "target_conflict_reason",
        "selected_failure_cluster",
        "semantic_failure_kind",
        "diagnostic_failure_kind",
        "semantic_cluster_source_of_truth",
        "preferred_repair_role",
        "observed_expected",
        "affected_cases",
        "candidate_artifacts",
        "weak_verifier_reason",
        "contract_conflict_status",
        "contract_conflict_sides",
        "contract_conflict_authority",
        "contract_conflict_repair_target_side",
        "contract_conflict_selected_action",
        "contract_conflict_safe_stop_reason",
        "contract_conflict_missing_evidence",
        "contract_conflict_source_of_truth",
        "admitted_cluster_targets",
        "unknown_diagnostic_count",
        "task_contract_kind",
        "task_contract_status",
        "task_contract_lifecycle",
        "task_contract_request_signals",
        "task_contract_constraints",
        "task_contract_completion_evidence",
        "behavior_obligation_codes",
        "behavior_obligation_status",
        "behavior_obligation_owners",
        "behavior_obligation_paths",
        "artifact_role_projection_status",
        "repair_brief_status",
        "action_envelope_status",
        "allowed_change_kind",
        "allowed_tool_category",
        "repair_root_cause",
        "repair_hypothesis",
        "expected_improvement",
        "target_confidence",
        "must_preserve",
        "disallowed_actions",
        "success_check",
        "repair_plan_rejection_reason",
        "repair_action",
        "tool_policy",
        "repair_attempt_count",
        "attempt_outcome",
        "attempt_outcome_reason",
        "before_signature",
        "after_signature",
        "exhausted_targets",
        "exhausted_roles",
        "exhausted_clusters",
        "no_progress_strategy",
        "repair_state_status",
        "safe_stop_payload",
        "patch_validation_status",
        "patch_validation_source",
        "patch_validation_outcomes",
        "patch_validation_rejected_paths",
        "mechanical_adapter",
        "mechanical_adapter_status",
        "mechanical_adapter_action",
        "rollback_admission_status",
        "rollback_reason",
        "tool_protocol_status",
        "tool_protocol_source",
        "tool_protocol_action",
        "tool_protocol_failed_tool",
        "tool_protocol_missing_field",
        "tool_protocol_required_fields",
        "tool_protocol_correction_spent",
        "tool_protocol_correction_exhausted",
        "evidence_binding_status",
        "completion_evidence_status",
        "explicit_stop_reason",
        "runtime_job_kind",
        "runtime_job_outcome",
        "setup_job_kind",
        "setup_job_state",
        "setup_target",
        "setup_manifest_kind",
        "setup_manifest_path",
        "setup_artifact_validation_status",
        "setup_readiness",
        "setup_command_authority",
        "setup_attempt_key",
        "setup_manifest_fingerprint",
        "setup_stale_reason",
        "setup_result",
        "setup_failure_signature",
        "setup_command",
        "verifier_rerun_result",
        "dev_server_state",
        "requested_port",
        "port_preflight",
        "endpoint_smoke",
        "profile_project_kind",
        "profile_manifest_artifacts",
        "profile_entrypoints",
        "profile_integration_artifacts",
        "profile_completion_evidence",
        "profile_failure_mapping",
        "profile_adapter_families",
        "profile_capability_status",
        "phase29_support_rows",
        "language_repair_adapter_status",
        "effective_tool_policy",
        "effective_tool_policy_status",
        "tool_failure_recovery_status",
        "setup_command_classification",
        "command_authority",
        "command_classification_reason",
        "workspace_candidate_status",
        "workspace_ignored_dir_policy",
        "workspace_candidate_ignored_reasons",
        "job_report_status",
        "job_report_owner_action",
        "scaffold_contract_status",
        "noncoding_evidence_status",
        "answer_work_mode_status",
        "lifecycle_projection_status",
        "provider_boundary_status",
        *ASSERTION_FIELD_NAMES,
    ]
    with open(path, "w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, delimiter="\t", fieldnames=fieldnames)
        writer.writeheader()
        writer.writerows(rows)


def recheck(root, cases):
    rows = []
    for meta_path in sorted(root.glob("*/*/meta.json")):
        meta = json.loads(meta_path.read_text(encoding="utf-8"))
        evidence = failure_evidence(meta_path.parent)
        case = cases.get(
            meta["case_id"],
            {"required_paths": [], "must_include": {}, "type": "semantic"},
        )
        workspace = meta_path.parent / "workspace"
        missing = [
            path for path in case["required_paths"] if not (workspace / path).exists()
        ]
        mismatches = semantic_mismatches(workspace, case, missing)
        rc = int(meta.get("rc", 1))
        success = rc == 0 and not missing and not mismatches
        reason = (
            "ok"
            if success
            else (
                "semantic_missing:" + ",".join(missing)
                if missing
                else (
                    "semantic_mismatch:" + ",".join(mismatches)
                    if mismatches
                    else f"rc:{rc}"
                )
            )
        )
        rows.append(
            {
                "case_id": meta["case_id"],
                "run": str(meta["run_index"]),
                "rc": str(rc),
                "elapsed_ms": str(meta.get("elapsed_ms", 0)),
                "success": str(success).lower(),
                "reason": reason,
                "failure_category": categorize(reason),
                "contract_layer": contract_layer(reason),
                "timeout_mode": meta.get("timeout_mode", ""),
                "effective_timeout_secs": (
                    ""
                    if meta.get("effective_timeout_secs") is None
                    else str(meta.get("effective_timeout_secs", ""))
                ),
                "matrix_row": meta.get("matrix_row", case.get("matrix_row", meta["case_id"])),
                "proof_mode": meta.get("proof_mode", case.get("proof_mode", "real_llm")),
                "active_job": meta.get("active_job", derive_active_job(reason)),
                "active_job_lifecycle": meta.get(
                    "active_job_lifecycle",
                    derive_active_job_lifecycle(
                        meta.get("active_job", derive_active_job(reason)),
                        meta.get("dispatch_status", derive_dispatch_status(reason)),
                    ),
                ),
                "recovery_owner": meta.get("recovery_owner", derive_recovery_owner(reason)),
                "loop_control_action": meta.get("loop_control_action", derive_loop_control_action(reason)),
                "dispatch_status": meta.get("dispatch_status", derive_dispatch_status(reason)),
                "dispatch_reason": meta.get("dispatch_reason", ""),
                "candidate_jobs": meta.get("candidate_jobs", ""),
                "tie_break_reason": meta.get("tie_break_reason", ""),
                "target_path": (
                    meta.get("target_path")
                    or contract_value(evidence, "repair_target")
                    or contract_value(evidence, "target_path")
                    or first_reason_target(reason)
                ),
                "target_role": (
                    meta.get("target_role")
                    or contract_value(evidence, "artifact_role")
                    or artifact_role_for_path(first_reason_target(reason))
                ),
                "target_candidate_count": meta.get("target_candidate_count", ""),
                "target_admitted_count": meta.get("target_admitted_count", ""),
                "target_rejected_count": meta.get("target_rejected_count", ""),
                "selected_target": (
                    meta.get("selected_target")
                    or contract_value(evidence, "repair_target")
                    or contract_value(evidence, "target_path")
                    or meta.get("target_path")
                    or first_reason_target(reason)
                ),
                "selected_target_role": (
                    meta.get("selected_target_role")
                    or contract_value(evidence, "artifact_role")
                    or meta.get("target_role")
                    or artifact_role_for_path(first_reason_target(reason))
                ),
                "target_rejection_reasons": meta.get("target_rejection_reasons", ""),
                "target_source_of_truth": meta.get("target_source_of_truth", ""),
                "target_ownership_source": meta.get("target_ownership_source", ""),
                "target_workspace_scope": meta.get("target_workspace_scope", ""),
                "target_evidence_freshness": meta.get("target_evidence_freshness", ""),
                "focused_edit_status": meta.get("focused_edit_status", ""),
                "current_excerpt_available": meta.get("current_excerpt_available", ""),
                "target_priority_components": meta.get("target_priority_components", ""),
                "target_conflict_reason": meta.get("target_conflict_reason", ""),
                "selected_failure_cluster": meta.get("selected_failure_cluster", ""),
                "semantic_failure_kind": meta.get("semantic_failure_kind", ""),
                "diagnostic_failure_kind": meta.get("diagnostic_failure_kind", ""),
                "semantic_cluster_source_of_truth": meta.get("semantic_cluster_source_of_truth", ""),
                "preferred_repair_role": meta.get("preferred_repair_role", ""),
                "observed_expected": meta.get("observed_expected", ""),
                "affected_cases": meta.get("affected_cases", ""),
                "candidate_artifacts": meta.get("candidate_artifacts", ""),
                "weak_verifier_reason": meta.get("weak_verifier_reason", ""),
                "contract_conflict_status": meta.get("contract_conflict_status", ""),
                "contract_conflict_sides": meta.get("contract_conflict_sides", ""),
                "contract_conflict_authority": meta.get("contract_conflict_authority", ""),
                "contract_conflict_repair_target_side": meta.get(
                    "contract_conflict_repair_target_side", ""
                ),
                "contract_conflict_selected_action": meta.get(
                    "contract_conflict_selected_action", ""
                ),
                "contract_conflict_safe_stop_reason": meta.get(
                    "contract_conflict_safe_stop_reason", ""
                ),
                "contract_conflict_missing_evidence": meta.get(
                    "contract_conflict_missing_evidence", ""
                ),
                "contract_conflict_source_of_truth": meta.get(
                    "contract_conflict_source_of_truth", ""
                ),
                "admitted_cluster_targets": meta.get("admitted_cluster_targets", ""),
                "unknown_diagnostic_count": meta.get("unknown_diagnostic_count", ""),
                "task_contract_kind": meta.get("task_contract_kind", ""),
                "task_contract_status": meta.get("task_contract_status", ""),
                "task_contract_lifecycle": meta.get("task_contract_lifecycle", ""),
                "task_contract_request_signals": meta.get(
                    "task_contract_request_signals", ""
                ),
                "task_contract_constraints": meta.get("task_contract_constraints", ""),
                "task_contract_completion_evidence": meta.get(
                    "task_contract_completion_evidence", ""
                ),
                "behavior_obligation_codes": meta.get("behavior_obligation_codes", ""),
                "behavior_obligation_status": meta.get("behavior_obligation_status", ""),
                "behavior_obligation_owners": meta.get("behavior_obligation_owners", ""),
                "behavior_obligation_paths": meta.get("behavior_obligation_paths", ""),
                "artifact_role_projection_status": meta.get("artifact_role_projection_status", ""),
                "repair_brief_status": meta.get("repair_brief_status", ""),
                "action_envelope_status": meta.get("action_envelope_status", ""),
                "allowed_change_kind": meta.get("allowed_change_kind", ""),
                "allowed_tool_category": meta.get("allowed_tool_category", ""),
                "repair_root_cause": meta.get("repair_root_cause", ""),
                "repair_hypothesis": meta.get("repair_hypothesis", ""),
                "expected_improvement": meta.get("expected_improvement", ""),
                "target_confidence": meta.get("target_confidence", ""),
                "must_preserve": meta.get("must_preserve", ""),
                "disallowed_actions": meta.get("disallowed_actions", ""),
                "success_check": meta.get("success_check", ""),
                "repair_plan_rejection_reason": meta.get(
                    "repair_plan_rejection_reason", ""
                ),
                "repair_action": meta.get("repair_action", derive_repair_action(reason)),
                "tool_policy": meta.get("tool_policy", derive_tool_policy(reason)),
                "repair_attempt_count": meta.get("repair_attempt_count", "0"),
                "attempt_outcome": meta.get("attempt_outcome", "not_attempted" if reason != "ok" else "passed"),
                "attempt_outcome_reason": meta.get("attempt_outcome_reason", ""),
                "before_signature": meta.get("before_signature", ""),
                "after_signature": meta.get("after_signature", ""),
                "exhausted_targets": meta.get("exhausted_targets", ""),
                "exhausted_roles": meta.get("exhausted_roles", ""),
                "exhausted_clusters": meta.get("exhausted_clusters", ""),
                "no_progress_strategy": meta.get("no_progress_strategy", ""),
                "repair_state_status": meta.get("repair_state_status", "not_attempted" if reason != "ok" else "passed"),
                "safe_stop_payload": meta.get("safe_stop_payload", ""),
                "patch_validation_status": meta.get("patch_validation_status", ""),
                "patch_validation_source": meta.get("patch_validation_source", ""),
                "patch_validation_outcomes": meta.get("patch_validation_outcomes", ""),
                "patch_validation_rejected_paths": meta.get("patch_validation_rejected_paths", ""),
                "mechanical_adapter": meta.get("mechanical_adapter", ""),
                "mechanical_adapter_status": meta.get("mechanical_adapter_status", ""),
                "mechanical_adapter_action": meta.get("mechanical_adapter_action", ""),
                "rollback_admission_status": meta.get("rollback_admission_status", ""),
                "rollback_reason": meta.get("rollback_reason", ""),
                "tool_protocol_status": meta.get("tool_protocol_status", ""),
                "tool_protocol_source": meta.get("tool_protocol_source", ""),
                "tool_protocol_action": meta.get("tool_protocol_action", ""),
                "tool_protocol_failed_tool": meta.get("tool_protocol_failed_tool", ""),
                "tool_protocol_missing_field": meta.get("tool_protocol_missing_field", ""),
                "tool_protocol_required_fields": meta.get("tool_protocol_required_fields", ""),
                "tool_protocol_correction_spent": meta.get("tool_protocol_correction_spent", ""),
                "tool_protocol_correction_exhausted": meta.get("tool_protocol_correction_exhausted", ""),
                "evidence_binding_status": meta.get("evidence_binding_status", "unknown" if reason != "ok" else "bound"),
                "completion_evidence_status": meta.get("completion_evidence_status", "unknown" if reason != "ok" else "passed"),
                "explicit_stop_reason": meta.get("explicit_stop_reason", ""),
                "runtime_job_kind": meta.get("runtime_job_kind", ""),
                "runtime_job_outcome": meta.get("runtime_job_outcome", ""),
                "setup_job_kind": meta.get("setup_job_kind", ""),
                "setup_job_state": meta.get("setup_job_state", ""),
                "setup_target": meta.get("setup_target", ""),
                "setup_manifest_kind": meta.get("setup_manifest_kind", ""),
                "setup_manifest_path": meta.get("setup_manifest_path", ""),
                "setup_artifact_validation_status": meta.get("setup_artifact_validation_status", ""),
                "setup_readiness": meta.get("setup_readiness", ""),
                "setup_command_authority": meta.get("setup_command_authority", ""),
                "setup_attempt_key": meta.get("setup_attempt_key", ""),
                "setup_manifest_fingerprint": meta.get("setup_manifest_fingerprint", ""),
                "setup_stale_reason": meta.get("setup_stale_reason", ""),
                "setup_result": meta.get("setup_result", ""),
                "setup_failure_signature": meta.get("setup_failure_signature", ""),
                "setup_command": meta.get("setup_command", ""),
                "verifier_rerun_result": meta.get("verifier_rerun_result", ""),
                "dev_server_state": meta.get("dev_server_state", ""),
                "requested_port": meta.get("requested_port", ""),
                "port_preflight": meta.get("port_preflight", ""),
                "endpoint_smoke": meta.get("endpoint_smoke", ""),
                "profile_project_kind": meta.get("profile_project_kind", ""),
                "profile_manifest_artifacts": meta.get("profile_manifest_artifacts", ""),
                "profile_entrypoints": meta.get("profile_entrypoints", ""),
                "profile_integration_artifacts": meta.get("profile_integration_artifacts", ""),
                "profile_completion_evidence": meta.get("profile_completion_evidence", ""),
                "profile_failure_mapping": meta.get("profile_failure_mapping", ""),
                "profile_adapter_families": meta.get("profile_adapter_families", ""),
                "profile_capability_status": meta.get("profile_capability_status", ""),
                "phase29_support_rows": meta.get("phase29_support_rows", ""),
                "language_repair_adapter_status": meta.get(
                    "language_repair_adapter_status", ""
                ),
                "effective_tool_policy": meta.get("effective_tool_policy", ""),
                "effective_tool_policy_status": meta.get(
                    "effective_tool_policy_status", ""
                ),
                "tool_failure_recovery_status": meta.get(
                    "tool_failure_recovery_status", ""
                ),
                "setup_command_classification": meta.get(
                    "setup_command_classification", ""
                ),
                "command_authority": meta.get("command_authority", ""),
                "command_classification_reason": meta.get(
                    "command_classification_reason", ""
                ),
                "workspace_candidate_status": meta.get("workspace_candidate_status", ""),
                "workspace_ignored_dir_policy": meta.get(
                    "workspace_ignored_dir_policy", ""
                ),
                "workspace_candidate_ignored_reasons": meta.get(
                    "workspace_candidate_ignored_reasons", ""
                ),
                "job_report_status": meta.get("job_report_status", ""),
                "job_report_owner_action": meta.get("job_report_owner_action", ""),
                "scaffold_contract_status": meta.get("scaffold_contract_status", ""),
                "noncoding_evidence_status": meta.get("noncoding_evidence_status", ""),
                "answer_work_mode_status": meta.get("answer_work_mode_status", ""),
                "lifecycle_projection_status": meta.get(
                    "lifecycle_projection_status", ""
                ),
                "provider_boundary_status": meta.get("provider_boundary_status", ""),
            }
        )
        observation_input = {
            **meta,
            **rows[-1],
            "reason": reason,
            "success": success,
            "rc": rc,
            "evidence": evidence,
        }
        for name in OBSERVATION_FIELD_NAMES:
            observation_input[name] = ""
        observation = normalize_observation(observation_input)
        rows[-1].update(
            {name: observation.get(name, "") for name in OBSERVATION_FIELD_NAMES}
        )
        rows[-1]["failure_category"] = observation["failure_category"]
        rows[-1]["contract_layer"] = observation["contract_layer"]
        rows[-1].update(
            build_runtime_job_report(
                rows[-1],
                dry_run=bool(meta.get("dry_run")),
                recheck=True,
            )
        )
        observed_fields = {
            "failure_category": rows[-1].get("failure_category", ""),
            "failure_class": rows[-1].get("failure_class", ""),
            "contract_layer": rows[-1].get("contract_layer", ""),
            **{name: rows[-1].get(name, "") for name in OBSERVATION_FIELD_NAMES},
            **{key: rows[-1].get(key, "") for key in rows[-1].keys()},
        }
        rows[-1].update(
            focused_assertions(
                case.get("expected_fields", {}),
                observed_fields,
                dry_run=bool(meta.get("dry_run")),
                recheck=True,
            )
        )
    out = root / "recheck_summary.tsv"
    write_summary(out, rows)
    return rows, out


def semantic_mismatches(workspace, case, missing):
    mismatches = []
    for path, needles in case.get("must_include", {}).items():
        target = workspace / path
        if not target.exists():
            if path not in missing:
                missing.append(path)
            continue
        text = target.read_text(encoding="utf-8", errors="replace")
        for needle in needles:
            if not semantic_contains(text, needle, case):
                mismatches.append(f"{path}:{needle}")
    return mismatches


def semantic_contains(text, needle, case):
    if case.get("type") == "semantic":
        return needle.casefold() in text.casefold()
    return needle in text


def categorize(reason):
    return category_for_reason(reason)


def contract_layer(reason):
    return contract_layer_for_reason(reason)


def derive_active_job(reason):
    category = categorize(reason)
    if reason == "ok":
        return "none"
    if reason == "port_in_use" or "EADDRINUSE" in reason or "address already in use" in reason:
        return "dev_server_smoke"
    if category == "setup":
        return "setup_bootstrap"
    if category == "tool_protocol":
        return "tool_protocol_correction"
    if category == "step_policy":
        return "tool_protocol_correction"
    if category == "profile" and ("route" in reason or "integration" in reason):
        return "route_integration_repair"
    if category == "profile" and (
        "dependency" in reason or "version_conflict" in reason or "manifest" in reason
    ):
        return "manifest_repair"
    if category == "planning" and first_reason_target(reason):
        role = artifact_role_for_path(first_reason_target(reason))
        if role == "test":
            return "test_artifact_completion"
        if role == "docs":
            return "documentation_repair"
        if role in {"setup_manifest", "setup_config"}:
            return "manifest_repair"
        return "scaffold_materialization"
    if category == "planning":
        return "verifier_contract_correction"
    if category in {"quality", "verifier", "profile"}:
        return "source_implementation_repair"
    return "explicit_stop"


def derive_recovery_owner(reason):
    job = derive_active_job(reason)
    if job == "setup_bootstrap":
        return "setup"
    if job == "dev_server_smoke":
        return "dev_server"
    if job == "manifest_repair":
        return "manifest"
    if job == "scaffold_materialization":
        return "scaffold"
    if job == "route_integration_repair":
        return "route_integration"
    if job == "test_artifact_completion":
        return "test"
    if job == "documentation_repair":
        return "docs"
    if job == "tool_protocol_correction":
        return "tool_protocol"
    if job == "verifier_contract_correction":
        return "verifier_contract"
    if job == "none":
        return "none"
    if job == "explicit_stop":
        return "explicit_stop"
    return "source"


def derive_loop_control_action(reason):
    job = derive_active_job(reason)
    if job == "setup_bootstrap":
        return "run_verifier_owned_setup"
    if job == "dev_server_smoke":
        return "run_dev_server_smoke"
    if job == "tool_protocol_correction":
        return "run_tool_protocol_correction"
    if job in {"explicit_stop", "contract_conflict"}:
        return "render_explicit_stop"
    if job == "none":
        return "none"
    return "run_bounded_repair_task"


def derive_dispatch_status(reason):
    job = derive_active_job(reason)
    if job == "none":
        return "selected"
    if job in {"explicit_stop", "contract_conflict"}:
        return "explicit_stop"
    return "selected"


def derive_active_job_lifecycle(active_job, dispatch_status):
    if active_job in {"", "none"}:
        return "not_applicable"
    if dispatch_status == "no_owner":
        return "no_owner"
    if dispatch_status == "ambiguous_tie":
        return "ambiguous_tie"
    if active_job == "contract_conflict":
        return "conflict_stop"
    if dispatch_status == "explicit_stop" or active_job == "explicit_stop":
        return "explicit_stop"
    return "selected"


def derive_repair_action(reason):
    job = derive_active_job(reason)
    if job == "setup_bootstrap":
        return "install_or_prepare_dependencies"
    if job == "dev_server_smoke":
        return "run_dev_server_smoke"
    if job == "manifest_repair":
        if "conflict" in reason:
            return "resolve_manifest_conflict"
        return "add_missing_manifest_dependency"
    if job in {"scaffold_materialization", "test_artifact_completion"}:
        return "create_required_artifact"
    if job == "route_integration_repair":
        return "connect_existing_artifact_to_entrypoint"
    if job == "documentation_repair":
        return "update_docs_literal"
    if job == "tool_protocol_correction":
        return "correct_tool_protocol"
    if job == "verifier_contract_correction":
        return "replace_invalid_verifier_command"
    if job == "explicit_stop":
        return "stop_with_structured_evidence"
    if job == "none":
        return "none"
    return "edit_source_for_diagnostic"


def derive_tool_policy(reason):
    job = derive_active_job(reason)
    if job == "setup_bootstrap":
        return "verifier_owned_setup_only"
    if job == "dev_server_smoke":
        return "verifier_owned_setup_only"
    if job == "manifest_repair":
        return "setup_config_mutation_only"
    if job in {"tool_protocol_correction", "explicit_stop", "none"}:
        return job
    if job == "verifier_contract_correction":
        return "read_only"
    return "file_mutation_repair"


def first_reason_target(reason):
    for prefix in ["missing:", "semantic_missing:", "semantic_mismatch:"]:
        if reason.startswith(prefix):
            value = reason[len(prefix):].split(",", 1)[0]
            if ":" in value and prefix == "semantic_mismatch:":
                value = value.split(":", 1)[0]
            if "/" in value or "." in value:
                return value
    return ""


def artifact_role_for_path(path):
    if not path:
        return ""
    name = path.rsplit("/", 1)[-1]
    if name in {"package.json", "Cargo.toml", "pyproject.toml"} or name.startswith("requirements"):
        return "setup_manifest"
    if name.startswith(("next.config.", "postcss.config.", "tailwind.config.")):
        return "setup_config"
    if path in {"app/page.tsx", "src/app/page.tsx", "app/layout.tsx"}:
        return "entrypoint"
    if path.startswith("tests/") or "test" in name or name.endswith("_test.rs"):
        return "test"
    if name.endswith(".md"):
        return "docs"
    if path.startswith(("raw/", "data/raw/", "input/", "inputs/")):
        return "raw_input"
    if path.startswith(("data/processed/", "reports/")):
        return "derived_output"
    if name.endswith((".json", ".csv", ".yaml", ".yml")):
        return "structured_data"
    if name.endswith((".ts", ".tsx", ".js", ".jsx", ".rs", ".py")):
        return "implementation"
    return "unknown"


def render_report(rows):
    total = len(rows)
    success = sum(1 for row in rows if row["success"] == "true")
    categories = {}
    layers = {}
    terminal_states = {}
    diagnostics = {}
    producers = {}
    guards = {}
    actionabilities = {}
    observation_defects = []
    by_case = {}
    recovery_jobs = {}
    runtime_jobs = {}
    loop_control_actions = {}
    dispatch_statuses = {}
    repair_brief_statuses = {}
    action_envelope_statuses = {}
    allowed_change_kinds = {}
    allowed_tool_categories = {}
    target_confidences = {}
    repair_plan_rejection_reasons = {}
    selected_failure_clusters = {}
    semantic_failure_kinds = {}
    diagnostic_failure_kinds = {}
    semantic_cluster_sources = {}
    preferred_repair_roles = {}
    observed_expected_pairs = {}
    affected_cases = {}
    candidate_artifacts = {}
    weak_verifier_reasons = {}
    contract_conflict_statuses = {}
    contract_conflict_authorities = {}
    contract_conflict_target_sides = {}
    contract_conflict_actions = {}
    contract_conflict_safe_stop_reasons = {}
    contract_conflict_sources = {}
    admitted_cluster_targets = {}
    unknown_diagnostic_total = 0
    tool_protocol_statuses = {}
    tool_protocol_sources = {}
    tool_protocol_actions = {}
    tool_protocol_failed_tools = {}
    tool_protocol_missing_fields = {}
    tool_protocol_required_fields = {}
    tool_protocol_correction_spent = {}
    tool_protocol_correction_exhausted = {}
    patch_validation_statuses = {}
    patch_validation_outcomes = {}
    patch_validation_sources = {}
    patch_validation_rejected_paths = {}
    mechanical_adapters = {}
    mechanical_adapter_statuses = {}
    mechanical_adapter_actions = {}
    profile_project_kinds = {}
    profile_manifest_artifacts = {}
    profile_entrypoints = {}
    profile_integration_artifacts = {}
    profile_completion_evidence = {}
    profile_failure_mappings = {}
    profile_adapter_families = {}
    profile_capability_statuses = {}
    phase29_support_rows = {}
    language_repair_adapter_statuses = {}
    effective_tool_policies = {}
    effective_tool_policy_statuses = {}
    tool_failure_recovery_statuses = {}
    setup_command_classifications = {}
    command_authorities = {}
    command_classification_reasons = {}
    workspace_candidate_statuses = {}
    workspace_ignored_dir_policies = {}
    workspace_candidate_ignored_reasons = {}
    job_report_statuses = {}
    job_report_owner_actions = {}
    scaffold_contract_statuses = {}
    noncoding_evidence_statuses = {}
    answer_work_mode_statuses = {}
    lifecycle_projection_statuses = {}
    provider_boundary_statuses = {}
    rollback_admission_statuses = {}
    rollback_reasons = {}
    task_contract_kinds = {}
    task_contract_statuses = {}
    task_contract_lifecycles = {}
    task_contract_request_signals = {}
    task_contract_constraints = {}
    task_contract_completion_evidence = {}
    behavior_obligation_statuses = {}
    behavior_obligation_owners = {}
    behavior_obligation_paths = {}
    artifact_role_projection_statuses = {}
    completion_authority_statuses = {}
    completion_source_of_truths = {}
    lifecycle_stages = {}
    active_owners = {}
    selected_actions = {}
    target_admission_statuses = {}
    repair_action_plan_statuses = {}
    completion_sources = {}
    attempt_outcomes = {}
    verifier_rerun_results = {}
    explicit_stop_reasons = {}
    evidence_runner_statuses = {}
    evidence_runner_kinds = {}
    evidence_binding_kinds = {}
    freshness_statuses = {}
    artifact_ledger_statuses = {}
    artifact_ledger_sources = {}
    workspace_scope_kinds = {}
    deliverable_obligation_kinds = {}
    deliverable_obligation_paths = {}
    artifact_ownerships = {}
    artifact_source_of_truths = {}
    rejected_target_reasons = {}
    artifact_ledger_signal_counts = {
        "read_paths": 0,
        "changed_paths": 0,
        "created_paths": 0,
        "verifier_mentioned_paths": 0,
        "scaffold_created_paths": 0,
        "setup_created_paths": 0,
        "out_of_scope_paths": 0,
    }
    focused_assertion_statuses = {}
    focused_assertion_failures = []
    matrix_rows = {}
    proof_modes = {}
    for row in rows:
        observation = normalize_observation(row)
        runtime_job_report = build_runtime_job_report(row)
        category = row.get("failure_category") or observation["failure_category"]
        layer = row.get("contract_layer") or observation["contract_layer"]
        terminal_state = row.get("terminal_state") or observation["terminal_state"]
        diagnostic_code = row.get("diagnostic_code") or observation["diagnostic_code"]
        producer = row.get("producer") or observation.get("producer", "")
        guard = row.get("guard") or observation.get("guard", "")
        actionability = row.get("actionability") or observation.get("actionability", "")
        evidence_runner_status = (
            row.get("evidence_runner_status") or observation["evidence_runner_status"]
        )
        lifecycle_stage = row.get("lifecycle_stage") or runtime_job_report["lifecycle_stage"]
        active_owner = row.get("active_owner") or runtime_job_report["active_owner"]
        selected_action = (
            row.get("selected_action") or runtime_job_report["selected_action"]
        )
        target_admission_status = (
            row.get("target_admission_status")
            or runtime_job_report["target_admission_status"]
        )
        repair_action_plan_status = (
            row.get("repair_action_plan_status")
            or runtime_job_report["repair_action_plan_status"]
        )
        completion_source = (
            row.get("completion_source") or runtime_job_report["completion_source"]
        )
        attempt_outcome = row.get("attempt_outcome", "")
        verifier_rerun_result = row.get("verifier_rerun_result", "")
        explicit_stop_reason = row.get("explicit_stop_reason", "")
        completion_authority_status = (
            row.get("completion_authority_status")
            or observation.get("completion_authority_status", "")
        )
        completion_source_of_truth = (
            row.get("completion_source_of_truth")
            or observation.get("completion_source_of_truth", "")
        )
        evidence_runner_kind = (
            row.get("evidence_runner_kind") or observation.get("evidence_runner_kind", "")
        )
        evidence_binding_kind = (
            row.get("evidence_binding_kind") or observation.get("evidence_binding_kind", "")
        )
        freshness_status = row.get("freshness_status") or observation.get(
            "freshness_status", ""
        )
        artifact_ledger_status = (
            row.get("artifact_ledger_status") or observation["artifact_ledger_status"]
        )
        artifact_ledger_source = (
            row.get("artifact_ledger_sources")
            or observation.get("artifact_ledger_sources", "")
        )
        workspace_scope_kind = (
            row.get("workspace_scope_kind") or observation.get("workspace_scope_kind", "")
        )
        deliverable_obligation_kind = (
            row.get("deliverable_obligation_kind")
            or observation.get("deliverable_obligation_kind", "")
        )
        deliverable_obligation_path = (
            row.get("deliverable_obligation_path")
            or observation.get("deliverable_obligation_path", "")
        )
        artifact_ownership = (
            row.get("artifact_ownership") or observation.get("artifact_ownership", "")
        )
        artifact_source_of_truth = (
            row.get("artifact_source_of_truth")
            or observation.get("artifact_source_of_truth", "")
        )
        rejected_target_reason = (
            row.get("rejected_target_reason")
            or observation.get("rejected_target_reason", "")
        )
        job = row.get("active_job") or derive_active_job(row["reason"])
        runtime_job = row.get("runtime_job_kind", "")
        loop_control_action = row.get("loop_control_action") or derive_loop_control_action(
            row["reason"]
        )
        dispatch_status = row.get("dispatch_status") or derive_dispatch_status(row["reason"])
        repair_brief_status = row.get("repair_brief_status", "")
        action_envelope_status = row.get("action_envelope_status", "")
        allowed_change_kind = row.get("allowed_change_kind", "")
        allowed_tool_category = row.get("allowed_tool_category", "")
        target_confidence = row.get("target_confidence", "")
        repair_plan_rejection_reason = row.get("repair_plan_rejection_reason", "")
        selected_failure_cluster = row.get("selected_failure_cluster", "")
        semantic_failure_kind = row.get("semantic_failure_kind", "")
        diagnostic_failure_kind = row.get("diagnostic_failure_kind", "")
        semantic_cluster_source = row.get("semantic_cluster_source_of_truth", "")
        preferred_repair_role = row.get("preferred_repair_role", "")
        observed_expected = row.get("observed_expected", "")
        affected_case = row.get("affected_cases", "")
        candidate_artifact = row.get("candidate_artifacts", "")
        weak_verifier_reason = row.get("weak_verifier_reason", "")
        contract_conflict_status = row.get("contract_conflict_status", "")
        contract_conflict_authority = row.get("contract_conflict_authority", "")
        contract_conflict_target_side = row.get(
            "contract_conflict_repair_target_side", ""
        )
        contract_conflict_action = row.get("contract_conflict_selected_action", "")
        contract_conflict_safe_stop_reason = row.get(
            "contract_conflict_safe_stop_reason", ""
        )
        contract_conflict_source = row.get("contract_conflict_source_of_truth", "")
        admitted_targets = row.get("admitted_cluster_targets", "")
        unknown_diagnostic_count = row.get("unknown_diagnostic_count", "")
        tool_protocol_status = row.get("tool_protocol_status", "")
        tool_protocol_source = row.get("tool_protocol_source", "")
        tool_protocol_action = row.get("tool_protocol_action", "")
        tool_protocol_failed_tool = row.get("tool_protocol_failed_tool", "")
        tool_protocol_missing_field = row.get("tool_protocol_missing_field", "")
        tool_protocol_required_field = row.get("tool_protocol_required_fields", "")
        protocol_correction_spent = row.get("tool_protocol_correction_spent", "")
        protocol_correction_exhausted = row.get(
            "tool_protocol_correction_exhausted", ""
        )
        patch_validation_status = row.get("patch_validation_status", "")
        patch_validation_source = row.get("patch_validation_source", "")
        patch_validation_outcome = row.get("patch_validation_outcomes", "")
        patch_validation_rejected_path = row.get("patch_validation_rejected_paths", "")
        mechanical_adapter = row.get("mechanical_adapter", "")
        mechanical_adapter_status = row.get("mechanical_adapter_status", "")
        mechanical_adapter_action = row.get("mechanical_adapter_action", "")
        profile_project_kind = row.get("profile_project_kind", "")
        profile_manifest_artifact = row.get("profile_manifest_artifacts", "")
        profile_entrypoint = row.get("profile_entrypoints", "")
        profile_integration_artifact = row.get("profile_integration_artifacts", "")
        profile_completion_evidence_value = row.get("profile_completion_evidence", "")
        profile_failure_mapping = row.get("profile_failure_mapping", "")
        profile_adapter_family = row.get("profile_adapter_families", "")
        profile_capability_status = row.get("profile_capability_status", "")
        phase29_support_row = row.get("phase29_support_rows", "")
        language_repair_adapter_status = row.get("language_repair_adapter_status", "")
        effective_tool_policy = row.get("effective_tool_policy", "")
        effective_tool_policy_status = row.get("effective_tool_policy_status", "")
        tool_failure_recovery_status = row.get("tool_failure_recovery_status", "")
        setup_command_classification = row.get("setup_command_classification", "")
        command_authority = row.get("command_authority", "")
        command_classification_reason = row.get("command_classification_reason", "")
        workspace_candidate_status = row.get("workspace_candidate_status", "")
        workspace_ignored_dir_policy = row.get("workspace_ignored_dir_policy", "")
        workspace_candidate_ignored_reason = row.get(
            "workspace_candidate_ignored_reasons", ""
        )
        job_report_status = row.get("job_report_status", "")
        job_report_owner_action = row.get("job_report_owner_action", "")
        scaffold_contract_status = row.get("scaffold_contract_status", "")
        noncoding_evidence_status = row.get("noncoding_evidence_status", "")
        answer_work_mode_status = row.get("answer_work_mode_status", "")
        lifecycle_projection_status = row.get("lifecycle_projection_status", "")
        provider_boundary_status = row.get("provider_boundary_status", "")
        rollback_admission_status = row.get("rollback_admission_status", "")
        rollback_reason = row.get("rollback_reason", "")
        task_contract_kind = row.get("task_contract_kind", "")
        task_contract_status = row.get("task_contract_status", "")
        task_contract_lifecycle = row.get("task_contract_lifecycle", "")
        task_contract_request_signal = row.get("task_contract_request_signals", "")
        task_contract_constraint = row.get("task_contract_constraints", "")
        task_contract_completion_evidence_value = row.get(
            "task_contract_completion_evidence", ""
        )
        behavior_obligation_status = row.get("behavior_obligation_status", "")
        behavior_obligation_owner = row.get("behavior_obligation_owners", "")
        behavior_obligation_path = row.get("behavior_obligation_paths", "")
        artifact_role_projection_status = row.get("artifact_role_projection_status", "")
        matrix_row = row.get("matrix_row", "") or row["case_id"]
        proof_mode = row.get("proof_mode", "") or "unknown"
        matrix_rows[matrix_row] = matrix_rows.get(matrix_row, 0) + 1
        proof_modes[proof_mode] = proof_modes.get(proof_mode, 0) + 1
        categories[category] = categories.get(category, 0) + 1
        layers[layer] = layers.get(layer, 0) + 1
        terminal_states[terminal_state] = terminal_states.get(terminal_state, 0) + 1
        diagnostics[diagnostic_code] = diagnostics.get(diagnostic_code, 0) + 1
        if producer:
            producers[producer] = producers.get(producer, 0) + 1
        if guard:
            guards[guard] = guards.get(guard, 0) + 1
        if actionability:
            actionabilities[actionability] = actionabilities.get(actionability, 0) + 1
        defect = observation_defect(row, observation, terminal_state, diagnostic_code)
        if defect:
            observation_defects.append((row["case_id"], defect))
        recovery_jobs[job] = recovery_jobs.get(job, 0) + 1
        if runtime_job:
            runtime_jobs[runtime_job] = runtime_jobs.get(runtime_job, 0) + 1
        loop_control_actions[loop_control_action] = (
            loop_control_actions.get(loop_control_action, 0) + 1
        )
        dispatch_statuses[dispatch_status] = dispatch_statuses.get(dispatch_status, 0) + 1
        if repair_brief_status:
            repair_brief_statuses[repair_brief_status] = (
                repair_brief_statuses.get(repair_brief_status, 0) + 1
            )
        if action_envelope_status:
            action_envelope_statuses[action_envelope_status] = (
                action_envelope_statuses.get(action_envelope_status, 0) + 1
            )
        if allowed_change_kind:
            allowed_change_kinds[allowed_change_kind] = (
                allowed_change_kinds.get(allowed_change_kind, 0) + 1
            )
        if allowed_tool_category:
            allowed_tool_categories[allowed_tool_category] = (
                allowed_tool_categories.get(allowed_tool_category, 0) + 1
            )
        if target_confidence:
            target_confidences[target_confidence] = (
                target_confidences.get(target_confidence, 0) + 1
            )
        if repair_plan_rejection_reason:
            repair_plan_rejection_reasons[repair_plan_rejection_reason] = (
                repair_plan_rejection_reasons.get(repair_plan_rejection_reason, 0) + 1
            )
        if selected_failure_cluster:
            selected_failure_clusters[selected_failure_cluster] = (
                selected_failure_clusters.get(selected_failure_cluster, 0) + 1
            )
        if semantic_failure_kind:
            semantic_failure_kinds[semantic_failure_kind] = (
                semantic_failure_kinds.get(semantic_failure_kind, 0) + 1
            )
        if diagnostic_failure_kind:
            diagnostic_failure_kinds[diagnostic_failure_kind] = (
                diagnostic_failure_kinds.get(diagnostic_failure_kind, 0) + 1
            )
        if semantic_cluster_source:
            semantic_cluster_sources[semantic_cluster_source] = (
                semantic_cluster_sources.get(semantic_cluster_source, 0) + 1
            )
        if preferred_repair_role:
            preferred_repair_roles[preferred_repair_role] = (
                preferred_repair_roles.get(preferred_repair_role, 0) + 1
            )
        if observed_expected:
            observed_expected_pairs[observed_expected] = (
                observed_expected_pairs.get(observed_expected, 0) + 1
            )
        if affected_case:
            affected_cases[affected_case] = affected_cases.get(affected_case, 0) + 1
        if candidate_artifact:
            candidate_artifacts[candidate_artifact] = (
                candidate_artifacts.get(candidate_artifact, 0) + 1
            )
        if weak_verifier_reason:
            weak_verifier_reasons[weak_verifier_reason] = (
                weak_verifier_reasons.get(weak_verifier_reason, 0) + 1
            )
        if contract_conflict_status:
            contract_conflict_statuses[contract_conflict_status] = (
                contract_conflict_statuses.get(contract_conflict_status, 0) + 1
            )
        if contract_conflict_authority:
            contract_conflict_authorities[contract_conflict_authority] = (
                contract_conflict_authorities.get(contract_conflict_authority, 0) + 1
            )
        if contract_conflict_target_side:
            contract_conflict_target_sides[contract_conflict_target_side] = (
                contract_conflict_target_sides.get(contract_conflict_target_side, 0) + 1
            )
        if contract_conflict_action:
            contract_conflict_actions[contract_conflict_action] = (
                contract_conflict_actions.get(contract_conflict_action, 0) + 1
            )
        if contract_conflict_safe_stop_reason:
            contract_conflict_safe_stop_reasons[contract_conflict_safe_stop_reason] = (
                contract_conflict_safe_stop_reasons.get(
                    contract_conflict_safe_stop_reason, 0
                )
                + 1
            )
        if contract_conflict_source:
            contract_conflict_sources[contract_conflict_source] = (
                contract_conflict_sources.get(contract_conflict_source, 0) + 1
            )
        if admitted_targets:
            admitted_cluster_targets[admitted_targets] = (
                admitted_cluster_targets.get(admitted_targets, 0) + 1
            )
        if unknown_diagnostic_count:
            try:
                unknown_diagnostic_total += int(unknown_diagnostic_count)
            except ValueError:
                pass
        if tool_protocol_status:
            tool_protocol_statuses[tool_protocol_status] = (
                tool_protocol_statuses.get(tool_protocol_status, 0) + 1
            )
        if tool_protocol_source:
            tool_protocol_sources[tool_protocol_source] = (
                tool_protocol_sources.get(tool_protocol_source, 0) + 1
            )
        if tool_protocol_action:
            tool_protocol_actions[tool_protocol_action] = (
                tool_protocol_actions.get(tool_protocol_action, 0) + 1
            )
        if tool_protocol_failed_tool:
            tool_protocol_failed_tools[tool_protocol_failed_tool] = (
                tool_protocol_failed_tools.get(tool_protocol_failed_tool, 0) + 1
            )
        if tool_protocol_missing_field:
            tool_protocol_missing_fields[tool_protocol_missing_field] = (
                tool_protocol_missing_fields.get(tool_protocol_missing_field, 0) + 1
            )
        if tool_protocol_required_field:
            tool_protocol_required_fields[tool_protocol_required_field] = (
                tool_protocol_required_fields.get(tool_protocol_required_field, 0) + 1
            )
        if protocol_correction_spent:
            tool_protocol_correction_spent[protocol_correction_spent] = (
                tool_protocol_correction_spent.get(protocol_correction_spent, 0) + 1
            )
        if protocol_correction_exhausted:
            tool_protocol_correction_exhausted[protocol_correction_exhausted] = (
                tool_protocol_correction_exhausted.get(protocol_correction_exhausted, 0)
                + 1
            )
        if patch_validation_status:
            patch_validation_statuses[patch_validation_status] = (
                patch_validation_statuses.get(patch_validation_status, 0) + 1
            )
        if patch_validation_source:
            patch_validation_sources[patch_validation_source] = (
                patch_validation_sources.get(patch_validation_source, 0) + 1
            )
        if patch_validation_outcome:
            patch_validation_outcomes[patch_validation_outcome] = (
                patch_validation_outcomes.get(patch_validation_outcome, 0) + 1
            )
        if patch_validation_rejected_path:
            patch_validation_rejected_paths[patch_validation_rejected_path] = (
                patch_validation_rejected_paths.get(patch_validation_rejected_path, 0) + 1
            )
        if mechanical_adapter:
            mechanical_adapters[mechanical_adapter] = (
                mechanical_adapters.get(mechanical_adapter, 0) + 1
            )
        if mechanical_adapter_status:
            mechanical_adapter_statuses[mechanical_adapter_status] = (
                mechanical_adapter_statuses.get(mechanical_adapter_status, 0) + 1
            )
        if mechanical_adapter_action:
            mechanical_adapter_actions[mechanical_adapter_action] = (
                mechanical_adapter_actions.get(mechanical_adapter_action, 0) + 1
            )
        if profile_project_kind:
            profile_project_kinds[profile_project_kind] = (
                profile_project_kinds.get(profile_project_kind, 0) + 1
            )
        if profile_manifest_artifact:
            profile_manifest_artifacts[profile_manifest_artifact] = (
                profile_manifest_artifacts.get(profile_manifest_artifact, 0) + 1
            )
        if profile_entrypoint:
            profile_entrypoints[profile_entrypoint] = (
                profile_entrypoints.get(profile_entrypoint, 0) + 1
            )
        if profile_integration_artifact:
            profile_integration_artifacts[profile_integration_artifact] = (
                profile_integration_artifacts.get(profile_integration_artifact, 0) + 1
            )
        if profile_completion_evidence_value:
            profile_completion_evidence[profile_completion_evidence_value] = (
                profile_completion_evidence.get(profile_completion_evidence_value, 0) + 1
            )
        if profile_failure_mapping:
            profile_failure_mappings[profile_failure_mapping] = (
                profile_failure_mappings.get(profile_failure_mapping, 0) + 1
            )
        if profile_adapter_family:
            profile_adapter_families[profile_adapter_family] = (
                profile_adapter_families.get(profile_adapter_family, 0) + 1
            )
        if profile_capability_status:
            profile_capability_statuses[profile_capability_status] = (
                profile_capability_statuses.get(profile_capability_status, 0) + 1
            )
        if phase29_support_row:
            phase29_support_rows[phase29_support_row] = (
                phase29_support_rows.get(phase29_support_row, 0) + 1
            )
        if language_repair_adapter_status:
            language_repair_adapter_statuses[language_repair_adapter_status] = (
                language_repair_adapter_statuses.get(language_repair_adapter_status, 0) + 1
            )
        if effective_tool_policy:
            effective_tool_policies[effective_tool_policy] = (
                effective_tool_policies.get(effective_tool_policy, 0) + 1
            )
        if effective_tool_policy_status:
            effective_tool_policy_statuses[effective_tool_policy_status] = (
                effective_tool_policy_statuses.get(effective_tool_policy_status, 0) + 1
            )
        if tool_failure_recovery_status:
            tool_failure_recovery_statuses[tool_failure_recovery_status] = (
                tool_failure_recovery_statuses.get(tool_failure_recovery_status, 0) + 1
            )
        if setup_command_classification:
            setup_command_classifications[setup_command_classification] = (
                setup_command_classifications.get(setup_command_classification, 0) + 1
            )
        if command_authority:
            command_authorities[command_authority] = (
                command_authorities.get(command_authority, 0) + 1
            )
        if command_classification_reason:
            command_classification_reasons[command_classification_reason] = (
                command_classification_reasons.get(command_classification_reason, 0) + 1
            )
        if workspace_candidate_status:
            workspace_candidate_statuses[workspace_candidate_status] = (
                workspace_candidate_statuses.get(workspace_candidate_status, 0) + 1
            )
        if workspace_ignored_dir_policy:
            workspace_ignored_dir_policies[workspace_ignored_dir_policy] = (
                workspace_ignored_dir_policies.get(workspace_ignored_dir_policy, 0) + 1
            )
        if workspace_candidate_ignored_reason:
            workspace_candidate_ignored_reasons[workspace_candidate_ignored_reason] = (
                workspace_candidate_ignored_reasons.get(
                    workspace_candidate_ignored_reason, 0
                )
                + 1
            )
        if job_report_status:
            job_report_statuses[job_report_status] = (
                job_report_statuses.get(job_report_status, 0) + 1
            )
        if job_report_owner_action:
            job_report_owner_actions[job_report_owner_action] = (
                job_report_owner_actions.get(job_report_owner_action, 0) + 1
            )
        if scaffold_contract_status:
            scaffold_contract_statuses[scaffold_contract_status] = (
                scaffold_contract_statuses.get(scaffold_contract_status, 0) + 1
            )
        if noncoding_evidence_status:
            noncoding_evidence_statuses[noncoding_evidence_status] = (
                noncoding_evidence_statuses.get(noncoding_evidence_status, 0) + 1
            )
        if answer_work_mode_status:
            answer_work_mode_statuses[answer_work_mode_status] = (
                answer_work_mode_statuses.get(answer_work_mode_status, 0) + 1
            )
        if lifecycle_projection_status:
            lifecycle_projection_statuses[lifecycle_projection_status] = (
                lifecycle_projection_statuses.get(lifecycle_projection_status, 0) + 1
            )
        if provider_boundary_status:
            provider_boundary_statuses[provider_boundary_status] = (
                provider_boundary_statuses.get(provider_boundary_status, 0) + 1
            )
        if rollback_admission_status:
            rollback_admission_statuses[rollback_admission_status] = (
                rollback_admission_statuses.get(rollback_admission_status, 0) + 1
            )
        if rollback_reason:
            rollback_reasons[rollback_reason] = rollback_reasons.get(rollback_reason, 0) + 1
        if task_contract_kind:
            task_contract_kinds[task_contract_kind] = (
                task_contract_kinds.get(task_contract_kind, 0) + 1
            )
        if task_contract_status:
            task_contract_statuses[task_contract_status] = (
                task_contract_statuses.get(task_contract_status, 0) + 1
            )
        if task_contract_lifecycle:
            task_contract_lifecycles[task_contract_lifecycle] = (
                task_contract_lifecycles.get(task_contract_lifecycle, 0) + 1
            )
        if task_contract_request_signal:
            task_contract_request_signals[task_contract_request_signal] = (
                task_contract_request_signals.get(task_contract_request_signal, 0) + 1
            )
        if task_contract_constraint:
            task_contract_constraints[task_contract_constraint] = (
                task_contract_constraints.get(task_contract_constraint, 0) + 1
            )
        if task_contract_completion_evidence_value:
            task_contract_completion_evidence[task_contract_completion_evidence_value] = (
                task_contract_completion_evidence.get(
                    task_contract_completion_evidence_value, 0
                )
                + 1
            )
        if behavior_obligation_status:
            behavior_obligation_statuses[behavior_obligation_status] = (
                behavior_obligation_statuses.get(behavior_obligation_status, 0) + 1
            )
        if behavior_obligation_owner:
            behavior_obligation_owners[behavior_obligation_owner] = (
                behavior_obligation_owners.get(behavior_obligation_owner, 0) + 1
            )
        if behavior_obligation_path:
            behavior_obligation_paths[behavior_obligation_path] = (
                behavior_obligation_paths.get(behavior_obligation_path, 0) + 1
            )
        if artifact_role_projection_status:
            artifact_role_projection_statuses[artifact_role_projection_status] = (
                artifact_role_projection_statuses.get(artifact_role_projection_status, 0) + 1
            )
        if evidence_runner_status:
            evidence_runner_statuses[evidence_runner_status] = (
                evidence_runner_statuses.get(evidence_runner_status, 0) + 1
            )
        if lifecycle_stage:
            lifecycle_stages[lifecycle_stage] = lifecycle_stages.get(lifecycle_stage, 0) + 1
        if active_owner:
            active_owners[active_owner] = active_owners.get(active_owner, 0) + 1
        if selected_action:
            selected_actions[selected_action] = selected_actions.get(selected_action, 0) + 1
        if target_admission_status:
            target_admission_statuses[target_admission_status] = (
                target_admission_statuses.get(target_admission_status, 0) + 1
            )
        if repair_action_plan_status:
            repair_action_plan_statuses[repair_action_plan_status] = (
                repair_action_plan_statuses.get(repair_action_plan_status, 0) + 1
            )
        if completion_source:
            completion_sources[completion_source] = (
                completion_sources.get(completion_source, 0) + 1
            )
        if attempt_outcome:
            attempt_outcomes[attempt_outcome] = (
                attempt_outcomes.get(attempt_outcome, 0) + 1
            )
        if verifier_rerun_result:
            verifier_rerun_results[verifier_rerun_result] = (
                verifier_rerun_results.get(verifier_rerun_result, 0) + 1
            )
        if explicit_stop_reason:
            explicit_stop_reasons[explicit_stop_reason] = (
                explicit_stop_reasons.get(explicit_stop_reason, 0) + 1
            )
        if completion_authority_status:
            completion_authority_statuses[completion_authority_status] = (
                completion_authority_statuses.get(completion_authority_status, 0) + 1
            )
        if completion_source_of_truth:
            completion_source_of_truths[completion_source_of_truth] = (
                completion_source_of_truths.get(completion_source_of_truth, 0) + 1
            )
        if evidence_runner_kind:
            evidence_runner_kinds[evidence_runner_kind] = (
                evidence_runner_kinds.get(evidence_runner_kind, 0) + 1
            )
        if evidence_binding_kind:
            evidence_binding_kinds[evidence_binding_kind] = (
                evidence_binding_kinds.get(evidence_binding_kind, 0) + 1
            )
        if freshness_status:
            freshness_statuses[freshness_status] = (
                freshness_statuses.get(freshness_status, 0) + 1
            )
        if artifact_ledger_status:
            artifact_ledger_statuses[artifact_ledger_status] = (
                artifact_ledger_statuses.get(artifact_ledger_status, 0) + 1
            )
        if artifact_ledger_source:
            for source in artifact_ledger_source.split("|"):
                source = source.strip()
                if source:
                    increment = 1
                    if ":" in source:
                        name, maybe_count = source.rsplit(":", 1)
                        if maybe_count.isdigit():
                            source = name
                            increment = int(maybe_count)
                    artifact_ledger_sources[source] = (
                        artifact_ledger_sources.get(source, 0) + increment
                    )
        if workspace_scope_kind:
            workspace_scope_kinds[workspace_scope_kind] = (
                workspace_scope_kinds.get(workspace_scope_kind, 0) + 1
            )
        if deliverable_obligation_kind:
            deliverable_obligation_kinds[deliverable_obligation_kind] = (
                deliverable_obligation_kinds.get(deliverable_obligation_kind, 0) + 1
            )
        if deliverable_obligation_path:
            deliverable_obligation_paths[deliverable_obligation_path] = (
                deliverable_obligation_paths.get(deliverable_obligation_path, 0) + 1
            )
        if artifact_ownership:
            artifact_ownerships[artifact_ownership] = (
                artifact_ownerships.get(artifact_ownership, 0) + 1
            )
        if artifact_source_of_truth:
            artifact_source_of_truths[artifact_source_of_truth] = (
                artifact_source_of_truths.get(artifact_source_of_truth, 0) + 1
            )
        if rejected_target_reason:
            rejected_target_reasons[rejected_target_reason] = (
                rejected_target_reasons.get(rejected_target_reason, 0) + 1
            )
        for field in artifact_ledger_signal_counts:
            if row.get(field) or observation.get(field):
                artifact_ledger_signal_counts[field] += 1
        assertion_status = row.get("expected_assertion_status", "")
        if assertion_status:
            focused_assertion_statuses[assertion_status] = (
                focused_assertion_statuses.get(assertion_status, 0) + 1
            )
        assertion_failures = row.get("expected_assertion_failures", "")
        if assertion_failures:
            focused_assertion_failures.append((row["case_id"], assertion_failures))
        stats = by_case.setdefault(row["case_id"], [0, 0])
        stats[1] += 1
        if row["success"] == "true":
            stats[0] += 1

    lines = [
        "# Eval Report",
        "",
        f"success: {success}/{total}",
        "",
        "## Failure Categories",
    ]
    for name, count in sorted(categories.items()):
        lines.append(f"- {name}: {count}")
    lines.extend(["", "## Terminal States"])
    for name, count in sorted(terminal_states.items()):
        lines.append(f"- {name}: {count}")
    lines.extend(["", "## Contract Layers"])
    for name, count in sorted(layers.items()):
        lines.append(f"- {name}: {count}")
    lines.extend(["", "## Failure Observation Summary"])
    for name, count in sorted(producers.items()):
        lines.append(f"- producer={name}: {count}")
    for name, count in sorted(actionabilities.items()):
        lines.append(f"- actionability={name}: {count}")
    lines.extend(["", "## Producer Coverage"])
    for name, count in sorted(producers.items()):
        lines.append(f"- {name}: {count}")
    for name, count in sorted(guards.items()):
        lines.append(f"- guard={name}: {count}")
    lines.extend(["", "## Unknown/Raw Failure Coverage Defects"])
    if observation_defects:
        for case_id, defect in observation_defects:
            lines.append(f"- {case_id}: {defect}")
    else:
        lines.append("- none")
    lines.extend(["", "## Lifecycle Funnel"])
    for name, count in sorted(terminal_states.items()):
        lines.append(f"- {name}: {count}")
    lines.extend(["", "## Runtime Job Lifecycle"])
    for name, count in sorted(lifecycle_stages.items()):
        lines.append(f"- lifecycle_stage={name}: {count}")
    for name, count in sorted(active_owners.items()):
        lines.append(f"- active_owner={name}: {count}")
    for name, count in sorted(selected_actions.items()):
        lines.append(f"- selected_action={name}: {count}")
    for name, count in sorted(target_admission_statuses.items()):
        lines.append(f"- target_admission_status={name}: {count}")
    for name, count in sorted(repair_action_plan_statuses.items()):
        lines.append(f"- repair_action_plan_status={name}: {count}")
    for name, count in sorted(completion_sources.items()):
        lines.append(f"- completion_source={name}: {count}")
    for name, count in sorted(attempt_outcomes.items()):
        lines.append(f"- attempt_outcome={name}: {count}")
    for name, count in sorted(verifier_rerun_results.items()):
        lines.append(f"- verifier_rerun_result={name}: {count}")
    if explicit_stop_reasons:
        for name, count in sorted(explicit_stop_reasons.items()):
            lines.append(f"- explicit_stop_reason={name}: {count}")
    else:
        lines.append("- explicit_stop_reason=none")
    lines.extend(["", "## Diagnostic Codes"])
    for name, count in sorted(diagnostics.items()):
        lines.append(f"- {name}: {count}")
    lines.extend(["", "## Evidence Authority"])
    for name, count in sorted(completion_authority_statuses.items()):
        lines.append(f"- completion_authority_status={name}: {count}")
    for name, count in sorted(completion_source_of_truths.items()):
        lines.append(f"- completion_source_of_truth={name}: {count}")
    for name, count in sorted(evidence_runner_statuses.items()):
        lines.append(f"- evidence_runner_status={name}: {count}")
    for name, count in sorted(evidence_runner_kinds.items()):
        lines.append(f"- evidence_runner_kind={name}: {count}")
    for name, count in sorted(evidence_binding_kinds.items()):
        lines.append(f"- evidence_binding_kind={name}: {count}")
    for name, count in sorted(freshness_statuses.items()):
        lines.append(f"- freshness_status={name}: {count}")
    for name, count in sorted(artifact_ledger_statuses.items()):
        lines.append(f"- artifact_ledger_status={name}: {count}")
    for name, count in sorted(artifact_ledger_sources.items()):
        lines.append(f"- artifact_ledger_source={name}: {count}")
    for name, count in sorted(workspace_scope_kinds.items()):
        lines.append(f"- workspace_scope_kind={name}: {count}")
    for name, count in sorted(deliverable_obligation_kinds.items()):
        lines.append(f"- deliverable_obligation_kind={name}: {count}")
    for name, count in sorted(deliverable_obligation_paths.items()):
        lines.append(f"- deliverable_obligation_path={name}: {count}")
    lines.extend(["", "## Artifact Ledger Signals"])
    for name, count in sorted(artifact_ledger_signal_counts.items()):
        lines.append(f"- {name}: {count}")
    lines.extend(["", "## Artifact Ownership"])
    for name, count in sorted(artifact_ownerships.items()):
        lines.append(f"- ownership={name}: {count}")
    for name, count in sorted(artifact_source_of_truths.items()):
        lines.append(f"- source_of_truth={name}: {count}")
    lines.extend(["", "## Rejected Targets"])
    if rejected_target_reasons:
        for name, count in sorted(rejected_target_reasons.items()):
            lines.append(f"- {name}: {count}")
    else:
        lines.append("- none")
    lines.extend(["", "## Recovery Jobs"])
    for name, count in sorted(recovery_jobs.items()):
        lines.append(f"- {name}: {count}")
    lines.extend(["", "## Runtime Jobs"])
    for name, count in sorted(runtime_jobs.items()):
        lines.append(f"- {name}: {count}")
    lines.extend(["", "## Dispatch Status"])
    for name, count in sorted(dispatch_statuses.items()):
        lines.append(f"- {name}: {count}")
    lines.extend(["", "## Loop Control Actions"])
    for name, count in sorted(loop_control_actions.items()):
        lines.append(f"- {name}: {count}")
    lines.extend(["", "## Repair Brief Status"])
    for name, count in sorted(repair_brief_statuses.items()):
        lines.append(f"- {name}: {count}")
    lines.extend(["", "## Action Envelope Status"])
    for name, count in sorted(action_envelope_statuses.items()):
        lines.append(f"- {name}: {count}")
    lines.extend(["", "## Repair Action Envelope"])
    for name, count in sorted(allowed_change_kinds.items()):
        lines.append(f"- allowed_change_kind={name}: {count}")
    for name, count in sorted(allowed_tool_categories.items()):
        lines.append(f"- allowed_tool_category={name}: {count}")
    for name, count in sorted(target_confidences.items()):
        lines.append(f"- target_confidence={name}: {count}")
    if repair_plan_rejection_reasons:
        for name, count in sorted(repair_plan_rejection_reasons.items()):
            lines.append(f"- rejection={name}: {count}")
    else:
        lines.append("- rejection=none")
    lines.extend(["", "## Selected Failure Clusters"])
    for name, count in sorted(selected_failure_clusters.items()):
        lines.append(f"- {name}: {count}")
    lines.extend(["", "## Semantic Failure Kinds"])
    for name, count in sorted(semantic_failure_kinds.items()):
        lines.append(f"- {name}: {count}")
    lines.extend(["", "## Diagnostic Failure Kinds"])
    for name, count in sorted(diagnostic_failure_kinds.items()):
        lines.append(f"- {name}: {count}")
    lines.extend(["", "## Semantic Cluster Sources"])
    for name, count in sorted(semantic_cluster_sources.items()):
        lines.append(f"- {name}: {count}")
    lines.extend(["", "## Preferred Repair Roles"])
    for name, count in sorted(preferred_repair_roles.items()):
        lines.append(f"- {name}: {count}")
    lines.extend(["", "## Observed/Expected Pairs"])
    for name, count in sorted(observed_expected_pairs.items()):
        lines.append(f"- {name}: {count}")
    lines.extend(["", "## Affected Cases"])
    for name, count in sorted(affected_cases.items()):
        lines.append(f"- {name}: {count}")
    lines.extend(["", "## Candidate Artifacts"])
    for name, count in sorted(candidate_artifacts.items()):
        lines.append(f"- {name}: {count}")
    lines.extend(["", "## Weak Verifier Reasons"])
    for name, count in sorted(weak_verifier_reasons.items()):
        lines.append(f"- {name}: {count}")
    lines.extend(["", "## Contract Conflict Decisions"])
    if (
        contract_conflict_statuses
        or contract_conflict_authorities
        or contract_conflict_actions
        or contract_conflict_target_sides
        or contract_conflict_safe_stop_reasons
        or contract_conflict_sources
    ):
        for name, count in sorted(contract_conflict_statuses.items()):
            lines.append(f"- status={name}: {count}")
        for name, count in sorted(contract_conflict_authorities.items()):
            lines.append(f"- authority={name}: {count}")
        for name, count in sorted(contract_conflict_target_sides.items()):
            lines.append(f"- repair_target_side={name}: {count}")
        for name, count in sorted(contract_conflict_actions.items()):
            lines.append(f"- selected_action={name}: {count}")
        for name, count in sorted(contract_conflict_safe_stop_reasons.items()):
            lines.append(f"- safe_stop_reason={name}: {count}")
        for name, count in sorted(contract_conflict_sources.items()):
            lines.append(f"- source_of_truth={name}: {count}")
    else:
        lines.append("- none")
    lines.extend(["", "## Admitted Cluster Targets"])
    for name, count in sorted(admitted_cluster_targets.items()):
        lines.append(f"- {name}: {count}")
    lines.extend(["", "## Unknown Diagnostic Count"])
    lines.append(f"- total: {unknown_diagnostic_total}")
    lines.extend(["", "## Tool Protocol Recovery"])
    for name, count in sorted(tool_protocol_statuses.items()):
        lines.append(f"- status={name}: {count}")
    for name, count in sorted(tool_protocol_sources.items()):
        lines.append(f"- source={name}: {count}")
    for name, count in sorted(tool_protocol_actions.items()):
        lines.append(f"- action={name}: {count}")
    for name, count in sorted(tool_protocol_failed_tools.items()):
        lines.append(f"- failed_tool={name}: {count}")
    for name, count in sorted(tool_protocol_missing_fields.items()):
        lines.append(f"- missing_field={name}: {count}")
    for name, count in sorted(tool_protocol_required_fields.items()):
        lines.append(f"- required_fields={name}: {count}")
    for name, count in sorted(tool_protocol_correction_spent.items()):
        lines.append(f"- correction_spent={name}: {count}")
    for name, count in sorted(tool_protocol_correction_exhausted.items()):
        lines.append(f"- correction_exhausted={name}: {count}")
    lines.extend(["", "## Patch Validation"])
    for name, count in sorted(patch_validation_statuses.items()):
        lines.append(f"- status={name}: {count}")
    for name, count in sorted(patch_validation_sources.items()):
        lines.append(f"- source={name}: {count}")
    for name, count in sorted(patch_validation_outcomes.items()):
        lines.append(f"- outcomes={name}: {count}")
    for name, count in sorted(patch_validation_rejected_paths.items()):
        lines.append(f"- rejected_paths={name}: {count}")
    lines.extend(["", "## Mechanical Repair Adapters"])
    if mechanical_adapters or mechanical_adapter_statuses or mechanical_adapter_actions:
        for name, count in sorted(mechanical_adapters.items()):
            lines.append(f"- adapter={name}: {count}")
        for name, count in sorted(mechanical_adapter_statuses.items()):
            lines.append(f"- status={name}: {count}")
        for name, count in sorted(mechanical_adapter_actions.items()):
            lines.append(f"- action={name}: {count}")
    else:
        lines.append("- none")
    lines.extend(["", "## Profile Parity"])
    if (
        profile_project_kinds
        or profile_manifest_artifacts
        or profile_entrypoints
        or profile_integration_artifacts
        or profile_completion_evidence
        or profile_failure_mappings
        or profile_adapter_families
        or profile_capability_statuses
    ):
        for name, count in sorted(profile_project_kinds.items()):
            lines.append(f"- project_kind={name}: {count}")
        for name, count in sorted(profile_manifest_artifacts.items()):
            lines.append(f"- manifest_artifacts={name}: {count}")
        for name, count in sorted(profile_entrypoints.items()):
            lines.append(f"- entrypoints={name}: {count}")
        for name, count in sorted(profile_integration_artifacts.items()):
            lines.append(f"- integration_artifacts={name}: {count}")
        for name, count in sorted(profile_completion_evidence.items()):
            lines.append(f"- completion_evidence={name}: {count}")
        for name, count in sorted(profile_failure_mappings.items()):
            lines.append(f"- failure_mapping={name}: {count}")
        for name, count in sorted(profile_adapter_families.items()):
            lines.append(f"- adapter_families={name}: {count}")
        for name, count in sorted(profile_capability_statuses.items()):
            lines.append(f"- capability_status={name}: {count}")
    else:
        lines.append("- none")
    lines.extend(["", "## Phase29 Runtime Support"])
    if (
        phase29_support_rows
        or language_repair_adapter_statuses
        or effective_tool_policies
        or effective_tool_policy_statuses
        or tool_failure_recovery_statuses
        or setup_command_classifications
        or command_authorities
        or command_classification_reasons
        or workspace_candidate_statuses
        or workspace_ignored_dir_policies
        or workspace_candidate_ignored_reasons
        or job_report_statuses
        or job_report_owner_actions
        or scaffold_contract_statuses
        or noncoding_evidence_statuses
        or answer_work_mode_statuses
        or lifecycle_projection_statuses
        or provider_boundary_statuses
    ):
        for name, count in sorted(phase29_support_rows.items()):
            lines.append(f"- support_rows={name}: {count}")
        for name, count in sorted(language_repair_adapter_statuses.items()):
            lines.append(f"- language_repair_adapter_status={name}: {count}")
        for name, count in sorted(effective_tool_policies.items()):
            lines.append(f"- effective_tool_policy={name}: {count}")
        for name, count in sorted(effective_tool_policy_statuses.items()):
            lines.append(f"- effective_tool_policy_status={name}: {count}")
        for name, count in sorted(tool_failure_recovery_statuses.items()):
            lines.append(f"- tool_failure_recovery_status={name}: {count}")
        for name, count in sorted(setup_command_classifications.items()):
            lines.append(f"- setup_command_classification={name}: {count}")
        for name, count in sorted(command_authorities.items()):
            lines.append(f"- command_authority={name}: {count}")
        for name, count in sorted(command_classification_reasons.items()):
            lines.append(f"- command_classification_reason={name}: {count}")
        for name, count in sorted(workspace_candidate_statuses.items()):
            lines.append(f"- workspace_candidate_status={name}: {count}")
        for name, count in sorted(workspace_ignored_dir_policies.items()):
            lines.append(f"- workspace_ignored_dir_policy={name}: {count}")
        for name, count in sorted(workspace_candidate_ignored_reasons.items()):
            lines.append(f"- workspace_candidate_ignored_reasons={name}: {count}")
        for name, count in sorted(job_report_statuses.items()):
            lines.append(f"- job_report_status={name}: {count}")
        for name, count in sorted(job_report_owner_actions.items()):
            lines.append(f"- job_report_owner_action={name}: {count}")
        for name, count in sorted(scaffold_contract_statuses.items()):
            lines.append(f"- scaffold_contract_status={name}: {count}")
        for name, count in sorted(noncoding_evidence_statuses.items()):
            lines.append(f"- noncoding_evidence_status={name}: {count}")
        for name, count in sorted(answer_work_mode_statuses.items()):
            lines.append(f"- answer_work_mode_status={name}: {count}")
        for name, count in sorted(lifecycle_projection_statuses.items()):
            lines.append(f"- lifecycle_projection_status={name}: {count}")
        for name, count in sorted(provider_boundary_statuses.items()):
            lines.append(f"- provider_boundary_status={name}: {count}")
    else:
        lines.append("- none")
    lines.extend(["", "## Rollback Admission"])
    if rollback_admission_statuses or rollback_reasons:
        for name, count in sorted(rollback_admission_statuses.items()):
            lines.append(f"- status={name}: {count}")
        for name, count in sorted(rollback_reasons.items()):
            lines.append(f"- reason={name}: {count}")
    else:
        lines.append("- none")
    lines.extend(["", "## Task Contract"])
    for name, count in sorted(task_contract_kinds.items()):
        lines.append(f"- kind={name}: {count}")
    for name, count in sorted(task_contract_statuses.items()):
        lines.append(f"- status={name}: {count}")
    for name, count in sorted(task_contract_lifecycles.items()):
        lines.append(f"- lifecycle={name}: {count}")
    for name, count in sorted(task_contract_request_signals.items()):
        lines.append(f"- request_signals={name}: {count}")
    for name, count in sorted(task_contract_constraints.items()):
        lines.append(f"- constraints={name}: {count}")
    for name, count in sorted(task_contract_completion_evidence.items()):
        lines.append(f"- completion_evidence={name}: {count}")
    lines.extend(["", "## Behavior Obligations"])
    for name, count in sorted(behavior_obligation_statuses.items()):
        lines.append(f"- status={name}: {count}")
    for name, count in sorted(behavior_obligation_owners.items()):
        lines.append(f"- owners={name}: {count}")
    for name, count in sorted(behavior_obligation_paths.items()):
        lines.append(f"- paths={name}: {count}")
    lines.extend(["", "## Artifact Role Projection"])
    for name, count in sorted(artifact_role_projection_statuses.items()):
        lines.append(f"- status={name}: {count}")
    lines.extend(["", "## Focused Matrix"])
    for name, count in sorted(proof_modes.items()):
        lines.append(f"- proof_mode={name}: {count}")
    for name, count in sorted(matrix_rows.items()):
        lines.append(f"- matrix_row={name}: {count}")
    lines.extend(["", "## Focused Assertions"])
    if focused_assertion_statuses:
        for name, count in sorted(focused_assertion_statuses.items()):
            lines.append(f"- {name}: {count}")
    else:
        lines.append("- not_configured: 0")
    if focused_assertion_failures:
        lines.extend(["", "## Focused Assertion Failures"])
        for case_id, failures in focused_assertion_failures:
            lines.append(f"- {case_id}: {failures}")
    lines.extend(["", "## By Case"])
    for case_id, (case_success, case_total) in sorted(by_case.items()):
        lines.append(f"- {case_id}: {case_success}/{case_total}")
    return "\n".join(lines) + "\n"


def observation_defect(row, observation, terminal_state, diagnostic_code):
    reason = row.get("reason", "")
    if terminal_state == "explicit_stop" and (
        row.get("explicit_stop_reason") or observation.get("explicit_stop_reason")
    ):
        return ""
    if terminal_state == "unknown":
        return "terminal_state=unknown"
    if (row.get("contract_layer") or observation.get("contract_layer")) == "unknown_contract":
        return "contract_layer=unknown_contract"
    if not diagnostic_code or diagnostic_code == "unknown":
        return "diagnostic_code=unknown"
    if reason.startswith("rc:") and diagnostic_code.startswith("rc_"):
        return f"raw_reason={reason} diagnostic_code={diagnostic_code}"
    return ""


def main():
    args = parse_args()
    root = Path(args.root)
    cases = read_cases(args.cases_dir)
    if args.recheck:
        rows, out = recheck(root, cases)
        print(f"wrote {out}")
    else:
        rows = read_summary(root / "summary.tsv")
    print(render_report(rows))


if __name__ == "__main__":
    main()
