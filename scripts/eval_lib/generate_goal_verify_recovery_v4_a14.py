from __future__ import annotations

import argparse
from pathlib import Path
from typing import Any

from eval_lib.generate_goal_verify_main_v4 import _load, _write_json
from eval_lib.goal_verify_manifest_visibility_v4 import POLICY_ID as MANIFEST_POLICY_ID
from eval_lib.goal_verify_recovery_experiment_v4 import (
    POLICY_ID as RECOVERY_POLICY_ID,
)
from eval_lib.goal_verify_recovery_experiment_v4 import (
    classify_case_recovery_eligibility,
)

ROOT = Path(__file__).resolve().parents[2]
EVAL = ROOT / "eval/goal_verify/v0"

CONTRACT_PATH = EVAL / "phase6-recovery-v4-a14-contract.json"
TASKS_PATH = EVAL / "phase6-task-contracts-v4-a13-main.json"
ADAPTERS_PATH = EVAL / "phase6-command-adapters-v4-a13-main.json"

CONTRACT_ID = "phase6-recovery-v4-20260829-a14-live-01"
SMOKE_ID = "phase6-recovery-v4-20260829-a14-smoke-01"
SMOKE_CASE_IDS = [
    "phase6-main-c01-task-01",
    "phase6-main-c04-task-01",
    "phase6-main-c06-task-01",
    "phase6-main-c07-task-01",
]


def _build_contract(
    *,
    status: str,
    code_sha: str,
    exact_sha_ci_evidence: str,
    live_collection_authorized: bool,
) -> dict[str, Any]:
    tasks = _load(TASKS_PATH)
    adapters = _load(ADAPTERS_PATH)["adapters"]
    tasks_by_id = {row["case_id"]: row for row in tasks["cases"]}
    eligibility = {
        case_id: classify_case_recovery_eligibility(
            task_contract=tasks_by_id[case_id], adapters=adapters
        )
        for case_id in SMOKE_CASE_IDS
    }
    return {
        "schema_version": "commandagent.goal_verify.recovery_experiment.v4_a14",
        "status": status,
        "contract_id": CONTRACT_ID,
        "smoke_run_id": SMOKE_ID,
        "supersedes": "phase6-main-v4-20260829-live-02",
        "superseded_run_policy": (
            "live-02 remains immutable early-terminated diagnostic evidence and is "
            "never resumed, rescored, or used for A14 effect inference"
        ),
        "code_sha": code_sha,
        "exact_sha_ci_evidence": exact_sha_ci_evidence,
        "endpoint": "http://127.0.0.1:11434/api/generate",
        "model": "qwen3.6:27b-coding-nvfp4",
        "model_digest": (
            "42a2d9de99b0e72ab7022637dd3f8ee3103e116e4b287901080b7c9c9cc0ee66"
        ),
        "corpus": "eval/goal_verify/v0/phase6-main-corpus-v4.json",
        "task_contract_registry": str(TASKS_PATH.relative_to(ROOT)),
        "frozen_external_oracles": str(ADAPTERS_PATH.relative_to(ROOT)),
        "workspace_registry": "eval/goal_verify/v0/phase6-real-workspaces-v3.json",
        "workspace_registry_additions": (
            "eval/goal_verify/v0/phase6-real-workspaces-v4-main.json"
        ),
        "resource_budget_config": "eval/goal_verify/v0/baseline-config.json",
        "product_timeout_sec": 900,
        "execution_root_policy": {
            "required_root": "/Volumes/SSD_NX/tmp/commandagent_trial",
            "internal_disk_execution_forbidden": True,
        },
        "paired_run_contract": {
            "pairing_unit": "same case, source fixture, product code SHA, model, and task contract",
            "arm_order": ["initial_only", "recovery_one"],
            "initial_only": {"recovery_plan_auto_runs": 0},
            "recovery_one": {"recovery_plan_auto_runs": 1},
            "maximum_recovery_runs": 1,
            "independent_workspace_copies": True,
            "same_input_snapshot_sha256_required": True,
            "stochastic_pairing_limitation": (
                "the two product invocations are task-paired but do not share one "
                "physical initial attempt; initial attempt telemetry is retained"
            ),
        },
        "recovery_eligibility": {
            "policy_id": RECOVERY_POLICY_ID,
            "pre_run_categories": [
                "dependency_or_provisioning",
                "capability_unavailable",
            ],
            "runtime_excluded_categories": [
                "dependency_or_provisioning",
                "capability_unavailable",
                "profile_or_completion_contract",
                "sandbox_or_policy",
                "task_information_missing",
                "instrumentation_unavailable",
                "recovery_candidate_unavailable",
            ],
            "free_form_stderr_used_for_classification": False,
            "structured_terminal_event_required": True,
            "ineligible_pairs_retained": (
                "retain with recovery_one not run; never relabel or drop from the "
                "reported task denominator"
            ),
            "preregistered_smoke_cases": eligibility,
        },
        "external_oracle_policy": {
            "source": "frozen host-owned adapters",
            "timing": "after each product arm terminates",
            "passed_to_product_or_recovery": False,
            "self_report_used_for_success": False,
            "unverified_or_error": "unusable, never pass",
            "product_observation_boundary_failures": [
                "server_not_ready",
                "timeout",
                "playwright_unavailable_or_failed",
            ],
        },
        "artifact_policy": {
            "full_snapshot_identity_retained": True,
            "candidate_visible_manifest_policy": MANIFEST_POLICY_ID,
            "generated_cache_entries_excluded_from_artifact_delta": True,
            "existing_artifact_harm": (
                "initial_only external oracle pass and recovery_one external oracle "
                "non-pass"
            ),
            "regression_introduced": (
                "initial_only frozen regression pass and recovery_one frozen "
                "regression fail"
            ),
        },
        "measurements": {
            "required": [
                "initial and recovery attempt status",
                "external oracle transition",
                "existing artifact harm",
                "regression introduced",
                "source/config artifact delta",
                "wall_time_ms",
                "input_tokens",
                "output_tokens",
                "total_tokens",
            ],
            "missing_resource_values": "record null; never impute",
        },
        "integrity": {
            "exclusive_run_lock": ".campaign.lock",
            "record_ledger": "record-ledger.jsonl",
            "append_only_records": True,
        },
        "smoke": {
            "inference_role": "instrument diagnostic only",
            "selected_pair_ids": [f"{case_id}--pair-01" for case_id in SMOKE_CASE_IDS],
            "expected_pair_count": len(SMOKE_CASE_IDS),
            "includes_next_manifest_regression": True,
            "includes_dependency_exclusion_sentinel": True,
            "effect_claim_allowed": False,
        },
        "analysis": {
            "primary_transition": "fail to pass by frozen external oracle",
            "attribution_requires_executed_recovery_runs": 1,
            "configured_one_but_executed_zero": (
                "arm differences are initial-attempt divergence, not Recovery effect"
            ),
            "safety_transitions": [
                "pass to non-pass",
                "regression pass to fail",
            ],
            "cost_deltas": [
                "wall_time_ms",
                "input_tokens",
                "output_tokens",
                "total_tokens",
            ],
            "bootstrap_iterations": 2000,
            "confidence_interval": 0.95,
            "exclusion_rules_change_after_collection": "forbidden",
        },
        "authorization": {
            "implementation_authorized": True,
            "smoke_collection_authorized": live_collection_authorized,
            "full_collection_authorized": False,
            "approved_at": "2026-08-29" if live_collection_authorized else None,
            "approved_by": "repository owner" if live_collection_authorized else None,
        },
        "runner_sources": [
            "scripts/eval-goal-verify-recovery-v4-report.py",
            "scripts/eval-goal-verify-recovery-v4.py",
            "scripts/eval_lib/generate_goal_verify_recovery_v4_a14.py",
            "scripts/eval_lib/goal_verify_additive_v4.py",
            "scripts/eval_lib/goal_verify_baseline_product_v3.py",
            "scripts/eval_lib/goal_verify_manifest_visibility_v4.py",
            "scripts/eval_lib/goal_verify_recovery_experiment_v4.py",
            "scripts/eval_lib/goal_verify_recovery_live_v4.py",
            "scripts/eval_lib/goal_verify_recovery_report_v4.py",
        ],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate A14 Recovery 0-vs-1 inputs")
    parser.add_argument("--code-sha")
    parser.add_argument("--exact-sha-ci-evidence")
    parser.add_argument("--smoke-collection-authorized", action="store_true")
    args = parser.parse_args()
    if bool(args.code_sha) != bool(args.exact_sha_ci_evidence):
        parser.error("--code-sha and --exact-sha-ci-evidence must be paired")
    if args.smoke_collection_authorized and not args.code_sha:
        parser.error("smoke authorization requires exact-SHA inputs")
    contract = _build_contract(
        status="frozen" if args.code_sha else "draft",
        code_sha=args.code_sha or "",
        exact_sha_ci_evidence=args.exact_sha_ci_evidence or "",
        live_collection_authorized=args.smoke_collection_authorized,
    )
    _write_json(CONTRACT_PATH, contract)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
