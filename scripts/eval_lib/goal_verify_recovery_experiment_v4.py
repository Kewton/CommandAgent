from __future__ import annotations

from collections.abc import Callable
from pathlib import Path
from typing import Any

from eval_lib.goal_verify_executors_v3 import execute_registered

OracleExecutor = Callable[..., dict[str, Any]]

POLICY_ID = "commandagent.goal_verify.recovery_eligibility.v4_a14"

RECOVERY_FIX_TERMINAL_OUTCOME_POLICY = {
    "allowed_outcomes": [
        "promoted_success",
        "honest_not_recoverable_control_retained",
    ],
    "honest_not_recoverable_control_retained_requires": [
        "failed product terminal with a nonzero return code",
        "recorded failed completion observation",
        "one failed Recovery attempt ending as not_recoverable",
        "one rejected promotion decision with recovery_execution_failed",
        "exactly one retained control and zero restore failures",
        "unchanged external failure with passing frozen regressions",
        "zero regression introduction and zero existing-artifact harm",
    ],
}

SMOKE_PROFILE_PATH_COVERAGE_POLICY = {
    "allowed_paths": [
        "executed_recovery",
        "all_initial_oracle_pass_with_current_success_protection",
    ],
    "effect_limitation": (
        "current-success-only coverage validates safety but does not establish "
        "profile-specific Recovery repair effect"
    ),
}

_RUNTIME_EXCLUSIONS = {
    "dependency_or_provisioning": (
        "dependency_setup",
        "foreign_toolchain",
        "offline_dependency",
        "package_cache",
    ),
    "capability_unavailable": (
        "browser_capability",
        "capability_unavailable",
        "executor_capability",
        "safe_execution_unavailable",
    ),
    "profile_or_completion_contract": (
        "profile_contract",
        "unsupported_completion_obligation",
    ),
    "sandbox_or_policy": (
        "command_policy",
        "policy_rejected",
        "sandbox_policy",
    ),
    "task_information_missing": (
        "insufficient_task_information",
        "missing_task_information",
    ),
}

_CAPABILITY_BLOCKER_PREFIXES = (
    "browser_interaction_unavailable",
    "browser_readiness_unavailable",
    "missing_required_capabilities",
    "unsupported_required_capability",
)


def recovery_contract_errors(contract: dict[str, Any]) -> list[str]:
    errors = []
    paired = contract.get("paired_run_contract", {})
    if paired.get("initial_only", {}).get("recovery_plan_auto_runs") != 0:
        errors.append("initial_only_recovery_runs_must_be_zero")
    if paired.get("recovery_one", {}).get("recovery_plan_auto_runs") != 1:
        errors.append("recovery_one_runs_must_be_one")
    if paired.get("maximum_recovery_runs") != 1:
        errors.append("maximum_recovery_runs_must_be_one")
    if paired.get("arm_order") != ["initial_only", "recovery_one"]:
        errors.append("paired_arm_order_invalid")
    execution_action = paired.get("execution_action", "plan_run")
    if execution_action not in {"plan_run", "ultra_plan_run"}:
        errors.append("paired_execution_action_invalid")
    eligibility = contract.get("recovery_eligibility", {})
    if eligibility.get("free_form_stderr_used_for_classification") is not False:
        errors.append("free_form_stderr_classification_must_be_false")
    if eligibility.get("structured_terminal_event_required") is not True:
        errors.append("structured_terminal_event_must_be_required")
    oracle = contract.get("external_oracle_policy", {})
    if oracle.get("passed_to_product_or_recovery") is not False:
        errors.append("external_oracle_must_not_be_passed_to_product")
    if oracle.get("self_report_used_for_success") is not False:
        errors.append("self_report_success_must_be_false")
    analysis = contract.get("analysis", {})
    if analysis.get("attribution_requires_executed_recovery_runs") != 1:
        errors.append("recovery_attribution_must_require_one_executed_run")
    smoke = contract.get("smoke", {})
    terminal_policy = smoke.get("recovery_fix_terminal_outcome_policy")
    if (
        terminal_policy is not None
        and terminal_policy != RECOVERY_FIX_TERMINAL_OUTCOME_POLICY
    ):
        errors.append("recovery_fix_terminal_outcome_policy_invalid")
    profile_path_policy = smoke.get("real_profile_path_coverage_policy")
    if (
        profile_path_policy is not None
        and profile_path_policy != SMOKE_PROFILE_PATH_COVERAGE_POLICY
    ):
        errors.append("smoke_profile_path_coverage_policy_invalid")
    pair_ids = smoke.get("selected_pair_ids")
    if not isinstance(pair_ids, list) or not pair_ids:
        errors.append("smoke_pair_ids_missing")
    elif len(pair_ids) != len(set(pair_ids)):
        errors.append("smoke_pair_ids_duplicate")
    elif smoke.get("expected_pair_count") != len(pair_ids):
        errors.append("smoke_pair_count_mismatch")
    if smoke.get("effect_claim_allowed") is not False:
        errors.append("smoke_effect_claim_must_be_false")
    minimum_executed = smoke.get("minimum_executed_recovery_pairs", 0)
    if (
        not isinstance(minimum_executed, int)
        or isinstance(minimum_executed, bool)
        or minimum_executed < 0
    ):
        errors.append("smoke_minimum_executed_recovery_pairs_invalid")
    elif (
        isinstance(smoke.get("expected_pair_count"), int)
        and minimum_executed > smoke["expected_pair_count"]
    ):
        errors.append("smoke_minimum_executed_recovery_pairs_exceeds_total")
    if (
        smoke.get("require_recovery_capable_execution_action") is True
        and execution_action != "ultra_plan_run"
    ):
        errors.append("smoke_execution_action_not_recovery_capable")
    full = contract.get("full_experiment")
    if full is not None:
        if not isinstance(full, dict):
            errors.append("full_experiment_invalid")
        else:
            eligible_pairs = full.get("eligible_pair_ids")
            sentinel_pairs = full.get("sentinel_pair_ids")
            frozen_pairs = (
                [*eligible_pairs, *sentinel_pairs]
                if isinstance(eligible_pairs, list) and isinstance(sentinel_pairs, list)
                else None
            )
            if (
                not eligible_pairs
                or not sentinel_pairs
                or frozen_pairs != pair_ids
                or len(frozen_pairs) != len(set(frozen_pairs))
            ):
                errors.append("full_experiment_pair_population_invalid")
            if full.get("effect_claim_allowed") is not True:
                errors.append("full_experiment_effect_claim_not_authorized")
            if full.get("bootstrap_samples") != 2000:
                errors.append("full_experiment_bootstrap_samples_invalid")
            if full.get("confidence_interval") != 0.95:
                errors.append("full_experiment_confidence_interval_invalid")
            if not isinstance(full.get("bootstrap_seed"), int) or isinstance(
                full.get("bootstrap_seed"), bool
            ):
                errors.append("full_experiment_bootstrap_seed_invalid")
            if full.get("pairs_per_eligible_cluster") != 3:
                errors.append("full_experiment_cluster_repeats_invalid")
            if full.get("minimum_clusters_per_cell") != 10:
                errors.append("full_experiment_clusters_per_cell_invalid")
            if full.get("minimum_executed_recovery_pairs") != minimum_executed:
                errors.append("full_experiment_minimum_executed_mismatch")
            if isinstance(eligible_pairs, list) and full.get(
                "eligible_pair_count"
            ) != len(eligible_pairs):
                errors.append("full_experiment_eligible_pair_count_invalid")
            if isinstance(sentinel_pairs, list) and full.get(
                "sentinel_pair_count"
            ) != len(sentinel_pairs):
                errors.append("full_experiment_sentinel_pair_count_invalid")
            budgets = full.get("resource_budgets")
            if not _valid_full_resource_budgets(budgets):
                errors.append("full_experiment_resource_budgets_invalid")
    integrity = contract.get("integrity", {})
    if integrity.get("exclusive_run_lock") != ".campaign.lock":
        errors.append("exclusive_run_lock_invalid")
    if integrity.get("record_ledger") != "record-ledger.jsonl":
        errors.append("record_ledger_invalid")
    if integrity.get("append_only_records") is not True:
        errors.append("append_only_records_must_be_true")
    if paired.get("pairing_unit") == "shared_pre_recovery_snapshot":
        frozen = contract.get("frozen_input_sha256")
        if not isinstance(frozen, dict) or len(frozen) < 6:
            errors.append("shared_boundary_frozen_input_hashes_missing")
        elif any(
            not isinstance(path, str)
            or not path
            or not isinstance(digest, str)
            or len(digest) != 64
            for path, digest in frozen.items()
        ):
            errors.append("shared_boundary_frozen_input_hash_invalid")
    return errors


def _valid_full_resource_budgets(value: Any) -> bool:
    if not isinstance(value, dict) or set(value) != {
        "wall_time_ms",
        "total_tokens",
    }:
        return False
    return all(
        isinstance(value[field], dict)
        and set(value[field]) == {"p50", "p95"}
        and all(
            isinstance(value[field][percentile], int)
            and not isinstance(value[field][percentile], bool)
            and value[field][percentile] > 0
            for percentile in ("p50", "p95")
        )
        and value[field]["p50"] <= value[field]["p95"]
        for field in ("wall_time_ms", "total_tokens")
    )


def classify_case_recovery_eligibility(
    *, task_contract: dict[str, Any], adapters: list[dict[str, Any]]
) -> dict[str, Any]:
    """Freeze whether a case has the inputs and external oracle needed by A14."""
    case_id = task_contract.get("case_id")
    constraints = task_contract.get("operational_constraints", {})
    profile = task_contract.get("completion_contract", {}).get("profile")
    forbidden_conversions = constraints.get("do_not_convert_to", [])
    normalized_profile = _normalized_contract_token(profile)
    if normalized_profile and any(
        normalized_profile in _normalized_contract_token(target)
        for target in forbidden_conversions
        if isinstance(target, str)
    ):
        return _classification(
            eligible=False,
            category="profile_or_completion_contract",
            reason="product_profile_explicitly_forbidden_by_task_contract",
        )
    if constraints.get("unavailable_dependencies"):
        return _classification(
            eligible=False,
            category="dependency_or_provisioning",
            reason="task_contract_declares_unavailable_dependencies",
        )
    case_adapters = [row for row in adapters if row.get("case_id") == case_id]
    if not case_adapters:
        return _classification(
            eligible=False,
            category="capability_unavailable",
            reason="frozen_external_oracle_missing",
        )
    if any(
        row.get("executor", {}).get("kind") == "unavailable" for row in case_adapters
    ):
        return _classification(
            eligible=False,
            category="capability_unavailable",
            reason="frozen_external_oracle_unavailable",
        )
    return _classification(
        eligible=True,
        category="recoverable_candidate",
        reason="task_inputs_and_frozen_external_oracles_available",
    )


def _normalized_contract_token(value: Any) -> str:
    return "".join(character for character in str(value).lower() if character.isalnum())


def classify_initial_recovery_eligibility(
    *, preregistered: dict[str, Any], baseline: dict[str, Any]
) -> dict[str, Any]:
    """Apply the frozen runtime boundary without interpreting free-form stderr."""
    if preregistered.get("eligible") is not True:
        return {
            **preregistered,
            "run_recovery_one_arm": False,
            "runtime_source": "preregistered_case_policy",
        }
    terminal = baseline.get("terminal_status", {})
    if terminal.get("recorded") is not True:
        return _runtime_classification(
            run=False,
            category="instrumentation_unavailable",
            reason="structured_terminal_status_missing",
        )
    if terminal.get("ok") is True:
        return _runtime_classification(
            run=True,
            category="initial_success",
            reason="paired_recovery_one_arm_runs_but_should_execute_zero_recoveries",
        )
    blockers = terminal.get("structured_blockers", [])
    if isinstance(blockers, list) and any(
        isinstance(blocker, str) and blocker.startswith(_CAPABILITY_BLOCKER_PREFIXES)
        for blocker in blockers
    ):
        return _runtime_classification(
            run=False,
            category="capability_unavailable",
            reason="structured_capability_blocker",
        )
    failure_kind = terminal.get("recovery_failure_kind") or terminal.get("failure_kind")
    if not isinstance(failure_kind, str) or not failure_kind:
        return _runtime_classification(
            run=False,
            category="instrumentation_unavailable",
            reason="structured_failure_kind_missing",
        )
    for category, prefixes in _RUNTIME_EXCLUSIONS.items():
        if failure_kind.startswith(prefixes):
            return _runtime_classification(
                run=False,
                category=category,
                reason=f"excluded_failure_kind:{failure_kind}",
            )
    recovery_path = terminal.get("recovery_ultra_plan_path")
    if not isinstance(recovery_path, str) or not recovery_path:
        return _runtime_classification(
            run=False,
            category="recovery_candidate_unavailable",
            reason="recovery_ultra_plan_path_missing",
        )
    return _runtime_classification(
        run=True,
        category="recoverable_candidate",
        reason=f"structured_recovery_candidate:{failure_kind}",
    )


def execute_frozen_external_oracles(
    *,
    case_id: str,
    adapters: list[dict[str, Any]],
    workspace: Path,
    a14_role: str | None = None,
    executor: OracleExecutor = execute_registered,
) -> dict[str, Any]:
    """Run registered host-owned oracles after product execution, never as input."""
    outcomes = []
    for adapter in adapters:
        if adapter.get("case_id") != case_id:
            continue
        if a14_role is not None and adapter.get("a14_role") != a14_role:
            continue
        outcome = _normalize_product_oracle_outcome(
            executor(adapter["executor"], workspace=workspace)
        )
        outcomes.append(
            {
                "adapter_id": adapter.get("adapter_id"),
                "claim_id": adapter.get("claim_id"),
                "executor_kind": adapter.get("executor", {}).get("kind"),
                "is_regression": (
                    adapter.get("claim_id") == "regressions"
                    or adapter.get("executor", {}).get("kind") == "regression_set"
                ),
                "outcome": outcome,
            }
        )
    summary = summarize_frozen_external_oracles(outcomes)
    if a14_role is not None:
        summary["a14_role"] = a14_role
    return summary


def validate_a14_oracle_semantics(
    *, case_id: str, intent: str, adapters: list[dict[str, Any]]
) -> dict[str, Any]:
    rows = [row for row in adapters if row.get("case_id") == case_id]
    final_rows = [row for row in rows if row.get("a14_role") == "final_success"]
    precondition_rows = [row for row in rows if row.get("a14_role") == "precondition"]
    errors = []
    if not final_rows:
        errors.append("final_success_oracle_missing")
    if any(
        row.get("a14_role") not in {"final_success", "precondition"} for row in rows
    ):
        errors.append("oracle_role_unregistered")
    for row in final_rows:
        executor = row.get("executor", {})
        if (
            executor.get("kind") == "fixture_hash_command"
            and executor.get("observation", {}).get("expected") != 0
        ):
            errors.append(f"final_fixture_not_success:{row.get('adapter_id')}")
        if executor.get("kind") == "file_content" and executor.get("polarity") not in {
            "present",
            "absent",
        }:
            errors.append(f"typed_polarity_invalid:{row.get('adapter_id')}")
    if intent == "fix":
        if not precondition_rows:
            errors.append("fix_precondition_oracle_missing")
        command_kinds = {"fixture_hash_command", "sandbox_command", "stage_command"}
        before_commands = [
            row["executor"]
            for row in precondition_rows
            if row.get("executor", {}).get("kind") in command_kinds
        ]
        after_commands = [
            row["executor"]
            for row in final_rows
            if row.get("executor", {}).get("kind") in command_kinds
        ]
        matched = [
            (before, after)
            for before in before_commands
            for after in after_commands
            if before.get("kind") == after.get("kind")
            and before.get("argv") == after.get("argv")
            and (
                before.get("kind") != "fixture_hash_command"
                or before.get("registered_fixture") == after.get("registered_fixture")
            )
        ]
        if not matched:
            errors.append("fix_before_after_command_pair_missing")
        elif not any(
            before.get("observation", {}).get("expected") not in {None, 0}
            and after.get("observation", {}).get("expected") == 0
            for before, after in matched
        ):
            errors.append("fix_before_after_polarity_not_distinct")
    return {
        "valid": not errors,
        "errors": errors,
        "final_success_adapter_ids": [row.get("adapter_id") for row in final_rows],
        "precondition_adapter_ids": [
            row.get("adapter_id") for row in precondition_rows
        ],
    }


def compare_shared_recovery_boundary(
    *,
    treatment: dict[str, Any],
    control_oracles: dict[str, Any],
    treatment_oracles: dict[str, Any],
    control_artifact_manifest: dict[str, Any],
    treatment_artifact_manifest: dict[str, Any],
    control_snapshot_matches_boundary: bool,
    oracle_semantics: dict[str, Any],
    precondition_oracles: dict[str, Any] | None,
) -> dict[str, Any]:
    attempts = treatment.get("recovery_plan_attempts", {})
    executed = attempts.get("executed_recovery_runs")
    initial_quality = control_oracles.get("overall")
    recovery_quality = treatment_oracles.get("overall")
    if initial_quality == "fail" and recovery_quality == "pass":
        raw_transition = "improved"
    elif initial_quality == "pass" and recovery_quality != "pass":
        raw_transition = "harmed"
    elif initial_quality == recovery_quality == "pass":
        raw_transition = "unchanged_pass"
    elif initial_quality == recovery_quality == "fail":
        raw_transition = "unchanged_fail"
    else:
        raw_transition = "unusable"
    precondition_valid = (
        precondition_oracles is None or precondition_oracles.get("overall") == "pass"
    )
    attribution_ready = (
        executed == 1
        and control_snapshot_matches_boundary
        and oracle_semantics.get("valid") is True
        and precondition_valid
        and raw_transition != "unusable"
    )
    transition = raw_transition if attribution_ready else "unattributed"
    boundary = treatment.get("recovery_boundary", {})
    changed = artifact_delta(control_artifact_manifest, treatment_artifact_manifest)
    return {
        "quality_transition": transition,
        "raw_oracle_transition": raw_transition,
        "effect_attribution_ready": attribution_ready,
        "success_improved": attribution_ready and raw_transition == "improved",
        "existing_artifact_harmed": attribution_ready and raw_transition == "harmed",
        "regression_introduced": (
            attribution_ready
            and control_oracles.get("regression_status") == "pass"
            and treatment_oracles.get("regression_status") == "fail"
        ),
        "executed_recovery_runs": executed,
        "shared_initial_history": True,
        "control_snapshot_matches_boundary": control_snapshot_matches_boundary,
        "oracle_semantics": oracle_semantics,
        "precondition_oracle_status": (
            precondition_oracles.get("overall")
            if precondition_oracles is not None
            else "not_applicable"
        ),
        "resource_delta": boundary.get("recovery_provider_usage", {}),
        "recovery_changed_paths": {
            "added": changed["added"],
            "removed": changed["removed"],
            "changed": changed["changed"],
            "change_count": changed["change_count"],
        },
        "artifact_delta_between_arms": changed,
        "initial_oracle_status": initial_quality,
        "recovery_oracle_status": recovery_quality,
        "initial_regression_status": control_oracles.get("regression_status"),
        "recovery_regression_status": treatment_oracles.get("regression_status"),
        "internal_external_outcome_matrix": {
            "control": {
                "internal": "failed_recoverable",
                "external": initial_quality,
            },
            "treatment": {
                "internal": treatment.get("terminal_status", {}).get("status"),
                "external": recovery_quality,
            },
        },
    }


def summarize_frozen_external_oracles(
    outcomes: list[dict[str, Any]],
) -> dict[str, Any]:
    results = [row.get("outcome", {}).get("result") for row in outcomes]
    if not outcomes or any(
        result in {"unverified", "oracle_error", "blocked", None} for result in results
    ):
        overall = "unusable"
    elif any(result == "fail" for result in results):
        overall = "fail"
    elif all(result == "pass" for result in results):
        overall = "pass"
    else:
        overall = "unusable"
    regressions = [row for row in outcomes if row.get("is_regression") is True]
    regression_results = [row.get("outcome", {}).get("result") for row in regressions]
    if not regressions:
        regression_status = "not_applicable"
    elif any(
        result in {"unverified", "oracle_error", "blocked", None}
        for result in regression_results
    ):
        regression_status = "unusable"
    elif any(result == "fail" for result in regression_results):
        regression_status = "fail"
    elif all(result == "pass" for result in regression_results):
        regression_status = "pass"
    else:
        regression_status = "unusable"
    return {
        "source": "frozen_host_adapter_post_execution",
        "overall": overall,
        "regression_status": regression_status,
        "oracle_count": len(outcomes),
        "pass_count": results.count("pass"),
        "fail_count": results.count("fail"),
        "unusable_count": len(outcomes) - results.count("pass") - results.count("fail"),
        "outcomes": outcomes,
    }


def artifact_delta(before: dict[str, Any], after: dict[str, Any]) -> dict[str, Any]:
    before_entries = _entries_by_path(before)
    after_entries = _entries_by_path(after)
    added = sorted(after_entries.keys() - before_entries.keys())
    removed = sorted(before_entries.keys() - after_entries.keys())
    changed = sorted(
        path
        for path in before_entries.keys() & after_entries.keys()
        if before_entries[path] != after_entries[path]
    )
    return {
        "before_snapshot_sha256": before.get("snapshot_sha256"),
        "after_snapshot_sha256": after.get("snapshot_sha256"),
        "added": added,
        "removed": removed,
        "changed": changed,
        "change_count": len(added) + len(removed) + len(changed),
    }


def compare_recovery_arms(
    *,
    initial_only: dict[str, Any],
    recovery_one: dict[str, Any],
    initial_oracles: dict[str, Any],
    recovery_oracles: dict[str, Any],
    initial_artifact_manifest: dict[str, Any],
    recovery_artifact_manifest: dict[str, Any],
) -> dict[str, Any]:
    initial_attempts = initial_only.get("recovery_plan_attempts", {})
    recovery_attempts = recovery_one.get("recovery_plan_attempts", {})
    if initial_attempts.get("configured_recovery_runs") != 0:
        raise ValueError("initial-only arm must configure zero Recovery Plan runs")
    if recovery_attempts.get("configured_recovery_runs") != 1:
        raise ValueError("recovery-one arm must configure one Recovery Plan run")
    initial_quality = initial_oracles.get("overall")
    recovery_quality = recovery_oracles.get("overall")
    executed_recovery_runs = recovery_attempts.get("executed_recovery_runs")
    if initial_quality == "fail" and recovery_quality == "pass":
        raw_transition = "improved"
    elif initial_quality == "pass" and recovery_quality != "pass":
        raw_transition = "harmed"
    elif initial_quality == recovery_quality == "pass":
        raw_transition = "unchanged_pass"
    elif initial_quality == recovery_quality == "fail":
        raw_transition = "unchanged_fail"
    else:
        raw_transition = "unusable"
    if executed_recovery_runs == 1:
        transition = raw_transition
    elif raw_transition in {"improved", "harmed"}:
        transition = "initial_attempt_divergence"
    elif raw_transition == "unchanged_pass":
        transition = "no_recovery_needed"
    elif raw_transition == "unchanged_fail":
        transition = "no_recovery_executed"
    else:
        transition = "unusable"
    initial_regression = initial_oracles.get("regression_status")
    recovery_regression = recovery_oracles.get("regression_status")
    return {
        "quality_transition": transition,
        "raw_oracle_transition": raw_transition,
        "success_improved": executed_recovery_runs == 1
        and raw_transition == "improved",
        "existing_artifact_harmed": executed_recovery_runs == 1
        and raw_transition == "harmed",
        "regression_introduced": (
            executed_recovery_runs == 1
            and initial_regression == "pass"
            and recovery_regression == "fail"
        ),
        "executed_recovery_runs": executed_recovery_runs,
        "initial_attempts": initial_attempts.get("attempts", []),
        "recovery_attempts": recovery_attempts.get("attempts", []),
        "resource_delta": _resource_delta(
            initial_only.get("resource_usage", {}),
            recovery_one.get("resource_usage", {}),
        ),
        "artifact_delta_between_arms": artifact_delta(
            initial_artifact_manifest, recovery_artifact_manifest
        ),
        "initial_oracle_status": initial_quality,
        "recovery_oracle_status": recovery_quality,
        "initial_regression_status": initial_regression,
        "recovery_regression_status": recovery_regression,
    }


def _classification(*, eligible: bool, category: str, reason: str) -> dict[str, Any]:
    return {
        "policy_id": POLICY_ID,
        "eligible": eligible,
        "category": category,
        "reason": reason,
    }


def _normalize_product_oracle_outcome(outcome: dict[str, Any]) -> dict[str, Any]:
    if outcome.get("result") != "blocked":
        return outcome
    if outcome.get("reason") not in {
        "playwright_unavailable_or_failed",
        "server_not_ready",
        "timeout",
    }:
        return outcome
    return {
        **outcome,
        "original_result": "blocked",
        "result": "fail",
        "a14_attribution": "product_did_not_reach_frozen_observation_boundary",
    }


def _runtime_classification(*, run: bool, category: str, reason: str) -> dict[str, Any]:
    return {
        "policy_id": POLICY_ID,
        "eligible": run,
        "category": category,
        "reason": reason,
        "run_recovery_one_arm": run,
        "runtime_source": "structured_terminal_status",
    }


def _entries_by_path(manifest: dict[str, Any]) -> dict[str, tuple[Any, ...]]:
    return {
        row["path"]: (
            row.get("kind"),
            row.get("sha256"),
            row.get("size"),
            row.get("target"),
        )
        for row in manifest.get("entries", [])
        if isinstance(row, dict) and isinstance(row.get("path"), str)
    }


def _resource_delta(
    initial: dict[str, Any], recovery: dict[str, Any]
) -> dict[str, int | None]:
    result = {}
    for field in ("wall_time_ms", "input_tokens", "output_tokens", "total_tokens"):
        left = initial.get(field)
        right = recovery.get(field)
        result[field] = (
            right - left
            if isinstance(left, int)
            and not isinstance(left, bool)
            and isinstance(right, int)
            and not isinstance(right, bool)
            else None
        )
    return result
