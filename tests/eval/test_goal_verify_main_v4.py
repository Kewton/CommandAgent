from __future__ import annotations

import copy
import json
import shlex
import shutil
import tempfile
import unittest
from pathlib import Path

from eval_lib.generate_goal_verify_main_v4 import _tracked_files
from eval_lib.generate_goal_verify_main_v4_a13 import (
    _build_adapters as build_a13_adapters,
)
from eval_lib.generate_goal_verify_main_v4_a13 import (
    _build_contract as build_a13_contract,
)
from eval_lib.generate_goal_verify_main_v4_a13 import _build_tasks as build_a13_tasks
from eval_lib.generate_goal_verify_recovery_v4_a14_a1 import (
    _build_contract as build_a14_recovery_contract,
)
from eval_lib.generate_goal_verify_recovery_v4_a14_a2 import (
    _build_adapters as build_a14_a2_adapters,
)
from eval_lib.generate_goal_verify_recovery_v4_a14_a2 import (
    _build_contract as build_a14_a2_contract,
)
from eval_lib.generate_goal_verify_recovery_v4_a14_a2 import (
    _build_tasks as build_a14_a2_tasks,
)
from eval_lib.generate_goal_verify_recovery_v4_a14_a3 import (
    _build_contract as build_a14_a3_contract,
)
from eval_lib.generate_goal_verify_recovery_v4_a14_a4 import (
    _build_contract as build_a14_a4_contract,
)
from eval_lib.generate_goal_verify_recovery_v4_a14_a5 import (
    _build_contract as build_a14_a5_contract,
)
from eval_lib.generate_goal_verify_recovery_v4_a14_a6 import (
    _build_adapters as build_a14_a6_adapters,
)
from eval_lib.generate_goal_verify_recovery_v4_a14_a6 import (
    _build_contract as build_a14_a6_contract,
)
from eval_lib.generate_goal_verify_recovery_v4_a14_a6 import (
    _build_tasks as build_a14_a6_tasks,
)
from eval_lib.generate_goal_verify_recovery_v4_a14_a7 import (
    _build_contract as build_a14_a7_contract,
)
from eval_lib.generate_goal_verify_recovery_v4_a14_a8 import (
    _build_contract as build_a14_a8_contract,
)
from eval_lib.generate_goal_verify_recovery_v4_a14_a9 import (
    _build_contract as build_a14_a9_contract,
)
from eval_lib.generate_goal_verify_recovery_v4_a14_a10 import (
    _build_contract as build_a14_a10_contract,
)
from eval_lib.generate_goal_verify_recovery_v4_a14_a11 import (
    _build_contract as build_a14_a11_contract,
)
from eval_lib.generate_goal_verify_recovery_v4_a14_a12 import (
    _build_contract as build_a14_a12_contract,
)
from eval_lib.generate_goal_verify_recovery_v4_a14_a13 import (
    _build_adapters as build_a14_a13_adapters,
)
from eval_lib.generate_goal_verify_recovery_v4_a14_a13 import (
    _build_contract as build_a14_a13_contract,
)
from eval_lib.generate_goal_verify_recovery_v4_a14_a13 import (
    _build_tasks as build_a14_a13_tasks,
)
from eval_lib.generate_goal_verify_recovery_v4_a14_a13_1 import (
    _build_contract as build_a14_a13_1_contract,
)
from eval_lib.generate_goal_verify_recovery_v4_a14_a13_1 import (
    _build_tasks as build_a14_a13_1_tasks,
)
from eval_lib.generate_goal_verify_recovery_v4_a14_a13_2 import (
    _build_contract as build_a14_a13_2_contract,
)
from eval_lib.generate_goal_verify_recovery_v4_a14_a13_3 import (
    _build_contract as build_a14_a13_3_contract,
)
from eval_lib.generate_goal_verify_recovery_v4_a14_a14 import (
    _build_contract as build_a14_a14_contract,
)
from eval_lib.goal_verify_baseline_product_v3 import (
    _completion_verify_status,
    _fix_reproducer_binding,
    _product_resource_usage,
    _product_terminal_status,
    _recovery_boundary,
    _recovery_plan_attempts,
    build_product_argv,
)
from eval_lib.goal_verify_blind_v4 import (
    build_blind_report,
    canonical_sha256,
    human_sample,
)
from eval_lib.goal_verify_executors_v3 import _run_registered_browser_command
from eval_lib.goal_verify_live_v4 import _candidate_resource_usage, _cluster_manifest
from eval_lib.goal_verify_main_design_v4 import main_design_errors
from eval_lib.goal_verify_main_report_v4 import (
    _a13_instrument_checks,
    build_main_report,
    build_main_smoke_report,
    evaluate_main_semantic_review,
)
from eval_lib.goal_verify_recovery_experiment_v4 import (
    artifact_delta,
    classify_case_recovery_eligibility,
    classify_initial_recovery_eligibility,
    compare_recovery_arms,
    compare_shared_recovery_boundary,
    execute_frozen_external_oracles,
    recovery_contract_errors,
    validate_a14_oracle_semantics,
)
from eval_lib.goal_verify_recovery_live_v4 import (
    _attach_frozen_browser_toolchain,
    _ensure_oracle_executability_preflight,
    _snapshot_content_sha256,
    bind_selected_recovery_cases,
    parse_recovery_pair_id,
    run_recovery_pair,
)
from eval_lib.goal_verify_recovery_report_v4 import (
    _typed_reproducer_matches,
    build_recovery_report,
)
from eval_lib.goal_verify_resource_diagnostics_v4 import build_resource_diagnostics
from eval_lib.goal_verify_task_contracts_v4 import (
    bind_task_contract,
    load_task_contract_registry,
    task_contract_registry_errors,
)
from eval_lib.goal_verify_workspaces_v3 import workspace_by_case
from eval_lib.goal_verify_workspaces_v4 import load_v4_workspace_registry

ROOT = Path(__file__).resolve().parents[2]


def load(relative: str):
    return json.loads((ROOT / relative).read_text(encoding="utf-8"))


class GoalVerifyMainV4Test(unittest.TestCase):
    def test_recovery_browser_toolchain_is_attached_after_product_execution(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            toolchain = root / "candidate-toolchain"
            workspace = root / "workspace"
            (toolchain / "playwright-core").mkdir(parents=True)
            (toolchain / "playwright-core/package.json").write_text(
                '{}', encoding="utf-8"
            )
            workspace.mkdir()

            attached = _attach_frozen_browser_toolchain(toolchain, workspace)

            link = workspace / "node_modules/playwright-core"
            self.assertEqual(attached, ["node_modules/playwright-core"])
            self.assertTrue(link.is_symlink())
            self.assertEqual(
                link.resolve(), (toolchain / "playwright-core").resolve()
            )
            self.assertEqual(
                _attach_frozen_browser_toolchain(toolchain, workspace), []
            )

    def test_generated_workspace_file_list_ignores_runtime_caches(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            stage = root / "before"
            (stage / "__pycache__").mkdir(parents=True)
            (stage / ".pytest_cache").mkdir()
            (stage / "keep.py").write_text("pass\n", encoding="utf-8")
            (stage / "__pycache__/keep.pyc").write_bytes(b"cache")
            (stage / ".pytest_cache/state").write_text("cache", encoding="utf-8")
            (stage / ".DS_Store").write_bytes(b"cache")
            self.assertEqual(_tracked_files(root, "before"), ["before/keep.py"])

    def test_product_resource_usage_is_frozen_from_terminal_time_profile(self):
        with tempfile.TemporaryDirectory() as temporary:
            run_dir = Path(temporary)
            rows = [
                {
                    "event": "time_profile",
                    "profile": {
                        "total_ms": 1234,
                        "prompt_eval_count": 40,
                        "eval_count": 2,
                    },
                }
            ]
            (run_dir / "events.jsonl").write_text(
                "\n".join(json.dumps(row) for row in rows) + "\n",
                encoding="utf-8",
            )
            self.assertEqual(
                _product_resource_usage(run_dir),
                {
                    "wall_time_ms": 1234,
                    "input_tokens": 40,
                    "output_tokens": 2,
                    "total_tokens": 42,
                },
            )

    def test_recovery_completion_requires_post_contract_pass_promotion_and_success(self):
        recovery_events = [
            {
                "event": "recovery_preflight_observation",
                "observation_phase": "post_recovery",
                "status": "pass",
                "source": "product_visible_completion_contract",
                "reason": (
                    "registered_final_success_and_completion_contract_passed:"
                    "1 commands"
                ),
            },
            {
                "event": "recovery_promotion_decision",
                "decision": "promoted",
            },
            {
                "event": "recovery_plan_auto_run_complete",
                "recovery_plan_auto_run_stop_reason": "recovery_succeeded",
            },
        ]
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            complete = root / "complete"
            complete.mkdir()
            rows = [{"event": "completion_verify", "ok": False}, *recovery_events]
            (complete / "events.jsonl").write_text(
                "\n".join(json.dumps(row) for row in rows) + "\n",
                encoding="utf-8",
            )
            self.assertEqual(
                _completion_verify_status(complete),
                {
                    "completion_verify_attempt_recorded": True,
                    "completion_verify_passed": True,
                },
            )

            for omitted in range(len(recovery_events)):
                incomplete = root / f"incomplete-{omitted}"
                incomplete.mkdir()
                rows = [
                    {"event": "completion_verify", "ok": False},
                    *(
                        row
                        for index, row in enumerate(recovery_events)
                        if index != omitted
                    ),
                ]
                (incomplete / "events.jsonl").write_text(
                    "\n".join(json.dumps(row) for row in rows) + "\n",
                    encoding="utf-8",
                )
                self.assertFalse(
                    _completion_verify_status(incomplete)[
                        "completion_verify_passed"
                    ]
                )
    def test_typed_fix_reproducer_binding_is_reduced_to_durable_fields(self):
        with tempfile.TemporaryDirectory() as temporary:
            run_dir = Path(temporary)
            rows = [
                {
                    "event": "fix_plan_synthesized",
                    "r_basis": "completion_contract:fix_reproducer_command",
                },
                {
                    "event": "ultra_phase_plan_validated",
                    "phase_id": "reproduce-before",
                    "step_count": 1,
                },
                {
                    "event": "fix_evidence_recorded",
                    "requirement_id": "before_fails",
                    "stage": "before",
                    "executed": True,
                    "expected_polarity": "failure",
                    "outcome": "failure",
                    "binding_id": "python3 cli.py 7",
                },
            ]
            (run_dir / "events.jsonl").write_text(
                "\n".join(json.dumps(row) for row in rows) + "\n",
                encoding="utf-8",
            )
            binding = _fix_reproducer_binding(run_dir)

        self.assertTrue(_typed_reproducer_matches(binding, "python3 cli.py 7"))
        self.assertFalse(_typed_reproducer_matches(binding, "python3 cli.py 8"))

    def test_recovery_boundary_splits_provider_usage_at_shared_snapshot(self):
        with tempfile.TemporaryDirectory() as temporary:
            run_dir = Path(temporary)
            rows = [
                {
                    "event": "provider_turn_duration",
                    "duration_ms": 10,
                    "prompt_eval_count": 20,
                    "eval_count": 3,
                },
                {
                    "event": "recovery_boundary_snapshot",
                    "status": "captured",
                    "workspace_relative_path": (
                        ".commandagent/recovery-boundaries/attempt-1/workspace"
                    ),
                    "file_count": 2,
                    "total_bytes": 30,
                    "snapshot_sha256": "b" * 64,
                },
                {"event": "recovery_plan_auto_run_start"},
                {
                    "event": "provider_turn_duration",
                    "duration_ms": 7,
                    "prompt_eval_count": 11,
                    "eval_count": 2,
                },
            ]
            (run_dir / "events.jsonl").write_text(
                "\n".join(json.dumps(row) for row in rows) + "\n",
                encoding="utf-8",
            )
            boundary = _recovery_boundary(run_dir)

        self.assertEqual(boundary["status"], "captured")
        self.assertEqual(boundary["snapshot_sha256"], "b" * 64)
        self.assertEqual(boundary["initial_provider_usage"]["total_tokens"], 23)
        self.assertEqual(boundary["recovery_provider_usage"]["total_tokens"], 13)

    def test_product_terminal_status_uses_last_structured_stop_event(self):
        with tempfile.TemporaryDirectory() as temporary:
            run_dir = Path(temporary)
            rows = [
                {
                    "event": "run_stop",
                    "ok": False,
                    "status": "failed",
                    "failure_kind": "older_failure",
                },
                {
                    "event": "recovery_prompt_saved",
                    "failure_kind": "dependency_setup_blocked_offline",
                    "recovery_ultra_plan_path": ".anvil/plans/recovery.yaml",
                },
                {
                    "event": "plan_final_contract",
                    "primary_reason": (
                        "missing_required_capabilities:"
                        "unsupported_required_capability:browser_readiness"
                    ),
                    "missing_capabilities": [
                        "unsupported_required_capability:browser_readiness"
                    ],
                    "recovery_handoff_kind": "release_gate_failed",
                },
                {
                    "event": "tui_command_stop",
                    "ok": False,
                    "status": "failed",
                    "failure_kind": "direct_cli_command_failed",
                    "stop_reason": "dependency cache is unavailable",
                    "recovery_next_action": "fix_dependency_boundary",
                    "recovery_ultra_plan_path": ".anvil/plans/recovery.yaml",
                },
            ]
            (run_dir / "events.jsonl").write_text(
                "\n".join(json.dumps(row) for row in rows) + "\n",
                encoding="utf-8",
            )

            status = _product_terminal_status(run_dir)

        self.assertTrue(status["recorded"])
        self.assertEqual(status["event"], "tui_command_stop")
        self.assertEqual(status["failure_kind"], "direct_cli_command_failed")
        self.assertEqual(
            status["recovery_failure_kind"], "dependency_setup_blocked_offline"
        )
        self.assertEqual(status["recovery_handoff_kind"], "release_gate_failed")
        self.assertIn(
            "unsupported_required_capability:browser_readiness",
            status["structured_blockers"],
        )
        self.assertEqual(status["next_action"], "fix_dependency_boundary")
        self.assertEqual(
            status["recovery_ultra_plan_path"], ".anvil/plans/recovery.yaml"
        )

    def test_recovery_zero_records_initial_attempt_without_recovery(self):
        with tempfile.TemporaryDirectory() as temporary:
            telemetry = _recovery_plan_attempts(
                Path(temporary), configured_runs=0, process_status="failed"
            )

        self.assertEqual(telemetry["configured_recovery_runs"], 0)
        self.assertEqual(telemetry["executed_recovery_runs"], 0)
        self.assertEqual(telemetry["attempts"][0]["kind"], "initial")
        self.assertEqual(telemetry["attempts"][0]["status"], "failed")
        self.assertEqual(telemetry["terminal_stop_reason"], "disabled")

    def test_recovery_one_exposes_initial_failure_and_first_recovery_success(self):
        with tempfile.TemporaryDirectory() as temporary:
            run_dir = Path(temporary)
            rows = [
                {
                    "event": "recovery_plan_auto_run_configured",
                    "recovery_plan_auto_run_current": 0,
                    "recovery_plan_auto_runs": 1,
                    "recovery_plan_auto_run_stop_reason": "initial_run",
                },
                {
                    "event": "recovery_plan_auto_run_start",
                    "recovery_plan_auto_run_current": 1,
                    "recovery_plan_auto_runs": 1,
                    "recovery_plan_auto_run_stop_reason": "running",
                    "recovery_handoff_kind": "verification_failed",
                    "recovery_ultra_plan_path": ".anvil/plans/recovery.yaml",
                    "recovery_verify_command_source": "completion_contract",
                },
                {
                    "event": "recovery_plan_auto_run_complete",
                    "recovery_plan_auto_run_current": 1,
                    "recovery_plan_auto_runs": 1,
                    "recovery_plan_auto_run_stop_reason": "recovery_succeeded",
                },
                {
                    "event": "recovery_step_plan_verify_commands_bound",
                    "phase_id": "inspect-current-state",
                    "binding_mode": "read_only_inspection",
                    "source": "product_visible_completion_contract",
                    "external_oracle_used": False,
                    "original_verify_commands": ["python3 cli.py 16"],
                    "bound_verify_commands": [],
                    "registered_verify_commands": ["python3 cli.py 16"],
                    "removed_step_ids": ["reproduce-failure"],
                },
                {
                    "event": "recovery_fix_contract_resumed",
                    "original_intent": "fix",
                    "contract_origin": "fix_intent_v0",
                    "contract_version": "v0",
                    "contract_ref": "docs/fix-intent-contract.md",
                    "fix_run_id": "01a-test-fix",
                    "reproducer_command": "python3 cli.py 16",
                    "source": "host_owned_recovery_fix_origin",
                    "external_oracle_used": False,
                },
            ]
            (run_dir / "events.jsonl").write_text(
                "\n".join(json.dumps(row) for row in rows) + "\n",
                encoding="utf-8",
            )
            telemetry = _recovery_plan_attempts(
                run_dir, configured_runs=1, process_status="succeeded"
            )

        self.assertEqual(telemetry["configured_recovery_runs"], 1)
        self.assertEqual(telemetry["executed_recovery_runs"], 1)
        self.assertTrue(telemetry["configured_matches_events"])
        self.assertEqual(
            [(row["kind"], row["status"]) for row in telemetry["attempts"]],
            [("initial", "failed_recoverable"), ("recovery", "succeeded")],
        )
        self.assertEqual(telemetry["terminal_stop_reason"], "recovery_succeeded")
        self.assertEqual(
            telemetry["attempts"][1]["recovery_verify_command_source"],
            "completion_contract",
        )
        self.assertEqual(
            telemetry["step_plan_contract_bindings"][0]["binding_mode"],
            "read_only_inspection",
        )
        self.assertEqual(
            telemetry["fix_contract_resumptions"][0]["fix_run_id"],
            "01a-test-fix",
        )

    def test_product_argv_makes_recovery_zero_and_one_explicit(self):
        common = {
            "commandagent_bin": Path("/tmp/commandagent"),
            "workspace": Path("/tmp/workspace"),
            "case": {"intent": "fix", "profile": "generic", "goal": "repair"},
            "model": "model",
        }
        zero = build_product_argv(**common, recovery_plan_auto_runs=0)
        one = build_product_argv(**common, recovery_plan_auto_runs=1)

        zero_index = zero.index("--recovery-plan-auto-runs")
        one_index = one.index("--recovery-plan-auto-runs")
        self.assertEqual(zero[zero_index : zero_index + 2], [zero[zero_index], "0"])
        self.assertEqual(one[one_index : one_index + 2], [one[one_index], "1"])
        self.assertIn("--plan-run", zero)

        ultra = build_product_argv(
            **common,
            recovery_plan_auto_runs=1,
            execution_action="ultra_plan_run",
        )
        self.assertIn("--ultra-plan-run", ultra)
        self.assertNotIn("--plan-run", ultra)
        with self.assertRaisesRegex(ValueError, "unsupported execution_action"):
            build_product_argv(**common, execution_action="unknown")

    def test_a14_case_eligibility_excludes_missing_dependency_and_oracle(self):
        tasks = load("eval/goal_verify/v0/phase6-task-contracts-v4-a13-main.json")
        adapters = load("eval/goal_verify/v0/phase6-command-adapters-v4-a13-main.json")[
            "adapters"
        ]
        by_id = {row["case_id"]: row for row in tasks["cases"]}

        eligible = classify_case_recovery_eligibility(
            task_contract=by_id["phase6-main-c01-task-01"], adapters=adapters
        )
        unavailable_oracle = classify_case_recovery_eligibility(
            task_contract=by_id["phase6-main-c02-task-01"], adapters=adapters
        )
        unavailable_dependency = classify_case_recovery_eligibility(
            task_contract=by_id["phase6-main-c06-task-01"], adapters=adapters
        )

        self.assertTrue(eligible["eligible"])
        self.assertEqual(unavailable_oracle["category"], "capability_unavailable")
        self.assertEqual(
            unavailable_dependency["category"], "dependency_or_provisioning"
        )

    def test_a14_runtime_eligibility_uses_structured_failure_only(self):
        preregistered = {
            "policy_id": "p",
            "eligible": True,
            "category": "recoverable_candidate",
            "reason": "available",
        }
        dependency = classify_initial_recovery_eligibility(
            preregistered=preregistered,
            baseline={
                "stderr": "bounded repair text that must not influence policy",
                "terminal_status": {
                    "recorded": True,
                    "ok": False,
                    "failure_kind": "dependency_setup_blocked_offline",
                    "recovery_ultra_plan_path": ".anvil/plans/recovery.yaml",
                },
            },
        )
        recoverable = classify_initial_recovery_eligibility(
            preregistered=preregistered,
            baseline={
                "stderr": "dependency_setup_blocked_offline is ignored here",
                "terminal_status": {
                    "recorded": True,
                    "ok": False,
                    "failure_kind": "verify_repair_progress_unchanged",
                    "recovery_ultra_plan_path": ".anvil/plans/recovery.yaml",
                },
            },
        )

        self.assertFalse(dependency["run_recovery_one_arm"])
        self.assertEqual(dependency["category"], "dependency_or_provisioning")
        self.assertTrue(recoverable["run_recovery_one_arm"])
        self.assertEqual(recoverable["category"], "recoverable_candidate")

    def test_a14_runtime_eligibility_excludes_structured_capability_blocker(self):
        eligibility = classify_initial_recovery_eligibility(
            preregistered={"eligible": True},
            baseline={
                "terminal_status": {
                    "recorded": True,
                    "ok": False,
                    "failure_kind": "direct_cli_command_failed",
                    "recovery_failure_kind": None,
                    "structured_blockers": [
                        "unsupported_required_capability:browser_readiness"
                    ],
                    "recovery_ultra_plan_path": ".anvil/plans/recovery.yaml",
                }
            },
        )

        self.assertFalse(eligibility["run_recovery_one_arm"])
        self.assertEqual(eligibility["category"], "capability_unavailable")

    def test_a14_external_oracle_is_host_owned_and_fail_closed(self):
        calls = []

        def executor(spec, *, workspace):
            calls.append((spec["kind"], workspace))
            return {"executed": True, "result": spec["expected_result"]}

        summary = execute_frozen_external_oracles(
            case_id="case-a",
            adapters=[
                {
                    "adapter_id": "primary",
                    "case_id": "case-a",
                    "claim_id": "behavior",
                    "executor": {
                        "kind": "sandbox_command",
                        "expected_result": "pass",
                    },
                },
                {
                    "adapter_id": "regression",
                    "case_id": "case-a",
                    "claim_id": "regressions",
                    "executor": {
                        "kind": "regression_set",
                        "expected_result": "unverified",
                    },
                },
            ],
            workspace=Path("/tmp/a14-workspace"),
            executor=executor,
        )

        self.assertEqual(summary["source"], "frozen_host_adapter_post_execution")
        self.assertEqual(summary["overall"], "unusable")
        self.assertEqual(summary["regression_status"], "unusable")
        self.assertEqual(len(calls), 2)

    def test_a14_server_not_ready_is_product_fail_not_oracle_unusable(self):
        summary = execute_frozen_external_oracles(
            case_id="case-a",
            adapters=[
                {
                    "adapter_id": "browser",
                    "case_id": "case-a",
                    "claim_id": "behavior",
                    "executor": {"kind": "playwright_script"},
                }
            ],
            workspace=Path("/tmp/a14-workspace"),
            executor=lambda spec, *, workspace: {
                "executed": False,
                "result": "blocked",
                "reason": "server_not_ready",
            },
        )

        self.assertEqual(summary["overall"], "fail")
        self.assertEqual(
            summary["outcomes"][0]["outcome"]["a14_attribution"],
            "product_did_not_reach_frozen_observation_boundary",
        )

    def test_a14_comparison_reports_improvement_cost_and_regression(self):
        initial = {
            "recovery_plan_attempts": {
                "configured_recovery_runs": 0,
                "executed_recovery_runs": 0,
                "attempts": [{"attempt_index": 0, "status": "failed"}],
            },
            "resource_usage": {
                "wall_time_ms": 100,
                "input_tokens": 10,
                "output_tokens": 5,
                "total_tokens": 15,
            },
        }
        recovery = {
            "recovery_plan_attempts": {
                "configured_recovery_runs": 1,
                "executed_recovery_runs": 1,
                "attempts": [
                    {"attempt_index": 0, "status": "failed_recoverable"},
                    {"attempt_index": 1, "status": "succeeded"},
                ],
            },
            "resource_usage": {
                "wall_time_ms": 250,
                "input_tokens": 30,
                "output_tokens": 15,
                "total_tokens": 45,
            },
        }
        before = {
            "snapshot_sha256": "a" * 64,
            "entries": [{"path": "app.py", "kind": "file", "sha256": "1", "size": 1}],
        }
        after = {
            "snapshot_sha256": "b" * 64,
            "entries": [
                {"path": "app.py", "kind": "file", "sha256": "2", "size": 2},
                {"path": "test.py", "kind": "file", "sha256": "3", "size": 3},
            ],
        }

        comparison = compare_recovery_arms(
            initial_only=initial,
            recovery_one=recovery,
            initial_oracles={"overall": "fail", "regression_status": "pass"},
            recovery_oracles={"overall": "pass", "regression_status": "fail"},
            initial_artifact_manifest=before,
            recovery_artifact_manifest=after,
        )

        self.assertEqual(comparison["quality_transition"], "improved")
        self.assertTrue(comparison["success_improved"])
        self.assertTrue(comparison["regression_introduced"])
        self.assertEqual(comparison["resource_delta"]["wall_time_ms"], 150)
        self.assertEqual(comparison["resource_delta"]["total_tokens"], 30)
        self.assertEqual(
            comparison["artifact_delta_between_arms"], artifact_delta(before, after)
        )

    def test_a14_comparison_does_not_attribute_zero_executed_recovery(self):
        comparison = compare_recovery_arms(
            initial_only={
                "recovery_plan_attempts": {
                    "configured_recovery_runs": 0,
                    "executed_recovery_runs": 0,
                    "attempts": [],
                },
                "resource_usage": {},
            },
            recovery_one={
                "recovery_plan_attempts": {
                    "configured_recovery_runs": 1,
                    "executed_recovery_runs": 0,
                    "attempts": [],
                },
                "resource_usage": {},
            },
            initial_oracles={"overall": "fail", "regression_status": "pass"},
            recovery_oracles={"overall": "pass", "regression_status": "pass"},
            initial_artifact_manifest={"entries": []},
            recovery_artifact_manifest={"entries": []},
        )

        self.assertEqual(comparison["quality_transition"], "initial_attempt_divergence")
        self.assertEqual(comparison["raw_oracle_transition"], "improved")
        self.assertFalse(comparison["success_improved"])

    def test_a14_recovery_contract_freezes_zero_vs_one_and_exclusions(self):
        contract = build_a14_recovery_contract(
            status="draft",
            code_sha="",
            exact_sha_ci_evidence="",
            live_collection_authorized=False,
        )

        paired = contract["paired_run_contract"]
        self.assertEqual(paired["initial_only"]["recovery_plan_auto_runs"], 0)
        self.assertEqual(paired["recovery_one"]["recovery_plan_auto_runs"], 1)
        self.assertEqual(paired["maximum_recovery_runs"], 1)
        self.assertEqual(paired["execution_action"], "ultra_plan_run")
        self.assertFalse(
            contract["recovery_eligibility"]["free_form_stderr_used_for_classification"]
        )
        eligibility = contract["recovery_eligibility"]["preregistered_smoke_cases"]
        self.assertEqual(
            eligibility["phase6-main-c06-task-01"]["category"],
            "dependency_or_provisioning",
        )
        self.assertFalse(contract["smoke"]["effect_claim_allowed"])
        self.assertEqual(contract["smoke"]["minimum_executed_recovery_pairs"], 1)
        self.assertEqual(
            contract["execution_root_policy"]["required_root"],
            "/Volumes/SSD_NX/tmp/commandagent_trial",
        )
        self.assertEqual(recovery_contract_errors(contract), [])
        invalid = copy.deepcopy(contract)
        invalid["paired_run_contract"]["recovery_one"]["recovery_plan_auto_runs"] = 2
        self.assertIn(
            "recovery_one_runs_must_be_one", recovery_contract_errors(invalid)
        )
        wrong_action = copy.deepcopy(contract)
        wrong_action["paired_run_contract"]["execution_action"] = "plan_run"
        self.assertIn(
            "smoke_execution_action_not_recovery_capable",
            recovery_contract_errors(wrong_action),
        )

    def test_a14_a2_contract_uses_shared_boundary_and_typed_oracles(self):
        contract = build_a14_a2_contract(
            status="draft",
            code_sha="",
            exact_sha_ci_evidence="",
            live_collection_authorized=False,
        )
        tasks = build_a14_a2_tasks()
        adapters = build_a14_a2_adapters()["adapters"]
        cli = next(
            row
            for row in tasks["cases"]
            if row["case_id"] == "phase6-main-c01-task-01"
        )["completion_contract"]
        self.assertEqual(
            contract["paired_run_contract"]["pairing_unit"],
            "shared_pre_recovery_snapshot",
        )
        self.assertGreaterEqual(len(contract["frozen_input_sha256"]), 6)
        self.assertFalse(
            contract["paired_run_contract"]["independent_workspace_copies"]
        )
        self.assertEqual(cli["required_obligations"], ["implementation"])
        self.assertEqual(cli["command_observations"][0]["expected_stdout"], "5\n")
        semantics = validate_a14_oracle_semantics(
            case_id="phase6-main-c07-task-01",
            intent="fix",
            adapters=adapters,
        )
        self.assertTrue(semantics["valid"], semantics)

        roles = {
            row["a14_role"]
            for row in adapters
            if row["case_id"] == "phase6-main-c07-task-01"
        }
        self.assertEqual(roles, {"precondition", "final_success"})
        for task_id in range(1, 11):
            case_id = f"phase6-main-c07-task-{task_id:02d}"
            self.assertTrue(
                validate_a14_oracle_semantics(
                    case_id=case_id,
                    intent="fix",
                    adapters=adapters,
                )["valid"],
                case_id,
            )
        c06 = [
            row for row in adapters if row["case_id"].startswith("phase6-main-c06-")
        ]
        self.assertTrue(c06)
        self.assertTrue(all(row["executor"].get("blocked_patterns") for row in c06))
        self.assertEqual(recovery_contract_errors(contract), [])
        missing_hashes = copy.deepcopy(contract)
        missing_hashes.pop("frozen_input_sha256")
        self.assertIn(
            "shared_boundary_frozen_input_hashes_missing",
            recovery_contract_errors(missing_hashes),
        )

    def test_a14_a3_scopes_recovery_semantics_away_from_excluded_pairs(self):
        contract = build_a14_a3_contract(
            status="draft",
            code_sha="",
            exact_sha_ci_evidence="",
            live_collection_authorized=False,
        )
        self.assertEqual(
            contract["analysis"]["oracle_semantics_validation_population"],
            "runtime_recovery_eligible_pairs_only",
        )
        self.assertEqual(
            contract["analysis"]["preregistered_exclusion_gate"],
            "ineligible_recovery_not_executed",
        )
        self.assertFalse(contract["authorization"]["smoke_collection_authorized"])

    def test_a14_a4_freezes_current_success_and_browser_safety_gates(self):
        contract = build_a14_a4_contract(
            status="draft",
            code_sha="",
            exact_sha_ci_evidence="",
            live_collection_authorized=False,
        )
        self.assertEqual(
            contract["recovery_safety_policy"]["maximum_automatic_recovery_runs"],
            1,
        )
        self.assertEqual(contract["smoke"]["minimum_executed_recovery_pairs"], 0)
        self.assertEqual(
            contract["smoke"]["minimum_current_success_suppressions"], 1
        )
        self.assertTrue(contract["smoke"]["require_browser_oracle_executability"])
        self.assertTrue(contract["smoke"]["require_isolated_treatment_workspace"])
        self.assertFalse(
            contract["smoke"]["require_executed_recovery_for_attribution"]
        )
        findings = contract["pre_live_amendments"][-1]["product_findings"]
        self.assertEqual(len(findings), 12)
        self.assertFalse(contract["authorization"]["smoke_collection_authorized"])
        self.assertEqual(recovery_contract_errors(contract), [])

    def test_a14_a4_report_accepts_protected_success_but_not_blocked_browser(self):
        contract = build_a14_a4_contract(
            status="draft",
            code_sha="",
            exact_sha_ci_evidence="",
            live_collection_authorized=False,
        )
        contract["smoke"]["expected_pair_count"] = 1
        usage = {
            "wall_time_ms": 100,
            "input_tokens": 10,
            "output_tokens": 5,
            "total_tokens": 15,
        }
        manifest = {
            "candidate_visibility_policy": (
                "commandagent.goal_verify.candidate_manifest.source_config_v1"
            )
        }
        external_oracles = {
            "source": "frozen_host_adapter_post_execution",
            "outcomes": [
                {
                    "adapter_id": "browser-golden",
                    "executor_kind": "playwright_script",
                    "outcome": {"executed": True, "result": "pass"},
                }
            ],
        }
        result = {
            "argv": ["commandagent", "--ultra-plan-run", "goal"],
            "recovery_plan_attempts": {
                "configured_recovery_runs": 1,
                "executed_recovery_runs": 0,
                "terminal_stop_reason": "current_success_protected",
            },
            "resource_usage": usage,
        }
        record = {
            "pair_id": "protected-current-success",
            "pairing_unit": "shared_pre_recovery_snapshot",
            "eligibility": {
                "runtime": {
                    "run_recovery_one_arm": True,
                    "category": "initial_success",
                }
            },
            "initial_only": {
                "input_manifest": {"snapshot_sha256": "a" * 64},
                "result": {
                    **result,
                    "recovery_plan_attempts": {
                        "configured_recovery_runs": 0,
                        "executed_recovery_runs": 0,
                    },
                },
                "output_artifact_manifest": manifest,
                "external_oracles": external_oracles,
            },
            "recovery_one": {
                "status": "completed",
                "input_manifest": {"snapshot_sha256": "a" * 64},
                "result": result,
                "output_artifact_manifest": manifest,
                "external_oracles": external_oracles,
            },
            "comparison": {
                "quality_transition": "no_recovery_needed",
                "success_improved": False,
                "shared_initial_history": True,
                "effect_attribution_ready": False,
                "oracle_semantics": {"valid": True},
                "internal_external_outcome_matrix": {},
                "resource_delta": {
                    "wall_time_ms": 0,
                    "input_tokens": 0,
                    "output_tokens": 0,
                    "total_tokens": 0,
                },
            },
        }

        report = build_recovery_report(records=[record], contract=contract)

        self.assertTrue(report["instrument_ready"], report["diagnostics"])
        self.assertTrue(report["checks"]["current_success_suppression_observed"])
        self.assertTrue(report["checks"]["browser_oracle_executability_preflight"])
        self.assertFalse(report["effect_attribution_ready"])

        blocked = copy.deepcopy(record)
        blocked["recovery_one"]["external_oracles"]["outcomes"][0]["outcome"] = {
            "executed": False,
            "result": "blocked",
            "reason": "playwright-core unavailable",
        }
        blocked_report = build_recovery_report(records=[blocked], contract=contract)
        self.assertFalse(blocked_report["instrument_ready"])
        self.assertFalse(
            blocked_report["checks"]["browser_oracle_executability_preflight"]
        )

    def test_a14_a5_separates_reference_preflight_from_candidate_failure(self):
        contract = build_a14_a5_contract(
            status="draft",
            code_sha="",
            exact_sha_ci_evidence="",
            live_collection_authorized=False,
        )
        self.assertTrue(
            contract["smoke"]["require_separate_browser_oracle_preflight"]
        )
        self.assertEqual(
            contract["analysis"]["browser_executability_preflight_source"],
            "frozen reference workspace, never candidate artifact",
        )
        self.assertEqual(recovery_contract_errors(contract), [])
        self.assertEqual(
            contract["frozen_external_oracles"],
            "eval/goal_verify/v0/phase6-command-adapters-v4-a14-a5.json",
        )
        authorized_contract = build_a14_a5_contract(
            status="frozen",
            code_sha="a" * 40,
            exact_sha_ci_evidence="exact-sha.json",
            live_collection_authorized=True,
        )
        self.assertEqual(
            authorized_contract["authorization"]["approved_at"], "2026-08-30"
        )

        contract["smoke"]["expected_pair_count"] = 1
        usage = {
            "wall_time_ms": 1,
            "input_tokens": 1,
            "output_tokens": 1,
            "total_tokens": 2,
        }
        manifest = {
            "candidate_visibility_policy": (
                "commandagent.goal_verify.candidate_manifest.source_config_v1"
            )
        }
        candidate_browser_failure = {
            "source": "frozen_host_adapter_post_execution",
            "outcomes": [
                {
                    "adapter_id": "candidate-browser",
                    "executor_kind": "playwright_script",
                    "outcome": {
                        "executed": False,
                        "result": "blocked",
                        "reason": "candidate_server_not_ready",
                    },
                }
            ],
        }
        record = {
            "pair_id": "runtime-excluded-after-preregistration",
            "pairing_unit": "shared_pre_recovery_snapshot",
            "eligibility": {
                "preregistered": {"eligible": True},
                "runtime": {
                    "run_recovery_one_arm": False,
                    "category": "dependency_or_provisioning",
                },
            },
            "initial_only": {
                "input_manifest": {"snapshot_sha256": "a" * 64},
                "result": {
                    "argv": ["commandagent", "--ultra-plan-run", "goal"],
                    "recovery_plan_attempts": {
                        "configured_recovery_runs": 0,
                        "executed_recovery_runs": 0,
                    },
                    "resource_usage": usage,
                },
                "output_artifact_manifest": manifest,
                "external_oracles": candidate_browser_failure,
            },
            "recovery_one": {
                "status": "completed",
                "input_manifest": {"snapshot_sha256": "a" * 64},
                "result": {
                    "argv": ["commandagent", "--ultra-plan-run", "goal"],
                    "recovery_plan_attempts": {
                        "configured_recovery_runs": 1,
                        "executed_recovery_runs": 0,
                    },
                    "resource_usage": usage,
                },
                "output_artifact_manifest": manifest,
                "external_oracles": candidate_browser_failure,
            },
            "comparison": {
                "quality_transition": "no_recovery_executed",
                "success_improved": False,
                "shared_initial_history": True,
                "internal_external_outcome_matrix": {},
                "resource_delta": {
                    "wall_time_ms": 0,
                    "input_tokens": 0,
                    "output_tokens": 0,
                    "total_tokens": 0,
                },
            },
        }
        preflight = {
            "contract_id": contract["contract_id"],
            "run_id": contract["smoke_run_id"],
            "source": "frozen_reference_workspace",
            "passed_to_product_or_recovery": False,
            "outcomes": [
                {
                    "adapter_id": "reference-browser",
                    "candidate_visible": False,
                    "build": {
                        "outcome": {
                            "executed": True,
                            "result": "pass",
                            "reason": "registered_reference_build_passed",
                        }
                    },
                    "outcome": {
                        "executed": True,
                        "result": "pass",
                        "reason": "observation_match",
                    },
                }
            ],
        }

        report = build_recovery_report(
            records=[record],
            contract=contract,
            oracle_executability_preflight=preflight,
        )

        self.assertTrue(
            report["checks"][
                "recovery_arm_configured_one_or_preregistered_not_run"
            ]
        )
        self.assertTrue(report["checks"]["maximum_one_recovery_executed"])
        self.assertTrue(report["checks"]["ineligible_recovery_not_executed"])
        self.assertTrue(
            report["checks"]["browser_oracle_executability_preflight"],
            report["diagnostics"],
        )

        wrong_run = copy.deepcopy(preflight)
        wrong_run["run_id"] = "different-run"
        wrong_run_report = build_recovery_report(
            records=[record],
            contract=contract,
            oracle_executability_preflight=wrong_run,
        )
        self.assertFalse(
            wrong_run_report["checks"]["browser_oracle_executability_preflight"]
        )

        too_many = copy.deepcopy(record)
        too_many["recovery_one"]["result"]["recovery_plan_attempts"][
            "executed_recovery_runs"
        ] = 2
        too_many_report = build_recovery_report(
            records=[too_many],
            contract=contract,
            oracle_executability_preflight=preflight,
        )
        self.assertTrue(
            too_many_report["checks"][
                "recovery_arm_configured_one_or_preregistered_not_run"
            ]
        )
        self.assertFalse(
            too_many_report["checks"]["maximum_one_recovery_executed"]
        )

    def test_a14_a6_types_c05_reproducer_and_freezes_cli_only_smoke(self):
        tasks = build_a14_a6_tasks()
        self.assertEqual(task_contract_registry_errors(tasks), [])
        task = next(
            row
            for row in tasks["cases"]
            if row["case_id"] == "phase6-main-c05-task-01"
        )
        self.assertEqual(
            task["completion_contract"]["fix_reproducer_command"],
            "python3 cli.py 7",
        )
        corpus = load("eval/goal_verify/v0/phase6-main-corpus-v4.json")
        for case_id, command in (
            ("phase6-main-c05-task-01", "python3 cli.py 7"),
            ("phase6-main-c05-task-05", "python3 cli.py 11"),
            ("phase6-main-c05-task-10", "python3 cli.py 16"),
        ):
            source = next(
                row for row in corpus["cases"] if row["case_id"] == case_id
            )
            bound = bind_task_contract(source, tasks)
            self.assertEqual(bound["goal"], source["goal"])
            self.assertEqual(
                bound["task_contract"]["completion_contract"][
                    "fix_reproducer_command"
                ],
                command,
            )
        adapters = build_a14_a6_adapters()["adapters"]
        before = next(
            row
            for row in adapters
            if row["adapter_id"].startswith("before-after-before--phase6-cell-05")
        )
        after = next(
            row
            for row in adapters
            if row["adapter_id"].startswith("before-after-after--phase6-cell-05")
        )
        self.assertEqual(before["a14_role"], "precondition")
        self.assertEqual(after["a14_role"], "final_success")
        self.assertEqual(before["executor"]["kind"], "fixture_hash_command")
        self.assertEqual(
            before["executor"]["registered_fixture"],
            after["executor"]["registered_fixture"],
        )
        contract = build_a14_a6_contract(
            status="draft",
            code_sha="",
            exact_sha_ci_evidence="",
            live_collection_authorized=False,
        )
        self.assertEqual(recovery_contract_errors(contract), [])
        self.assertFalse(contract["smoke"]["require_browser_oracle_executability"])
        self.assertEqual(contract["smoke"]["expected_pair_count"], 3)
        self.assertEqual(
            contract["smoke"]["typed_fix_reproducer_commands"][
                "phase6-main-c05-task-10--pair-01"
            ],
            "python3 cli.py 16",
        )

    def test_a14_a7_requires_a_naturally_executed_recovery_pair(self):
        contract = build_a14_a7_contract(
            status="draft",
            code_sha="",
            exact_sha_ci_evidence="",
            live_collection_authorized=False,
        )

        self.assertEqual(recovery_contract_errors(contract), [])
        self.assertEqual(
            contract["task_contract_registry"],
            "eval/goal_verify/v0/phase6-task-contracts-v4-a14-a6-1.json",
        )
        self.assertEqual(contract["smoke"]["minimum_executed_recovery_pairs"], 1)
        self.assertTrue(contract["smoke"]["require_executed_recovery_for_attribution"])
        self.assertTrue(
            contract["analysis"]["smoke_readiness_requires_live_recovery_execution"]
        )
        self.assertEqual(
            contract["analysis"]["unsupported_required_capability_policy"],
            "fail_closed",
        )

    def test_a14_a8_freezes_treatment_owned_contract_binding(self):
        contract = build_a14_a8_contract(
            status="draft",
            code_sha="",
            exact_sha_ci_evidence="",
            live_collection_authorized=False,
        )

        self.assertEqual(recovery_contract_errors(contract), [])
        self.assertEqual(
            contract["supersedes_contract"],
            "phase6-recovery-v4-20260830-a14-a7-live-01",
        )
        self.assertEqual(contract["smoke"]["minimum_executed_recovery_pairs"], 1)
        self.assertEqual(
            contract["analysis"]["treatment_completion_contract_binding"],
            "exact bytes copied by host to "
            ".commandagent/recovery-runtime/completion-contract.json",
        )
        self.assertFalse(
            contract["analysis"]["treatment_contract_source_in_promotion_manifest"]
        )

    def test_a14_a9_requires_registered_recovery_verify_commands(self):
        contract = build_a14_a9_contract(
            status="draft",
            code_sha="",
            exact_sha_ci_evidence="",
            live_collection_authorized=False,
        )

        self.assertEqual(recovery_contract_errors(contract), [])
        self.assertEqual(
            contract["supersedes_contract"],
            "phase6-recovery-v4-20260830-a14-a8-live-01",
        )
        self.assertTrue(
            contract["smoke"]["require_registered_recovery_verify_commands"]
        )
        self.assertIn(
            "registered_recovery_verify_commands",
            contract["smoke"]["required_readiness_checks"],
        )
        self.assertEqual(
            contract["analysis"]["recovery_verify_command_authority"],
            "CompletionContract.verify_commands",
        )

    def test_a14_a10_requires_registered_inner_recovery_verify_commands(self):
        contract = build_a14_a10_contract(
            status="draft",
            code_sha="",
            exact_sha_ci_evidence="",
            live_collection_authorized=False,
        )

        self.assertEqual(recovery_contract_errors(contract), [])
        self.assertEqual(
            contract["supersedes_contract"],
            "phase6-recovery-v4-20260830-a14-a9-live-01",
        )
        self.assertTrue(
            contract["smoke"][
                "require_registered_inner_recovery_verify_commands"
            ]
        )
        self.assertIn(
            "registered_inner_recovery_verify_commands",
            contract["smoke"]["required_readiness_checks"],
        )
        self.assertEqual(
            contract["analysis"]["pytest_evidence_policy"],
            "classify pytest invocation as Test only when a test artifact exists",
        )

    def test_a14_a11_requires_fix_contract_continuity(self):
        contract = build_a14_a11_contract(
            status="draft",
            code_sha="",
            exact_sha_ci_evidence="",
            live_collection_authorized=False,
        )

        self.assertEqual(recovery_contract_errors(contract), [])
        self.assertEqual(
            contract["supersedes_contract"],
            "phase6-recovery-v4-20260830-a14-a10-live-01",
        )
        self.assertTrue(contract["smoke"]["require_fix_contract_continuity"])
        self.assertIn(
            "fix_contract_continuity",
            contract["smoke"]["required_readiness_checks"],
        )
        self.assertIn(
            "fix-origin.json", contract["analysis"]["recovery_fix_origin_source"]
        )

    def test_a14_a12_requires_recovery_fix_terminal_completion(self):
        contract = build_a14_a12_contract(
            status="draft",
            code_sha="",
            exact_sha_ci_evidence="",
            live_collection_authorized=False,
        )

        self.assertEqual(recovery_contract_errors(contract), [])
        self.assertEqual(
            contract["supersedes_contract"],
            "phase6-recovery-v4-20260830-a14-a11-live-01",
        )
        self.assertTrue(
            contract["smoke"]["require_recovery_fix_terminal_completion"]
        )
        self.assertIn(
            "recovery_fix_terminal_completion",
            contract["smoke"]["required_readiness_checks"],
        )
        self.assertIn(
            "AcceptancePassed",
            contract["analysis"]["recovery_fix_phase_completion_policy"],
        )

    def test_a14_a13_types_three_fix_profiles_and_repeats_pairs(self):
        tasks = build_a14_a13_tasks()
        adapters = build_a14_a13_adapters()
        contract = build_a14_a13_contract(
            status="draft",
            code_sha="",
            exact_sha_ci_evidence="",
            live_collection_authorized=False,
        )

        self.assertEqual(task_contract_registry_errors(tasks), [])
        self.assertEqual(recovery_contract_errors(contract), [])
        task_by_id = {row["case_id"]: row for row in tasks["cases"]}
        for case_id in (
            "phase6-main-c05-task-05",
            "phase6-main-c07-task-01",
            "phase6-main-c08-task-01",
        ):
            completion = task_by_id[case_id]["completion_contract"]
            self.assertEqual(
                completion["fix_reproducer_command"],
                completion["verify_commands"][0],
            )
        before = [
            row
            for row in adapters["adapters"]
            if row.get("case_id") == "phase6-main-c08-task-01"
            and row.get("a14_role") == "precondition"
        ]
        self.assertEqual(len(before), 1)
        self.assertEqual(before[0]["executor"]["stage"], "before")
        self.assertEqual(before[0]["executor"]["observation"]["expected"], 1)
        self.assertTrue(
            validate_a14_oracle_semantics(
                case_id="phase6-main-c08-task-01",
                intent="fix",
                adapters=adapters["adapters"],
            )["valid"]
        )
        self.assertEqual(contract["smoke"]["expected_pair_count"], 10)
        self.assertIn(
            "phase6-main-c05-task-05--pair-03",
            contract["smoke"]["selected_pair_ids"],
        )
        self.assertEqual(
            len(contract["smoke"]["typed_fix_reproducer_commands"]), 9
        )
        self.assertFalse(
            contract["recovery_eligibility"]["preregistered_smoke_cases"][
                "phase6-main-c06-task-01"
            ]["eligible"]
        )

    def test_recovery_pair_id_parser_preserves_replicate(self):
        self.assertEqual(
            parse_recovery_pair_id("phase6-main-c05-task-05--pair-03"),
            ("phase6-main-c05-task-05", 3, "pair-03.json"),
        )
        for invalid in (
            "phase6-main-c05-task-05--pair-00",
            "phase6-main-c05-task-05--pair-1",
            "phase6-main-c05-task-05",
        ):
            with self.assertRaises(ValueError):
                parse_recovery_pair_id(invalid)

    def test_a14_a14_freezes_full_cluster_design_and_four_budgets(self):
        contract = build_a14_a14_contract(
            status="draft",
            code_sha="",
            exact_sha_ci_evidence="",
            live_collection_authorized=False,
        )

        self.assertEqual(recovery_contract_errors(contract), [])
        full = contract["full_experiment"]
        self.assertEqual(full["eligible_pair_count"], 60)
        self.assertEqual(full["sentinel_pair_count"], 20)
        self.assertEqual(full["eligible_cell_ids"], ["cell-05", "cell-07"])
        self.assertTrue(
            all("c08" not in pair_id for pair_id in full["eligible_pair_ids"])
        )
        self.assertTrue(
            any("c08" in pair_id for pair_id in full["sentinel_pair_ids"])
        )
        self.assertEqual(full["pairs_per_eligible_cluster"], 3)
        self.assertEqual(full["minimum_executed_recovery_pairs"], 30)
        self.assertEqual(full["bootstrap_samples"], 2000)
        self.assertEqual(
            full["resource_budgets"],
            {
                "wall_time_ms": {"p50": 240000, "p95": 600000},
                "total_tokens": {"p50": 60000, "p95": 120000},
            },
        )
        self.assertFalse(contract["authorization"]["full_collection_authorized"])

    def test_a14_a13_1_prebinds_every_selected_task(self):
        corpus = load("eval/goal_verify/v0/phase6-main-corpus-v4.json")
        corpus_by_id = {row["case_id"]: row for row in corpus["cases"]}
        tasks = build_a14_a13_1_tasks()
        contract = build_a14_a13_1_contract(
            status="draft",
            code_sha="",
            exact_sha_ci_evidence="",
            live_collection_authorized=False,
        )

        self.assertEqual(task_contract_registry_errors(tasks), [])
        self.assertEqual(recovery_contract_errors(contract), [])
        with self.assertRaisesRegex(
            ValueError, "completion contract fix reproducer mismatch"
        ):
            bind_selected_recovery_cases(
                selected_pair_ids=contract["smoke"]["selected_pair_ids"],
                corpus_by_id=corpus_by_id,
                task_registry=build_a14_a13_tasks(),
            )
        bound = bind_selected_recovery_cases(
            selected_pair_ids=contract["smoke"]["selected_pair_ids"],
            corpus_by_id=corpus_by_id,
            task_registry=tasks,
        )
        self.assertEqual(
            set(bound),
            {
                "phase6-main-c05-task-05",
                "phase6-main-c06-task-01",
                "phase6-main-c07-task-01",
                "phase6-main-c08-task-01",
            },
        )
        for case_id in ("phase6-main-c07-task-01", "phase6-main-c08-task-01"):
            completion = bound[case_id]["task_contract"]["completion_contract"]
            constraints = bound[case_id]["task_contract"][
                "operational_constraints"
            ]
            self.assertEqual(
                completion["fix_reproducer_command"],
                shlex.join(constraints["reproducer"]["argv"]),
            )

    def test_a14_a13_2_excludes_explicit_profile_contract_conflict(self):
        contract = build_a14_a13_2_contract(
            status="draft",
            code_sha="",
            exact_sha_ci_evidence="",
            live_collection_authorized=False,
        )

        self.assertEqual(recovery_contract_errors(contract), [])
        eligibility = contract["recovery_eligibility"][
            "preregistered_smoke_cases"
        ]
        self.assertTrue(eligibility["phase6-main-c05-task-05"]["eligible"])
        self.assertTrue(eligibility["phase6-main-c07-task-01"]["eligible"])
        self.assertEqual(
            eligibility["phase6-main-c06-task-01"]["category"],
            "dependency_or_provisioning",
        )
        self.assertEqual(
            eligibility["phase6-main-c08-task-01"],
            {
                "eligible": False,
                "category": "profile_or_completion_contract",
                "reason": "product_profile_explicitly_forbidden_by_task_contract",
                "policy_id": "commandagent.goal_verify.recovery_eligibility.v4_a14",
            },
        )

    def test_a14_a13_3_preregisters_completion_safe_recovery(self):
        contract = build_a14_a13_3_contract(
            status="draft",
            code_sha="",
            exact_sha_ci_evidence="",
            live_collection_authorized=False,
        )

        self.assertEqual(recovery_contract_errors(contract), [])
        self.assertEqual(
            contract["pre_live_amendments"][-1]["amendment_id"],
            "v4-A14-A13-3",
        )
        self.assertEqual(
            contract["supersedes_smoke_run"],
            "phase6-recovery-v4-20260830-a14-a13-2-smoke-01",
        )
        self.assertEqual(
            contract["analysis"]["recovery_promotion_policy"],
            "promote only after the registered final-success observation and "
            "the remaining product-visible completion contract both pass",
        )
        self.assertTrue(
            contract["smoke"]["require_recovery_fix_terminal_completion"]
        )
        self.assertFalse(
            contract["authorization"]["smoke_collection_authorized"]
        )

    def test_registered_browser_process_records_execution(self):
        with tempfile.TemporaryDirectory() as temporary:
            outcome = _run_registered_browser_command(
                ["node", "-e", "process.stdout.write('ok')"],
                Path(temporary),
                1_000,
            )

        self.assertTrue(outcome["executed"])
        self.assertEqual(outcome["exit_code"], 0)

    def test_reference_browser_preflight_builds_registered_stage_once(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            root = base / "root"
            reference = root / "fixture/reference"
            next_package = reference / "node_modules/next/package.json"
            next_package.parent.mkdir(parents=True)
            next_package.write_text('{"version":"16.3.1"}', encoding="utf-8")
            (reference / "package.json").write_text("{}", encoding="utf-8")
            run_dir = base / "run"
            run_dir.mkdir()
            build_calls = []

            def build_runner(argv, workspace, timeout_ms):
                build_calls.append((argv, workspace, timeout_ms))
                return {
                    "executed": True,
                    "exit_code": 0,
                    "timed_out": False,
                }

            def oracle_executor(executor, *, workspace):
                self.assertEqual(executor["kind"], "playwright_script")
                self.assertTrue((workspace / "package.json").is_file())
                return {
                    "executed": True,
                    "result": "pass",
                    "reason": "observation_match",
                }

            document = _ensure_oracle_executability_preflight(
                contract={
                    "contract_id": "a14-a5-test",
                    "smoke": {"require_separate_browser_oracle_preflight": True},
                },
                adapters=[
                    {
                        "adapter_id": "browser-reference",
                        "case_id": "case-1",
                        "executor": {
                            "kind": "playwright_script",
                            "workspace": "workspace-1",
                            "stage": "reference",
                        },
                    }
                ],
                workspaces={
                    "workspace-1": {
                        "case_id": "workspace-1",
                        "profile": "nextjs",
                        "root": "fixture",
                        "stages": {"reference": "golden"},
                    }
                },
                root=root,
                execution_root=base / "execution",
                namespace="a14-a5-smoke",
                selected_pair_ids=["case-1--pair-01"],
                run_dir=run_dir,
                build_runner=build_runner,
                oracle_executor=oracle_executor,
            )

        self.assertEqual(len(build_calls), 1)
        self.assertEqual(build_calls[0][0], ["npx", "next", "build", "--webpack"])
        self.assertEqual(document["source"], "frozen_reference_workspace")
        self.assertFalse(document["passed_to_product_or_recovery"])
        self.assertTrue(document["outcomes"][0]["outcome"]["executed"])

    def test_shared_boundary_comparison_requires_semantic_oracle_validation(self):
        treatment = {
            "recovery_plan_attempts": {"executed_recovery_runs": 1},
            "recovery_boundary": {
                "recovery_provider_usage": {
                    "wall_time_ms": 20,
                    "input_tokens": 10,
                    "output_tokens": 2,
                    "total_tokens": 12,
                }
            },
            "terminal_status": {"status": "completed"},
        }
        before = {
            "entries": [{"path": "app.py", "kind": "file", "sha256": "a"}]
        }
        after = {
            "entries": [{"path": "app.py", "kind": "file", "sha256": "b"}]
        }
        blocked = compare_shared_recovery_boundary(
            treatment=treatment,
            control_oracles={"overall": "fail", "regression_status": "pass"},
            treatment_oracles={"overall": "pass", "regression_status": "pass"},
            control_artifact_manifest=before,
            treatment_artifact_manifest=after,
            control_snapshot_matches_boundary=True,
            oracle_semantics={"valid": False},
            precondition_oracles={"overall": "pass"},
        )
        self.assertEqual(blocked["quality_transition"], "unattributed")
        self.assertFalse(blocked["success_improved"])
        attributed = compare_shared_recovery_boundary(
            treatment=treatment,
            control_oracles={"overall": "fail", "regression_status": "pass"},
            treatment_oracles={"overall": "pass", "regression_status": "pass"},
            control_artifact_manifest=before,
            treatment_artifact_manifest=after,
            control_snapshot_matches_boundary=True,
            oracle_semantics={"valid": True},
            precondition_oracles={"overall": "pass"},
        )
        self.assertEqual(attributed["quality_transition"], "improved")
        self.assertTrue(attributed["success_improved"])
        self.assertEqual(attributed["resource_delta"]["total_tokens"], 12)

    def test_a14_pair_runs_explicit_zero_then_one_and_scores_external_oracle(self):
        contract = build_a14_recovery_contract(
            status="draft",
            code_sha="",
            exact_sha_ci_evidence="",
            live_collection_authorized=False,
        )
        corpus = load("eval/goal_verify/v0/phase6-main-corpus-v4.json")
        source = next(
            row
            for row in corpus["cases"]
            if row["case_id"] == "phase6-main-c01-task-01"
        )
        task_registry = load_task_contract_registry(
            ROOT / contract["task_contract_registry"]
        )
        case = bind_task_contract(source, task_registry)
        tasks = {row["case_id"]: row for row in task_registry["cases"]}
        adapters = load(contract["frozen_external_oracles"])["adapters"]
        workspaces = workspace_by_case(
            load_v4_workspace_registry(root=ROOT, contract=contract)
        )
        configured = []

        def baseline_runner(**kwargs):
            configured_runs = kwargs["recovery_plan_auto_runs"]
            configured.append((configured_runs, kwargs["execution_action"]))
            output = kwargs["workspace"] / "cli/main.py"
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_text(
                "print('good')\n" if configured_runs == 1 else "print('bad')\n",
                encoding="utf-8",
            )
            return {
                "status": "completed" if configured_runs == 1 else "failed",
                "terminal_status": {
                    "recorded": True,
                    "ok": configured_runs == 1,
                    "failure_kind": (
                        None
                        if configured_runs == 1
                        else "verify_repair_progress_unchanged"
                    ),
                    "recovery_ultra_plan_path": (
                        None if configured_runs == 1 else ".anvil/plans/recovery.yaml"
                    ),
                },
                "recovery_plan_attempts": {
                    "configured_recovery_runs": configured_runs,
                    "executed_recovery_runs": configured_runs,
                    "attempts": [
                        {
                            "attempt_index": configured_runs,
                            "status": (
                                "succeeded" if configured_runs == 1 else "failed"
                            ),
                        }
                    ],
                },
                "resource_usage": {
                    "wall_time_ms": 100 + configured_runs * 50,
                    "input_tokens": 10 + configured_runs * 5,
                    "output_tokens": 5 + configured_runs * 2,
                    "total_tokens": 15 + configured_runs * 7,
                },
            }

        def oracle_executor(spec, *, workspace):
            del spec
            content = (workspace / "cli/main.py").read_text(encoding="utf-8")
            return {
                "executed": True,
                "result": "pass" if "good" in content else "fail",
            }

        with tempfile.TemporaryDirectory() as temporary:
            execution_root = Path(temporary)
            record = run_recovery_pair(
                root=ROOT,
                contract=contract,
                case=case,
                task_contract=tasks[case["case_id"]],
                workspace_contract=workspaces[case["workspace_case_id"]],
                pair_id=f"{case['case_id']}--pair-01",
                execution_root=execution_root,
                namespace="a14-test",
                commandagent_bin=ROOT / "target/release/commandagent",
                adapters=adapters,
                baseline_runner=baseline_runner,
                oracle_executor=oracle_executor,
            )

        self.assertEqual(
            configured,
            [(0, "ultra_plan_run"), (1, "ultra_plan_run")],
        )
        self.assertEqual(record["comparison"]["quality_transition"], "improved")
        self.assertEqual(
            record["initial_only"]["input_manifest"]["snapshot_sha256"],
            record["recovery_one"]["input_manifest"]["snapshot_sha256"],
        )
        self.assertEqual(record["comparison"]["executed_recovery_runs"], 1)

    def test_a14_a2_pair_uses_one_product_run_and_shared_pre_recovery_snapshot(self):
        contract = build_a14_a2_contract(
            status="draft",
            code_sha="",
            exact_sha_ci_evidence="",
            live_collection_authorized=False,
        )
        corpus = load("eval/goal_verify/v0/phase6-main-corpus-v4.json")
        source = next(
            row
            for row in corpus["cases"]
            if row["case_id"] == "phase6-main-c07-task-01"
        )
        task_registry = build_a14_a2_tasks()
        case = bind_task_contract(source, task_registry)
        tasks = {row["case_id"]: row for row in task_registry["cases"]}
        adapters = build_a14_a2_adapters()["adapters"]
        workspaces = workspace_by_case(
            load_v4_workspace_registry(root=ROOT, contract=contract)
        )
        calls = []

        def baseline_runner(**kwargs):
            calls.append(
                (
                    kwargs["recovery_plan_auto_runs"],
                    kwargs["capture_recovery_boundary"],
                )
            )
            workspace = kwargs["workspace"]
            relative = Path(
                ".commandagent/recovery-boundaries/attempt-1/workspace"
            )
            boundary = workspace / relative
            boundary.mkdir(parents=True)
            shutil.copy2(workspace / "app.py", boundary / "app.py")
            shutil.copytree(workspace / "fixture", boundary / "fixture")
            snapshot_sha256 = _snapshot_content_sha256(boundary)
            (workspace / "app.py").write_text("print('fixed')\n", encoding="utf-8")
            return {
                "status": "completed",
                "argv": ["commandagent", "--ultra-plan-run", "goal"],
                "terminal_status": {"recorded": True, "ok": True, "status": "completed"},
                "recovery_plan_attempts": {
                    "configured_recovery_runs": 1,
                    "executed_recovery_runs": 1,
                    "attempts": [
                        {
                            "attempt_index": 0,
                            "kind": "initial",
                            "status": "failed_recoverable",
                        },
                        {
                            "attempt_index": 1,
                            "kind": "recovery",
                            "status": "succeeded",
                            "recovery_handoff_kind": "verification_failed",
                        },
                    ],
                },
                "resource_usage": {
                    "wall_time_ms": 100,
                    "input_tokens": 30,
                    "output_tokens": 5,
                    "total_tokens": 35,
                },
                "recovery_boundary": {
                    "status": "captured",
                    "workspace_relative_path": relative.as_posix(),
                    "file_count": 3,
                    "total_bytes": 100,
                    "snapshot_sha256": snapshot_sha256,
                    "initial_provider_usage": {
                        "wall_time_ms": 70,
                        "input_tokens": 20,
                        "output_tokens": 3,
                        "total_tokens": 23,
                    },
                    "recovery_provider_usage": {
                        "wall_time_ms": 30,
                        "input_tokens": 10,
                        "output_tokens": 2,
                        "total_tokens": 12,
                    },
                },
            }

        def oracle_executor(spec, *, workspace):
            if spec["kind"] == "file_content":
                present = spec["pattern"] in (workspace / spec["path"]).read_text(
                    encoding="utf-8"
                )
                passed = not present if spec["polarity"] == "absent" else present
            else:
                fixed = "fixed" in (workspace / "app.py").read_text(encoding="utf-8")
                passed = fixed if spec["observation"]["expected"] == 0 else not fixed
            return {"executed": True, "result": "pass" if passed else "fail"}

        with tempfile.TemporaryDirectory() as temporary:
            record = run_recovery_pair(
                root=ROOT,
                contract=contract,
                case=case,
                task_contract=tasks[case["case_id"]],
                workspace_contract=workspaces[case["workspace_case_id"]],
                pair_id=f"{case['case_id']}--pair-01",
                execution_root=Path(temporary),
                namespace="a14-a2-test",
                commandagent_bin=ROOT / "target/release/commandagent",
                adapters=adapters,
                baseline_runner=baseline_runner,
                oracle_executor=oracle_executor,
            )

        self.assertEqual(calls, [(1, True)])
        self.assertEqual(record["initial_only"]["status"], "captured_pre_recovery_control")
        self.assertEqual(record["comparison"]["quality_transition"], "improved")
        self.assertTrue(record["comparison"]["effect_attribution_ready"])
        self.assertTrue(record["comparison"]["control_snapshot_matches_boundary"])
        self.assertEqual(record["comparison"]["resource_delta"]["total_tokens"], 12)
        contract["smoke"]["expected_pair_count"] = 1
        report = build_recovery_report(records=[record], contract=contract)
        self.assertTrue(report["instrument_ready"], report["diagnostics"])
        self.assertTrue(report["effect_attribution_ready"])

        excluded = copy.deepcopy(record)
        excluded["pair_id"] = "preregistered-dependency-exclusion"
        excluded["eligibility"]["runtime"] = {
            "run_recovery_one_arm": False,
            "category": "dependency_or_provisioning",
        }
        excluded["recovery_one"]["result"]["recovery_plan_attempts"] = {
            "configured_recovery_runs": 0,
            "executed_recovery_runs": 0,
        }
        excluded["comparison"]["oracle_semantics"] = {
            "valid": False,
            "errors": ["fix_precondition_oracle_missing"],
        }
        excluded["comparison"]["effect_attribution_ready"] = False
        excluded["comparison"]["quality_transition"] = "no_recovery_executed"
        contract["smoke"]["expected_pair_count"] = 2
        report_with_exclusion = build_recovery_report(
            records=[record, excluded], contract=contract
        )
        self.assertTrue(
            report_with_exclusion["checks"][
                "final_success_oracle_semantics_validated"
            ]
        )
        self.assertTrue(
            report_with_exclusion["checks"][
                "fix_before_and_after_polarity_distinct"
            ]
        )
        self.assertTrue(
            report_with_exclusion["checks"]["ineligible_recovery_not_executed"]
        )

    def test_recovery_snapshot_hash_matches_product_fixed_vector(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "a.txt").write_text("x\n", encoding="utf-8")
            self.assertEqual(
                _snapshot_content_sha256(root),
                "fca205d27f585d85835e310c03faf89448c6707cc889e2fda18085ae527122bf",
            )

    def test_a14_pair_does_not_run_recovery_for_declared_missing_dependency(self):
        contract = build_a14_recovery_contract(
            status="draft",
            code_sha="",
            exact_sha_ci_evidence="",
            live_collection_authorized=False,
        )
        corpus = load("eval/goal_verify/v0/phase6-main-corpus-v4.json")
        source = next(
            row
            for row in corpus["cases"]
            if row["case_id"] == "phase6-main-c06-task-01"
        )
        task_registry = load_task_contract_registry(
            ROOT / contract["task_contract_registry"]
        )
        case = bind_task_contract(source, task_registry)
        tasks = {row["case_id"]: row for row in task_registry["cases"]}
        adapters = load(contract["frozen_external_oracles"])["adapters"]
        workspaces = workspace_by_case(
            load_v4_workspace_registry(root=ROOT, contract=contract)
        )
        configured = []

        def baseline_runner(**kwargs):
            configured.append(kwargs["recovery_plan_auto_runs"])
            return {
                "status": "failed",
                "terminal_status": {
                    "recorded": True,
                    "ok": False,
                    "failure_kind": "dependency_setup_blocked_offline",
                    "recovery_ultra_plan_path": ".anvil/plans/recovery.yaml",
                },
                "recovery_plan_attempts": {
                    "configured_recovery_runs": 0,
                    "executed_recovery_runs": 0,
                    "attempts": [],
                },
                "resource_usage": {},
            }

        def oracle_executor(spec, *, workspace):
            del spec, workspace
            return {"executed": False, "result": "unverified"}

        with tempfile.TemporaryDirectory() as temporary:
            record = run_recovery_pair(
                root=ROOT,
                contract=contract,
                case=case,
                task_contract=tasks[case["case_id"]],
                workspace_contract=workspaces[case["workspace_case_id"]],
                pair_id=f"{case['case_id']}--pair-01",
                execution_root=Path(temporary),
                namespace="a14-test",
                commandagent_bin=ROOT / "target/release/commandagent",
                adapters=adapters,
                baseline_runner=baseline_runner,
                oracle_executor=oracle_executor,
            )

        self.assertEqual(configured, [0])
        self.assertEqual(record["recovery_one"]["status"], "not_run")
        self.assertEqual(
            record["eligibility"]["runtime"]["runtime_source"],
            "preregistered_case_policy",
        )

    def test_a14_report_separates_attributed_effect_from_excluded_pair(self):
        contract = build_a14_recovery_contract(
            status="draft",
            code_sha="",
            exact_sha_ci_evidence="",
            live_collection_authorized=False,
        )
        contract["smoke"]["expected_pair_count"] = 2
        manifest = {
            "candidate_visibility_policy": (
                "commandagent.goal_verify.candidate_manifest.source_config_v1"
            )
        }
        usage = {
            "wall_time_ms": 100,
            "input_tokens": 10,
            "output_tokens": 5,
            "total_tokens": 15,
        }
        oracle = {"source": "frozen_host_adapter_post_execution"}
        records = [
            {
                "pair_id": "eligible",
                "eligibility": {"runtime": {"run_recovery_one_arm": True}},
                "initial_only": {
                    "input_manifest": {"snapshot_sha256": "a"},
                    "result": {
                        "argv": ["commandagent", "--ultra-plan-run", "goal"],
                        "recovery_plan_attempts": {
                            "configured_recovery_runs": 0,
                            "executed_recovery_runs": 0,
                        },
                        "resource_usage": usage,
                    },
                    "output_artifact_manifest": manifest,
                    "external_oracles": oracle,
                },
                "recovery_one": {
                    "status": "completed",
                    "input_manifest": {"snapshot_sha256": "a"},
                    "result": {
                        "argv": ["commandagent", "--ultra-plan-run", "goal"],
                        "recovery_plan_attempts": {
                            "configured_recovery_runs": 1,
                            "executed_recovery_runs": 1,
                        },
                        "resource_usage": {
                            **usage,
                            "wall_time_ms": 150,
                            "total_tokens": 22,
                        },
                    },
                    "output_artifact_manifest": manifest,
                    "external_oracles": oracle,
                },
                "comparison": {
                    "quality_transition": "improved",
                    "resource_delta": {
                        "wall_time_ms": 50,
                        "input_tokens": 0,
                        "output_tokens": 0,
                        "total_tokens": 7,
                    },
                },
            },
            {
                "pair_id": "excluded",
                "eligibility": {"runtime": {"run_recovery_one_arm": False}},
                "initial_only": {
                    "result": {
                        "argv": ["commandagent", "--ultra-plan-run", "goal"],
                        "recovery_plan_attempts": {
                            "configured_recovery_runs": 0,
                            "executed_recovery_runs": 0,
                        },
                        "resource_usage": usage,
                    },
                    "output_artifact_manifest": manifest,
                    "external_oracles": oracle,
                },
                "recovery_one": {"status": "not_run"},
                "comparison": None,
            },
        ]

        report = build_recovery_report(records=records, contract=contract)

        self.assertTrue(report["instrument_ready"])
        self.assertFalse(report["effect_claim_allowed"])
        self.assertEqual(report["counts"]["attributed_improved"], 1)
        self.assertEqual(report["median_resource_delta"]["wall_time_ms"], 50)
        self.assertNotIn("registered_recovery_verify_commands", report["checks"])

        contract["smoke"]["require_registered_recovery_verify_commands"] = True
        missing_source = build_recovery_report(records=records, contract=contract)
        self.assertFalse(
            missing_source["checks"]["registered_recovery_verify_commands"]
        )
        records[0]["recovery_one"]["result"]["recovery_plan_attempts"][
            "attempts"
        ] = [
            {
                "attempt_index": 1,
                "recovery_verify_command_source": "completion_contract",
            }
        ]
        registered_source = build_recovery_report(records=records, contract=contract)
        self.assertTrue(
            registered_source["checks"]["registered_recovery_verify_commands"]
        )

        contract["smoke"][
            "require_registered_inner_recovery_verify_commands"
        ] = True
        missing_inner_binding = build_recovery_report(
            records=records, contract=contract
        )
        self.assertFalse(
            missing_inner_binding["checks"][
                "registered_inner_recovery_verify_commands"
            ]
        )
        records[0]["recovery_one"]["result"]["recovery_plan_attempts"][
            "step_plan_contract_bindings"
        ] = [
            {
                "binding_mode": "read_only_inspection",
                "source": "product_visible_completion_contract",
                "external_oracle_used": False,
                "bound_verify_commands": [],
                "registered_verify_commands": ["python3 cli.py 16"],
            },
            {
                "binding_mode": "completion_contract_final_success",
                "source": "product_visible_completion_contract",
                "external_oracle_used": False,
                "bound_verify_commands": ["python3 cli.py 16"],
                "registered_verify_commands": ["python3 cli.py 16"],
            },
        ]
        registered_inner_binding = build_recovery_report(
            records=records, contract=contract
        )
        self.assertTrue(
            registered_inner_binding["checks"][
                "registered_inner_recovery_verify_commands"
            ]
        )

        contract["smoke"]["require_fix_contract_continuity"] = True
        contract["smoke"]["typed_fix_reproducer_commands"] = {
            "eligible": "python3 cli.py 16"
        }
        missing_fix_continuity = build_recovery_report(
            records=records, contract=contract
        )
        self.assertFalse(
            missing_fix_continuity["checks"]["fix_contract_continuity"]
        )
        records[0]["recovery_one"]["result"]["recovery_plan_attempts"][
            "fix_contract_resumptions"
        ] = [
            {
                "original_intent": "fix",
                "contract_origin": "fix_intent_v0",
                "contract_version": "v0",
                "contract_ref": "docs/fix-intent-contract.md",
                "fix_run_id": "01a-test-fix",
                "reproducer_command": "python3 cli.py 16",
                "source": "host_owned_recovery_fix_origin",
                "external_oracle_used": False,
            }
        ]
        fixed_continuity = build_recovery_report(records=records, contract=contract)
        self.assertTrue(fixed_continuity["checks"]["fix_contract_continuity"])

        contract["smoke"]["require_recovery_fix_terminal_completion"] = True
        incomplete_recovery_fix = build_recovery_report(
            records=records, contract=contract
        )
        self.assertFalse(
            incomplete_recovery_fix["checks"][
                "recovery_fix_terminal_completion"
            ]
        )
        recovery_result = records[0]["recovery_one"]["result"]
        recovery_result.update(
            {
                "status": "completed",
                "returncode": 0,
                "completion_verify_passed": True,
                "terminal_status": {"ok": True, "status": "completed"},
            }
        )
        recovery_attempts = recovery_result["recovery_plan_attempts"]
        recovery_attempts["attempts"][0].update(
            {"status": "succeeded", "stop_reason": "recovery_succeeded"}
        )
        recovery_attempts["terminal_stop_reason"] = "recovery_succeeded"
        recovery_attempts["promotion_decisions"] = [{"decision": "promoted"}]
        completed_recovery_fix = build_recovery_report(
            records=records, contract=contract
        )
        self.assertTrue(
            completed_recovery_fix["checks"][
                "recovery_fix_terminal_completion"
            ]
        )

        records[0]["recovery_one"]["result"]["recovery_plan_attempts"][
            "executed_recovery_runs"
        ] = 0
        no_live_recovery = build_recovery_report(records=records, contract=contract)
        self.assertFalse(no_live_recovery["instrument_ready"])
        self.assertFalse(
            no_live_recovery["checks"]["minimum_executed_recovery_pairs_observed"]
        )

        records[0]["recovery_one"]["result"]["recovery_plan_attempts"][
            "executed_recovery_runs"
        ] = 1
        records[0]["recovery_one"]["result"]["argv"][1] = "--plan-run"
        wrong_action = build_recovery_report(records=records, contract=contract)
        self.assertFalse(wrong_action["instrument_ready"])
        self.assertFalse(wrong_action["checks"]["execution_action_matches_contract"])

    def test_candidate_resource_usage_covers_full_lane_wall_and_all_attempt_tokens(
        self,
    ):
        usage = _candidate_resource_usage(
            attempts=[
                {"response": {"response": {"prompt_eval_count": 10, "eval_count": 2}}},
                {"response": {"response": {"prompt_eval_count": 20, "eval_count": 3}}},
            ],
            wall_time_ns=9_500_000,
            phase_timings_ns={
                "prompt_assembly": 500_000,
                "provider_request": 8_000_000,
            },
        )
        self.assertEqual(usage["wall_time_ms"], 9.5)
        self.assertEqual(usage["total_tokens"], 35)
        self.assertTrue(usage["token_measurement_complete"])
        self.assertEqual(
            usage["phase_timings_ms"],
            {"prompt_assembly": 0.5, "provider_request": 8.0},
        )
        self.assertEqual(usage["phase_timing_residual_ms"], 1.0)

    def test_generated_main_design_is_12_by_10_by_3(self):
        corpus = load("eval/goal_verify/v0/phase6-main-corpus-v4.json")
        contract = load("eval/goal_verify/v0/phase6-main-v4-contract.json")
        matrix = load("eval/goal_verify/v0/phase6-matrix.json")
        self.assertEqual(
            main_design_errors(corpus=corpus, contract=contract, matrix=matrix), []
        )
        self.assertEqual(len(corpus["cases"]), 120)
        self.assertEqual(len(contract["selected_cells"]), 120)
        self.assertEqual(contract["samples_per_cell"], 3)

    def test_a13_draft_preserves_thresholds_and_fails_closed(self):
        base = load("eval/goal_verify/v0/phase6-main-v4-contract.json")
        contract = build_a13_contract(
            status="draft",
            code_sha="",
            exact_sha_ci_evidence="",
            live_collection_authorized=False,
        )
        self.assertEqual(contract["status"], "draft")
        self.assertFalse(contract["authorization"]["live_collection_authorized"])
        self.assertEqual(contract["resource_budgets"], base["resource_budgets"])
        self.assertEqual(
            contract["main_analysis"]["threshold_mapping"],
            base["main_analysis"]["threshold_mapping"],
        )
        self.assertEqual(contract["full_experiment"], base["full_experiment"])

        adapters = build_a13_adapters(status="draft")["adapters"]
        investigation = [
            row for row in adapters if row["case_id"].startswith("phase6-main-c09-")
        ]
        self.assertTrue(investigation)
        self.assertTrue(
            all(row["executor"]["kind"] == "unavailable" for row in investigation)
        )

        tasks = build_a13_tasks(status="draft")["cases"]
        self.assertTrue(
            all(row["completion_contract"]["goal"] == row["goal"] for row in tasks)
        )

    def test_a13_instrument_checks_bind_policy_and_phase_timing(self):
        contract = build_a13_contract(
            status="draft",
            code_sha="",
            exact_sha_ci_evidence="",
            live_collection_authorized=False,
        )
        policy_sha = contract["semantic_oracle_policy"]["sha256"]
        phases = {
            phase: 0.0
            for phase in (
                "prompt_assembly",
                "provider_request",
                "raw_schema_validation",
                "canonicalization",
                "proposal_validation",
                "oracle_execution",
                "scoring",
            )
        }
        records = [
            {
                "lanes": {
                    "held_out_synthesis": {
                        "validation": {"valid": True},
                        "execution": {
                            "semantic_policy_sha256": policy_sha,
                            "execution_policy_source": "candidate_visible_prompt",
                            "evaluations": [
                                {
                                    "classification": "semantic_rejected",
                                    "execution_attempt_recorded": False,
                                    "executed": False,
                                    "result": "unverified",
                                    "semantic_policy_sha256": policy_sha,
                                }
                            ],
                        },
                        "resource_usage": {
                            "phase_timings_ms": phases,
                            "phase_timing_residual_ms": 0.1,
                        },
                    }
                }
            }
        ]

        checks = _a13_instrument_checks(contract=contract, records=records)
        self.assertTrue(all(checks.values()), checks)
        records[0]["lanes"]["held_out_synthesis"]["execution"]["evaluations"][0][
            "executed"
        ] = True
        checks = _a13_instrument_checks(contract=contract, records=records)
        self.assertFalse(checks["semantic_rejected_not_executed"])

    def test_main_design_rejects_pseudoreplicated_task_binding(self):
        corpus = load("eval/goal_verify/v0/phase6-main-corpus-v4.json")
        contract = load("eval/goal_verify/v0/phase6-main-v4-contract.json")
        matrix = load("eval/goal_verify/v0/phase6-matrix.json")
        corpus["cases"][1]["goal"] = corpus["cases"][0]["goal"]
        errors = main_design_errors(
            corpus=corpus,
            contract=contract,
            matrix=matrix,
        )
        self.assertIn("main_cell_goals_not_distinct:cell-01", errors)

    def test_main_cluster_manifest_distinguishes_smoke_from_population(self):
        corpus = load("eval/goal_verify/v0/phase6-main-corpus-v4.json")
        selected = corpus["cases"]
        smoke_pairs = [
            f"{case['case_id']}--pair-01"
            for case in selected
            if case["case_id"].endswith("-task-01")
        ]
        manifest = _cluster_manifest(
            selected=selected,
            samples_per_task=3,
            selected_pair_ids=smoke_pairs,
        )["cluster_design"]
        self.assertEqual(manifest["population_cell_count"], 12)
        self.assertEqual(manifest["population_source_task_count"], 120)
        self.assertEqual(manifest["population_pair_count"], 360)
        self.assertEqual(manifest["selected_cell_count"], 12)
        self.assertEqual(manifest["selected_source_task_count"], 12)
        self.assertEqual(manifest["selected_pair_count"], 12)

    def test_main_report_uses_cluster_bootstrap_and_fixed_budgets(self):
        contract = load("eval/goal_verify/v0/phase6-main-v4-contract.json")
        contract = copy.deepcopy(contract)
        contract["main_analysis"]["bootstrap_samples"] = 20
        config = load("eval/goal_verify/v0/baseline-config.json")
        records = _synthetic_records()
        report = build_main_report(
            contract=contract,
            config=config,
            records=records,
            semantic_review_complete=True,
            semantic_review_evaluation={"pass": True},
        )
        self.assertEqual(report["final_decision"], "GO")
        self.assertTrue(report["checks"]["cluster_design"])
        primary = report["lane_reports"]["held_out_synthesis"]
        self.assertEqual(
            primary["bootstrap"]["strong_binding"]["method"],
            "hierarchical_cluster_paired_percentile",
        )
        self.assertEqual(
            primary["resources"]["percentiles"]["p50_wall_time_increase_pct"],
            7.0,
        )
        self.assertEqual(
            primary["resources"]["diagnostics"]["record_count"],
            len(records),
        )
        records[0].pop("source_task_id")
        no_go = build_main_report(
            contract=contract,
            config=config,
            records=records,
            semantic_review_complete=True,
            semantic_review_evaluation={"pass": True},
        )
        self.assertEqual(no_go["final_decision"], "NO-GO")
        self.assertFalse(no_go["checks"]["cluster_design"])

    def test_resource_diagnostics_separates_coverage_ratios_and_tails(self):
        records = [
            _resource_record(
                pair_id="pair-01",
                cell_id="cell-01",
                baseline_wall_ms=100,
                baseline_tokens=100,
                candidate_wall_ms=50,
                candidate_input_tokens=80,
                candidate_output_tokens=20,
                provider_client_ms=40,
                provider_prompt_ms=10,
                provider_output_ms=30,
                evaluations=[{"executed": True, "runtime_ms": 5}],
            ),
            _resource_record(
                pair_id="pair-02",
                cell_id="cell-02",
                baseline_wall_ms=200,
                baseline_tokens=200,
                candidate_wall_ms=20,
                candidate_input_tokens=30,
                candidate_output_tokens=10,
                provider_client_ms=10,
                provider_prompt_ms=2,
                provider_output_ms=8,
                evaluations=[{"executed": True}, {"executed": False}],
            ),
        ]
        diagnostics = build_resource_diagnostics(
            records=records,
            lane_name="held_out_synthesis",
        )
        self.assertTrue(diagnostics["provider_timing"]["complete"])
        self.assertEqual(diagnostics["provider_timing"]["attempt_count"], 2)
        runtime = diagnostics["oracle_runtime"]
        self.assertEqual(runtime["evaluation_count"], 3)
        self.assertEqual(runtime["runtime_recorded_count"], 1)
        self.assertEqual(runtime["executed_count"], 2)
        self.assertEqual(runtime["executed_runtime_missing_count"], 1)
        self.assertEqual(runtime["unexecuted_count"], 1)
        self.assertEqual(diagnostics["attribution"]["residual_ms"]["p50"], 5.0)
        self.assertFalse(diagnostics["candidate_phase_timing"]["complete"])
        self.assertEqual(diagnostics["candidate_phase_timing"]["recorded_count"], 0)
        self.assertEqual(
            diagnostics["attribution"]["overall_input_share_of_candidate_tokens_pct"],
            78.571429,
        )
        self.assertEqual(
            diagnostics["cell_median_increase_pct"]["cell-01"],
            {
                "wall_time_increase_pct": 50.0,
                "total_tokens_increase_pct": 100.0,
            },
        )
        self.assertEqual(
            diagnostics["tails"]["wall_increase_ratio_top_5pct"]["cell_counts"],
            {"cell-01": 1},
        )
        self.assertEqual(
            diagnostics["tails"]["candidate_absolute_wall_top_5pct"]["cell_counts"],
            {"cell-01": 1},
        )

    def test_resource_diagnostics_reports_incomplete_provider_timing(self):
        record = _resource_record(
            pair_id="pair-01",
            cell_id="cell-01",
            baseline_wall_ms=100,
            baseline_tokens=100,
            candidate_wall_ms=50,
            candidate_input_tokens=80,
            candidate_output_tokens=20,
            provider_client_ms=40,
            provider_prompt_ms=10,
            provider_output_ms=30,
            evaluations=[],
        )
        del record["lanes"]["held_out_synthesis"]["attempts"][0]["response"][
            "response"
        ]["eval_duration"]
        diagnostics = build_resource_diagnostics(
            records=[record],
            lane_name="held_out_synthesis",
        )
        self.assertFalse(diagnostics["provider_timing"]["complete"])
        self.assertEqual(diagnostics["provider_timing"]["complete_attempt_count"], 0)
        self.assertEqual(diagnostics["provider_timing"]["output_eval_ms"]["count"], 0)
        self.assertEqual(diagnostics["attribution"]["residual_ms"]["p50"], 10.0)

    def test_resource_diagnostics_reports_candidate_phase_timing(self):
        record = _resource_record(
            pair_id="pair-01",
            cell_id="cell-01",
            baseline_wall_ms=100,
            baseline_tokens=100,
            candidate_wall_ms=50,
            candidate_input_tokens=80,
            candidate_output_tokens=20,
            provider_client_ms=40,
            provider_prompt_ms=10,
            provider_output_ms=30,
            evaluations=[],
            phase_timings_ms={
                "prompt_assembly": 2.0,
                "provider_request": 40.0,
            },
            phase_timing_residual_ms=8.0,
        )
        diagnostics = build_resource_diagnostics(
            records=[record], lane_name="held_out_synthesis"
        )
        timing = diagnostics["candidate_phase_timing"]
        self.assertTrue(timing["complete"])
        self.assertEqual(timing["phases_ms"]["provider_request"]["p50"], 40.0)
        self.assertEqual(timing["instrumentation_residual_ms"]["p50"], 8.0)

    def test_main_semantic_review_safety_rule_is_authoritative(self):
        contract = load("eval/goal_verify/v0/phase6-main-v4-contract.json")
        review = {
            "review_count": 36,
            "verdict_counts": {
                "acceptable": 30,
                "needs_revision": 6,
                "unusable": 0,
            },
            "axis_pass_counts": {
                "false_positive_or_overconstraint_risk_acceptable": 36,
            },
        }
        passed = evaluate_main_semantic_review(
            contract=contract,
            blind_report={"calibration_review": review},
            semantic_review_complete=True,
        )
        self.assertTrue(passed["pass"])
        review["verdict_counts"]["unusable"] = 1
        failed = evaluate_main_semantic_review(
            contract=contract,
            blind_report={"calibration_review": review},
            semantic_review_complete=True,
        )
        self.assertFalse(failed["pass"])
        self.assertFalse(failed["checks"]["unusable_count"])

    def test_main_smoke_report_checks_frozen_instrument_only(self):
        contract = load("eval/goal_verify/v0/phase6-main-v4-contract.json")
        expected_pair_ids = contract["smoke"]["pair_ids"]
        records = [
            record
            for record in _synthetic_records()
            if record["source_task_id"].endswith("-01")
            and record["pair_id"].endswith("-r01")
        ]
        for record, pair_id in zip(records, expected_pair_ids, strict=True):
            record["pair_id"] = pair_id
            record["snapshot_manifests"] = {"product": {"snapshot_sha256": "a" * 64}}
            record["baseline"].update(
                {
                    "completion_contract_bound": True,
                    "completion_verify_attempt_recorded": True,
                    "product_run_dir": "/frozen/product/run",
                    "recovery_plan_auto_runs": 0,
                }
            )
            for lane in record["lanes"].values():
                lane["validation"].update(
                    {"valid_before_host_repairs": True, "host_repairs": []}
                )
        manifest = {
            "campaign_role": "preregistered_smoke",
            "request_namespace": contract["smoke"]["request_namespace"],
            "selected_pair_ids": expected_pair_ids,
            "target_pairs": len(expected_pair_ids),
            "cluster_design": {
                "cluster_unit": "source_task_id",
                "population_cell_count": 12,
                "population_source_task_count": 120,
                "runs_per_source_task": 3,
                "population_pair_count": 360,
                "selected_cell_count": 12,
                "selected_source_task_count": 12,
                "selected_pair_count": 12,
            },
        }
        report = build_main_smoke_report(
            contract=contract,
            records=records,
            manifest=manifest,
        )
        self.assertEqual(report["final_decision"], "GO")
        self.assertTrue(report["checks"]["resource_measurement_complete"])
        records[0]["baseline"]["recovery_plan_auto_runs"] = 1
        no_go = build_main_smoke_report(
            contract=contract,
            records=records,
            manifest=manifest,
        )
        self.assertEqual(no_go["final_decision"], "NO-GO")
        self.assertFalse(no_go["checks"]["recovery_plan_auto_runs_zero"])

    def test_main_review_selects_three_distinct_tasks_per_cell(self):
        items = []
        mapping = {}
        for cell in range(1, 13):
            for task in range(1, 11):
                item_id = f"item-{cell:02d}-{task:02d}"
                items.append({"item_id": item_id})
                mapping[item_id] = {
                    "source_case_id": f"case-{cell:02d}-{task:02d}",
                    "source_lane": "held_out_synthesis",
                    "cell_id": f"cell-{cell:02d}",
                    "source_task_id": f"task-{cell:02d}-{task:02d}",
                }
        selected = human_sample(
            items=items,
            mapping=mapping,
            sample_spec={
                "size": 36,
                "items_per_cell": 3,
                "source_lane": "held_out_synthesis",
            },
        )
        self.assertEqual(len(selected), 36)
        self.assertEqual(len(set(selected)), 36)

    def test_main_review_does_not_require_inadmissible_model_evidence(self):
        item = {
            "item_id": "item-1",
            "item_sha256": "a" * 64,
            "goal": "g",
            "intent": "create",
            "profile": "cli",
            "required_claims": [],
            "group_kind": "empty_proposal",
            "raw_claim": None,
            "raw_oracles": [],
        }
        document = {
            "items_sha256": canonical_sha256([item]),
            "human_items_sha256": canonical_sha256([item]),
            "reviewer_id": "human",
            "reviewer_type": "human",
            "contract_authoring_involvement": False,
            "independence_confirmed": True,
            "item_ids": ["item-1"],
            "reviews": [
                {
                    "item_id": "item-1",
                    "verdict": "unusable",
                    "axes": {
                        "requirement_clear": False,
                        "input_observation_expected_specific": False,
                        "executable_from_visible_information": False,
                        "false_positive_or_overconstraint_risk_acceptable": False,
                        "semantic_duplication_absent": True,
                    },
                    "reason_codes": ["empty"],
                    "rationale": "No proposal was produced.",
                }
            ],
        }
        report = build_blind_report(
            items=[item],
            model_documents=[],
            human_document=document,
            human_items=[item],
            review_contract={
                "model_reviews_required": False,
                "main_sample": {"size": 1},
            },
        )
        self.assertTrue(report["semantic_review_complete"])
        self.assertFalse(report["model_reviews_required"])


def _synthetic_records():
    records = []
    for cell in range(1, 13):
        for task in range(1, 11):
            for run in range(1, 4):
                lanes = {}
                for lane_name in ("contract_conformance", "held_out_synthesis"):
                    lanes[lane_name] = {
                        "validation": {"valid": True},
                        "execution": {
                            "same_snapshot": True,
                            "reference_fallback_count": 0,
                            "gold_used_for_execution_count": 0,
                        },
                        "attempts": [
                            {
                                "response": {
                                    "response": {
                                        "client_wall_time_ns": 5_000_000,
                                        "prompt_eval_count": 3,
                                        "eval_count": 2,
                                    }
                                }
                            }
                        ],
                        "resource_usage": {
                            "wall_time_ms": 7.0,
                            "input_tokens": 4,
                            "output_tokens": 2,
                            "total_tokens": 6,
                            "token_measurement_complete": True,
                        },
                        "additive_comparison": {
                            "baseline_failure_overridden": False,
                            "shadow_verdict": "failure",
                            "combined_score": {
                                "claims": [{"status": "strong"}],
                            },
                            "paired_delta": {
                                "required_claim_recall": 0.20,
                                "strong_binding": 0.20,
                                "unverified_rate": -0.20,
                            },
                        },
                    }
                records.append(
                    {
                        "pair_id": f"c{cell:02d}-t{task:02d}-r{run:02d}",
                        "cell_id": f"cell-{cell:02d}",
                        "source_task_id": f"task-{cell:02d}-{task:02d}",
                        "baseline": {
                            "resource_usage": {
                                "wall_time_ms": 100,
                                "total_tokens": 100,
                            }
                        },
                        "lanes": lanes,
                    }
                )
    return records


def _resource_record(
    *,
    pair_id: str,
    cell_id: str,
    baseline_wall_ms: int,
    baseline_tokens: int,
    candidate_wall_ms: int,
    candidate_input_tokens: int,
    candidate_output_tokens: int,
    provider_client_ms: int,
    provider_prompt_ms: int,
    provider_output_ms: int,
    evaluations: list[dict],
    phase_timings_ms: dict[str, float] | None = None,
    phase_timing_residual_ms: float | None = None,
):
    record = {
        "pair_id": pair_id,
        "cell_id": cell_id,
        "baseline": {
            "resource_usage": {
                "wall_time_ms": baseline_wall_ms,
                "total_tokens": baseline_tokens,
            }
        },
        "lanes": {
            "held_out_synthesis": {
                "attempts": [
                    {
                        "response": {
                            "response": {
                                "client_wall_time_ns": provider_client_ms * 1_000_000,
                                "prompt_eval_duration": provider_prompt_ms * 1_000_000,
                                "eval_duration": provider_output_ms * 1_000_000,
                            }
                        }
                    }
                ],
                "resource_usage": {
                    "wall_time_ms": candidate_wall_ms,
                    "input_tokens": candidate_input_tokens,
                    "output_tokens": candidate_output_tokens,
                    "total_tokens": candidate_input_tokens + candidate_output_tokens,
                },
                "execution": {"evaluations": evaluations},
            }
        },
    }
    if phase_timings_ms is not None:
        usage = record["lanes"]["held_out_synthesis"]["resource_usage"]
        usage["phase_timings_ms"] = phase_timings_ms
        usage["phase_timing_residual_ms"] = phase_timing_residual_ms
    return record


if __name__ == "__main__":
    unittest.main()
