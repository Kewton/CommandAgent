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
    contract_layer_for_reason,
    normalize_observation,
)
from eval_case_schema import (  # noqa: E402
    ASSERTION_FIELD_NAMES,
    focused_assertions,
    iter_case_paths,
    read_eval_case,
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
    }


def unquote(value):
    if len(value) >= 2 and value[0] == value[-1] and value[0] in {'"', "'"}:
        return value[1:-1]
    return value


def read_summary(path):
    with open(path, encoding="utf-8", newline="") as handle:
        return list(csv.DictReader(handle, delimiter="\t"))


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
        *OBSERVATION_FIELD_NAMES,
        "active_job",
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
        "selected_failure_cluster",
        "semantic_failure_kind",
        "preferred_repair_role",
        "weak_verifier_reason",
        "admitted_cluster_targets",
        "task_contract_kind",
        "task_contract_status",
        "behavior_obligation_codes",
        "behavior_obligation_status",
        "artifact_role_projection_status",
        "repair_brief_status",
        "action_envelope_status",
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
                "active_job": meta.get("active_job", derive_active_job(reason)),
                "recovery_owner": meta.get("recovery_owner", derive_recovery_owner(reason)),
                "loop_control_action": meta.get("loop_control_action", derive_loop_control_action(reason)),
                "dispatch_status": meta.get("dispatch_status", derive_dispatch_status(reason)),
                "dispatch_reason": meta.get("dispatch_reason", ""),
                "candidate_jobs": meta.get("candidate_jobs", ""),
                "tie_break_reason": meta.get("tie_break_reason", ""),
                "target_path": meta.get("target_path", first_reason_target(reason)),
                "target_role": meta.get("target_role", artifact_role_for_path(first_reason_target(reason))),
                "target_candidate_count": meta.get("target_candidate_count", ""),
                "target_admitted_count": meta.get("target_admitted_count", ""),
                "target_rejected_count": meta.get("target_rejected_count", ""),
                "selected_target": meta.get("selected_target", meta.get("target_path", first_reason_target(reason))),
                "selected_target_role": meta.get("selected_target_role", meta.get("target_role", artifact_role_for_path(first_reason_target(reason)))),
                "target_rejection_reasons": meta.get("target_rejection_reasons", ""),
                "selected_failure_cluster": meta.get("selected_failure_cluster", ""),
                "semantic_failure_kind": meta.get("semantic_failure_kind", ""),
                "preferred_repair_role": meta.get("preferred_repair_role", ""),
                "weak_verifier_reason": meta.get("weak_verifier_reason", ""),
                "admitted_cluster_targets": meta.get("admitted_cluster_targets", ""),
                "task_contract_kind": meta.get("task_contract_kind", ""),
                "task_contract_status": meta.get("task_contract_status", ""),
                "behavior_obligation_codes": meta.get("behavior_obligation_codes", ""),
                "behavior_obligation_status": meta.get("behavior_obligation_status", ""),
                "artifact_role_projection_status": meta.get("artifact_role_projection_status", ""),
                "repair_brief_status": meta.get("repair_brief_status", ""),
                "action_envelope_status": meta.get("action_envelope_status", ""),
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
            }
        )
        observation = normalize_observation(
            {
                **meta,
                **rows[-1],
                "reason": reason,
                "success": success,
                "rc": rc,
            }
        )
        rows[-1].update(
            {name: observation.get(name, "") for name in OBSERVATION_FIELD_NAMES}
        )
        rows[-1]["failure_category"] = observation["failure_category"]
        rows[-1]["contract_layer"] = observation["contract_layer"]
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
    if category == "profile" and ("route" in reason or "integration" in reason):
        return "route_integration_repair"
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


def derive_repair_action(reason):
    job = derive_active_job(reason)
    if job == "setup_bootstrap":
        return "install_or_prepare_dependencies"
    if job == "dev_server_smoke":
        return "run_dev_server_smoke"
    if job == "manifest_repair":
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
    selected_failure_clusters = {}
    semantic_failure_kinds = {}
    preferred_repair_roles = {}
    weak_verifier_reasons = {}
    admitted_cluster_targets = {}
    task_contract_kinds = {}
    task_contract_statuses = {}
    behavior_obligation_statuses = {}
    artifact_role_projection_statuses = {}
    completion_authority_statuses = {}
    completion_source_of_truths = {}
    evidence_runner_statuses = {}
    evidence_runner_kinds = {}
    evidence_binding_kinds = {}
    freshness_statuses = {}
    artifact_ledger_statuses = {}
    workspace_scope_kinds = {}
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
    for row in rows:
        observation = normalize_observation(row)
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
        workspace_scope_kind = (
            row.get("workspace_scope_kind") or observation.get("workspace_scope_kind", "")
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
        selected_failure_cluster = row.get("selected_failure_cluster", "")
        semantic_failure_kind = row.get("semantic_failure_kind", "")
        preferred_repair_role = row.get("preferred_repair_role", "")
        weak_verifier_reason = row.get("weak_verifier_reason", "")
        admitted_targets = row.get("admitted_cluster_targets", "")
        task_contract_kind = row.get("task_contract_kind", "")
        task_contract_status = row.get("task_contract_status", "")
        behavior_obligation_status = row.get("behavior_obligation_status", "")
        artifact_role_projection_status = row.get("artifact_role_projection_status", "")
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
        if selected_failure_cluster:
            selected_failure_clusters[selected_failure_cluster] = (
                selected_failure_clusters.get(selected_failure_cluster, 0) + 1
            )
        if semantic_failure_kind:
            semantic_failure_kinds[semantic_failure_kind] = (
                semantic_failure_kinds.get(semantic_failure_kind, 0) + 1
            )
        if preferred_repair_role:
            preferred_repair_roles[preferred_repair_role] = (
                preferred_repair_roles.get(preferred_repair_role, 0) + 1
            )
        if weak_verifier_reason:
            weak_verifier_reasons[weak_verifier_reason] = (
                weak_verifier_reasons.get(weak_verifier_reason, 0) + 1
            )
        if admitted_targets:
            admitted_cluster_targets[admitted_targets] = (
                admitted_cluster_targets.get(admitted_targets, 0) + 1
            )
        if task_contract_kind:
            task_contract_kinds[task_contract_kind] = (
                task_contract_kinds.get(task_contract_kind, 0) + 1
            )
        if task_contract_status:
            task_contract_statuses[task_contract_status] = (
                task_contract_statuses.get(task_contract_status, 0) + 1
            )
        if behavior_obligation_status:
            behavior_obligation_statuses[behavior_obligation_status] = (
                behavior_obligation_statuses.get(behavior_obligation_status, 0) + 1
            )
        if artifact_role_projection_status:
            artifact_role_projection_statuses[artifact_role_projection_status] = (
                artifact_role_projection_statuses.get(artifact_role_projection_status, 0) + 1
            )
        if evidence_runner_status:
            evidence_runner_statuses[evidence_runner_status] = (
                evidence_runner_statuses.get(evidence_runner_status, 0) + 1
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
        if workspace_scope_kind:
            workspace_scope_kinds[workspace_scope_kind] = (
                workspace_scope_kinds.get(workspace_scope_kind, 0) + 1
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
    for name, count in sorted(workspace_scope_kinds.items()):
        lines.append(f"- workspace_scope_kind={name}: {count}")
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
    lines.extend(["", "## Selected Failure Clusters"])
    for name, count in sorted(selected_failure_clusters.items()):
        lines.append(f"- {name}: {count}")
    lines.extend(["", "## Semantic Failure Kinds"])
    for name, count in sorted(semantic_failure_kinds.items()):
        lines.append(f"- {name}: {count}")
    lines.extend(["", "## Preferred Repair Roles"])
    for name, count in sorted(preferred_repair_roles.items()):
        lines.append(f"- {name}: {count}")
    lines.extend(["", "## Weak Verifier Reasons"])
    for name, count in sorted(weak_verifier_reasons.items()):
        lines.append(f"- {name}: {count}")
    lines.extend(["", "## Admitted Cluster Targets"])
    for name, count in sorted(admitted_cluster_targets.items()):
        lines.append(f"- {name}: {count}")
    lines.extend(["", "## Task Contract"])
    for name, count in sorted(task_contract_kinds.items()):
        lines.append(f"- kind={name}: {count}")
    for name, count in sorted(task_contract_statuses.items()):
        lines.append(f"- status={name}: {count}")
    lines.extend(["", "## Behavior Obligations"])
    for name, count in sorted(behavior_obligation_statuses.items()):
        lines.append(f"- status={name}: {count}")
    lines.extend(["", "## Artifact Role Projection"])
    for name, count in sorted(artifact_role_projection_statuses.items()):
        lines.append(f"- status={name}: {count}")
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
