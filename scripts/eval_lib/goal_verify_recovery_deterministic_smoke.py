from __future__ import annotations

import hashlib
import json
import shutil
import subprocess
import tempfile
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any

VERIFY_COMMAND = (
    'python3 -c "from pathlib import Path; '
    "assert Path('result.txt').read_text() == 'recovered\\n'\""
)
DATA_REPRO_COMMAND = "python3 scripts/repro.py data/task-02.csv"
DATA_REGRESSION_COMMANDS = (
    "python3 -m pytest -q tests",
    "python3 scripts/contract_check.py",
)
DATA_REGRESSION_IDS = (
    "completion_contract_verify_2",
    "completion_contract_verify_3",
)
DATA_FIXTURE = (
    Path(__file__).resolve().parents[2]
    / "tests/fixtures/goal_verify_v4/a15/fix-data-reconciliation/before"
)
DATA_GOAL = (
    "Fix the existing deterministic data pipeline so `python3 scripts/repro.py "
    "data/task-02.csv` changes from exit 1 to exit 0, with input_rows = used_rows "
    "+ excluded rows. Preserve the frozen pytest and contract-check regressions "
    "and regenerate the standard data profile outputs."
)
GENERIC_FIX_REPRO_COMMAND = "python3 app.py fixture/task-02.json"
GENERIC_FIXTURE = (
    Path(__file__).resolve().parents[2]
    / "tests/fixtures/goal_verify_v4/main/fix-generic-fixtures/before"
)
GENERIC_FIX_GOAL = (
    "Fix app.py so the registered reproducer handles items without amount while "
    "preserving the existing command-line interface."
)


def _step_plan(
    goal: str,
    step_id: str,
    kind: str,
    instruction: str,
    *,
    expected_paths: list[str] | None = None,
    verify: list[str] | None = None,
) -> str:
    return json.dumps(
        {
            "goal": goal,
            "steps": [
                {
                    "id": step_id,
                    "kind": kind,
                    "expected_result": "pass",
                    "instruction": instruction,
                    "expected_paths": expected_paths or [],
                    "verify": verify or [],
                }
            ],
        },
        separators=(",", ":"),
    )


class ScriptedRecoveryProvider:
    """Deterministic provider for path coverage, never for an effect claim."""

    def __init__(self) -> None:
        self._lock = threading.Lock()
        self.phase = "initial"
        self.inspected = False
        self.wrote = False
        self.verified = False
        self.request_count = 0
        self.trace: list[dict[str, Any]] = []

    def response_for(self, body: dict[str, Any]) -> dict[str, Any]:
        messages = body.get("messages", [])
        text = "\n".join(str(row.get("content", "")) for row in messages)
        tools = body.get("tools", [])
        with self._lock:
            self.request_count += 1
            if not tools:
                response = self._planner_response(text)
                response_kind = "step_plan"
            else:
                response, response_kind = self._execution_response(text)
            self.trace.append(
                {
                    "request_index": self.request_count,
                    "request_kind": "planner" if not tools else "execution",
                    "phase": self.phase,
                    "response_kind": response_kind,
                }
            )
            return {
                "message": response,
                "done": True,
                "prompt_eval_count": 1,
                "eval_count": 1,
            }

    def _planner_response(self, text: str) -> dict[str, Any]:
        if "Inspect the current workspace before changing files" in text:
            self.phase = "inspect"
            content = _step_plan(
                "Inspect recovery state",
                "inspect-state",
                "inspect",
                "Inspect result.txt",
            )
        elif "Repair the incomplete work for the failed phase" in text:
            self.phase = "repair"
            content = _step_plan(
                "Repair result",
                "repair-result",
                "implement",
                "Repair result.txt",
                expected_paths=["result.txt"],
                verify=[VERIFY_COMMAND],
            )
        elif "Verify the recovered output with deterministic checks" in text:
            self.phase = "verify"
            content = _step_plan(
                "Verify result",
                "verify-result",
                "verify",
                "Verify result.txt",
                verify=[VERIFY_COMMAND],
            )
        else:
            self.phase = "initial"
            content = _step_plan(
                "Repair result",
                "repair-result",
                "implement",
                "Repair result.txt",
                expected_paths=["result.txt"],
                verify=[VERIFY_COMMAND],
            )
        return {"content": content}

    def _execution_response(self, _text: str) -> tuple[dict[str, Any], str]:
        if self.phase == "inspect" and not self.inspected:
            self.inspected = True
            return (
                {
                    "content": "",
                    "tool_calls": [
                        {
                            "function": {
                                "name": "Read",
                                "arguments": {"path": "result.txt"},
                            }
                        }
                    ],
                },
                "Read",
            )
        if self.phase == "repair" and not self.wrote:
            self.wrote = True
            return (
                {
                    "content": "",
                    "tool_calls": [
                        {
                            "function": {
                                "name": "Write",
                                "arguments": {
                                    "path": "result.txt",
                                    "content": "recovered\n",
                                },
                            }
                        }
                    ],
                },
                "Write",
            )
        if self.phase == "verify" and not self.verified:
            self.verified = True
            return (
                {
                    "content": "",
                    "tool_calls": [
                        {
                            "function": {
                                "name": "Bash",
                                "arguments": {"command": VERIFY_COMMAND},
                            }
                        }
                    ],
                },
                "Bash",
            )
        if self.phase == "initial":
            return (
                {"content": "Initial attempt made no workspace edit."},
                "intentional_no_tool",
            )
        return {"content": "Scripted Recovery step complete."}, "complete"


class ScriptedDataFixRecoveryProvider(ScriptedRecoveryProvider):
    """Deterministic data/fix provider covering bound regression execution."""

    def __init__(self, corrected_pipeline: str) -> None:
        super().__init__()
        self.corrected_pipeline = corrected_pipeline
        self.initial_inspected = False

    def _planner_response(self, text: str) -> dict[str, Any]:
        if "Inspect the current workspace before changing files" in text:
            self.phase = "data_recovery_inspect"
            content = _step_plan(
                "Inspect data fix",
                "inspect-state",
                "inspect",
                "Inspect pipeline/main.py",
            )
        elif "Repair the incomplete work for the failed phase" in text:
            self.phase = "data_recovery_repair"
            content = _step_plan(
                "Repair data fix",
                "repair-pipeline",
                "implement",
                "Repair pipeline/main.py",
                expected_paths=["pipeline/main.py"],
                verify=[DATA_REPRO_COMMAND],
            )
        elif "Verify the recovered output with deterministic checks" in text:
            self.phase = "data_recovery_verify"
            content = _step_plan(
                "Verify data fix",
                "verify-data-fix",
                "verify",
                "Verify registered data fix commands",
                verify=[DATA_REPRO_COMMAND, *DATA_REGRESSION_COMMANDS],
            )
        else:
            self.phase = "data_initial"
            content = _step_plan(
                "Inspect data fix",
                "inspect-state",
                "inspect",
                "Inspect pipeline/main.py",
            )
        return {"content": content}

    def _execution_response(self, text: str) -> tuple[dict[str, Any], str]:
        if self.phase == "data_recovery_inspect" and not self.inspected:
            self.inspected = True
            return self._tool("Read", {"path": "pipeline/main.py"}), "Read"
        if self.phase == "data_recovery_repair" and not self.wrote:
            self.wrote = True
            return (
                self._tool(
                    "Write",
                    {
                        "path": "pipeline/main.py",
                        "content": self.corrected_pipeline,
                    },
                ),
                "Write",
            )
        if self.phase == "data_recovery_verify" and not self.verified:
            self.verified = True
            return self._tool("Bash", {"command": DATA_REPRO_COMMAND}), "Bash"
        if "Read only the executed runtime-bound F1 failure evidence" in text:
            if not self.initial_inspected:
                self.initial_inspected = True
                return self._tool("Read", {"path": "pipeline/main.py"}), "Read"
            return {"content": "Cause isolated."}, "complete"
        if (
            "Repair the F1-diagnosed defect" in text
            or "Fix F1 failure diagnostic" in text
        ):
            return (
                {"content": "Initial repair intentionally made no edit."},
                "intentional_no_tool",
            )
        return {"content": "Scripted data Recovery step complete."}, "complete"

    @staticmethod
    def _tool(name: str, arguments: dict[str, Any]) -> dict[str, Any]:
        return {
            "content": "",
            "tool_calls": [{"function": {"name": name, "arguments": arguments}}],
        }


class ScriptedGenericFixRecoveryProvider(ScriptedRecoveryProvider):
    """Deterministic generic/fix provider covering host-owned final verification."""

    def __init__(self, corrected_app: str) -> None:
        super().__init__()
        self.corrected_app = corrected_app
        self.initial_inspected = False

    def _planner_response(self, text: str) -> dict[str, Any]:
        if "Inspect the current workspace before changing files" in text:
            self.phase = "generic_recovery_inspect"
            content = _step_plan(
                "Inspect generic fix",
                "inspect-state",
                "inspect",
                "Inspect app.py",
            )
        elif "Repair the incomplete work for the failed phase" in text:
            self.phase = "generic_recovery_repair"
            content = _step_plan(
                "Repair generic fix",
                "repair-app",
                "implement",
                "Repair app.py",
                expected_paths=["app.py"],
                verify=[GENERIC_FIX_REPRO_COMMAND],
            )
        elif "Verify the recovered output with deterministic checks" in text:
            self.phase = "generic_recovery_verify"
            content = _step_plan(
                "Verify generic fix",
                "verify-generic-fix",
                "verify",
                "Verify the registered generic fix command",
                verify=[GENERIC_FIX_REPRO_COMMAND],
            )
        else:
            self.phase = "generic_initial"
            content = _step_plan(
                "Inspect generic fix",
                "inspect-state",
                "inspect",
                "Inspect app.py",
            )
        return {"content": content}

    def _execution_response(self, text: str) -> tuple[dict[str, Any], str]:
        if self.phase == "generic_recovery_inspect" and not self.inspected:
            self.inspected = True
            return self._tool("Read", {"path": "app.py"}), "Read"
        if self.phase == "generic_recovery_repair" and not self.wrote:
            self.wrote = True
            return (
                self._tool(
                    "Write",
                    {"path": "app.py", "content": self.corrected_app},
                ),
                "Write",
            )
        if self.phase == "generic_recovery_verify" and not self.verified:
            self.verified = True
            return self._tool("Bash", {"command": GENERIC_FIX_REPRO_COMMAND}), "Bash"
        if "Read only the executed runtime-bound F1 failure evidence" in text:
            if not self.initial_inspected:
                self.initial_inspected = True
                return self._tool("Read", {"path": "app.py"}), "Read"
            return {"content": "Cause isolated."}, "complete"
        if (
            "Repair the F1-diagnosed defect" in text
            or "Fix F1 failure diagnostic" in text
        ):
            return (
                {"content": "Initial repair intentionally made no edit."},
                "intentional_no_tool",
            )
        return {"content": "Scripted generic Recovery step complete."}, "complete"

    @staticmethod
    def _tool(name: str, arguments: dict[str, Any]) -> dict[str, Any]:
        return {
            "content": "",
            "tool_calls": [{"function": {"name": name, "arguments": arguments}}],
        }


class _ProviderServer(ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self, provider: ScriptedRecoveryProvider) -> None:
        self.provider = provider
        super().__init__(("127.0.0.1", 0), _ProviderHandler)


class _ProviderHandler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def do_POST(self) -> None:
        length = int(self.headers.get("Content-Length", "0"))
        body = json.loads(self.rfile.read(length))
        response = self.server.provider.response_for(body)  # type: ignore[attr-defined]
        if body.get("stream") is True:
            payload = (
                json.dumps(
                    {"message": response["message"], "done": False},
                    separators=(",", ":"),
                )
                + "\n"
                + json.dumps(
                    {"done": True, "prompt_eval_count": 1, "eval_count": 1},
                    separators=(",", ":"),
                )
                + "\n"
            ).encode()
            content_type = "application/x-ndjson"
        else:
            payload = json.dumps(response, separators=(",", ":")).encode()
            content_type = "application/json"
        self.send_response(200)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def log_message(self, _format: str, *args: Any) -> None:
        del args


def _write_fixture(workspace: Path) -> tuple[Path, Path]:
    initial_plan = workspace / "initial.yaml"
    initial_plan.write_text(
        """goal: \"Repair result.txt so it contains recovered\"
profile: \"generic\"
style: \"default\"
intent: \"create\"
phases:
  - id: \"repair\"
    prompt: \"Repair result.txt so it contains recovered.\"
  - id: \"verify\"
    prompt: \"Verify result.txt contains recovered.\"
""",
        encoding="utf-8",
    )
    completion_contract = workspace / "completion.json"
    completion_contract.write_text(
        json.dumps(
            {
                "goal": "Repair result.txt so it contains recovered",
                "profile": "generic",
                "required_paths": ["result.txt"],
                "verify_commands": [VERIFY_COMMAND],
                "required_capabilities": [],
                "required_evidence": [],
                "required_obligations": [],
                "deferred_verify_requirements": [],
                "evidence_hint_tokens": [],
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    (workspace / "result.txt").write_text("broken\n", encoding="utf-8")
    return initial_plan, completion_contract


def _write_data_fix_fixture(workspace: Path) -> tuple[Path, Path, str]:
    if not DATA_FIXTURE.is_dir():
        raise ValueError(f"data fix fixture is missing:{DATA_FIXTURE}")
    shutil.copytree(
        DATA_FIXTURE,
        workspace,
        dirs_exist_ok=True,
        ignore=shutil.ignore_patterns(".pytest_cache", "__pycache__", "*.pyc"),
    )
    pipeline_path = workspace / "pipeline/main.py"
    corrected_pipeline = pipeline_path.read_text(encoding="utf-8").replace(
        '"used_rows": len(rows),',
        '"used_rows": len(valid_rows),',
    )
    if corrected_pipeline == pipeline_path.read_text(encoding="utf-8"):
        raise ValueError("data fix fixture no longer contains the expected defect")
    initial_plan = workspace / "initial.yaml"
    initial_plan.write_text(
        "\n".join(
            [
                f"goal: {json.dumps(DATA_GOAL)}",
                'profile: "data"',
                'style: "default"',
                'intent: "fix"',
                "phases:",
                '  - id: "reproduce-before"',
                '    prompt: "Bind and run the deterministic failing reproducer."',
                '  - id: "isolate-cause"',
                '    prompt: "Isolate the cause without editing."',
                '  - id: "repair"',
                '    prompt: "Repair the diagnosed defect."',
                '  - id: "verify-regressions"',
                '    prompt: "Verify the exact reproducer and registered regressions."',
                "",
            ]
        ),
        encoding="utf-8",
    )
    completion_contract = workspace / "completion.json"
    completion_contract.write_text(
        json.dumps(
            {
                "goal": DATA_GOAL,
                "profile": "data",
                "protected_paths": [
                    "data",
                    "scripts/repro.py",
                    "scripts/contract_check.py",
                    "tests",
                ],
                "required_paths": [
                    "pipeline/main.py",
                    "data/task-02.csv",
                    "scripts/repro.py",
                    "scripts/contract_check.py",
                    "tests/test_pipeline.py",
                    "output/inspection.json",
                    "output/results.json",
                    "output/report.md",
                ],
                "verify_commands": [
                    DATA_REPRO_COMMAND,
                    *DATA_REGRESSION_COMMANDS,
                ],
                "fix_reproducer_command": DATA_REPRO_COMMAND,
                "required_capabilities": [
                    "data_reconciliation",
                    "data_claims_binding",
                    "data_rerun_consistency",
                    "data_results_schema",
                ],
                "required_evidence": [
                    "implementation_artifact",
                    "test_artifact",
                    "bound_verify_command",
                    "non_zero_test_or_assertion_evidence",
                ],
                "required_obligations": [
                    "implementation",
                    "verification",
                    "acceptance_evidence",
                ],
                "deferred_verify_requirements": [],
                "evidence_hint_tokens": [
                    "data/task-02.csv",
                    "data_reconciliation",
                    "contract-check",
                ],
                "verify_repair_cap": 1,
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    return initial_plan, completion_contract, corrected_pipeline


def _write_generic_fix_fixture(workspace: Path) -> tuple[Path, Path, str]:
    if not GENERIC_FIXTURE.is_dir():
        raise ValueError(f"generic fix fixture is missing:{GENERIC_FIXTURE}")
    shutil.copytree(GENERIC_FIXTURE, workspace, dirs_exist_ok=True)
    app_path = workspace / "app.py"
    original_app = app_path.read_text(encoding="utf-8")
    corrected_app = original_app.replace(
        'item["amount"]',
        'item.get("amount", 0)',
    )
    if corrected_app == original_app:
        raise ValueError("generic fix fixture no longer contains the expected defect")
    initial_plan = workspace / "initial.yaml"
    initial_plan.write_text(
        "\n".join(
            [
                f"goal: {json.dumps(GENERIC_FIX_GOAL)}",
                'profile: "generic"',
                'style: "default"',
                'intent: "fix"',
                "phases:",
                '  - id: "reproduce-before"',
                '    prompt: "Bind and run the deterministic failing reproducer."',
                '  - id: "isolate-cause"',
                '    prompt: "Isolate the cause without editing."',
                '  - id: "repair"',
                '    prompt: "Repair the diagnosed defect."',
                '  - id: "verify-regressions"',
                '    prompt: "Verify the registered reproducer."',
                "",
            ]
        ),
        encoding="utf-8",
    )
    completion_contract = workspace / "completion.json"
    completion_contract.write_text(
        json.dumps(
            {
                "goal": GENERIC_FIX_GOAL,
                "profile": "generic",
                "protected_paths": ["fixture"],
                "required_paths": ["app.py", "fixture/task-02.json"],
                "verify_commands": [GENERIC_FIX_REPRO_COMMAND],
                "fix_reproducer_command": GENERIC_FIX_REPRO_COMMAND,
                "required_capabilities": [],
                "required_evidence": [
                    "implementation_artifact",
                    "bound_verify_command",
                ],
                "required_obligations": ["implementation"],
                "deferred_verify_requirements": [],
                "evidence_hint_tokens": ["fixture/task-02.json"],
                "verify_repair_cap": 1,
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    return initial_plan, completion_contract, corrected_app


def _rows(path: Path) -> list[dict[str, Any]]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]


def _event(rows: list[dict[str, Any]], name: str) -> list[dict[str, Any]]:
    return [row for row in rows if row.get("event") == name]


def build_report(
    *,
    rows: list[dict[str, Any]],
    returncode: int,
    final_artifact: str | None,
    provider_trace: list[dict[str, Any]],
    binary_sha256: str,
) -> dict[str, Any]:
    preflight = _event(rows, "recovery_preflight_observation")
    starts = _event(rows, "recovery_plan_auto_run_start")
    completions = _event(rows, "recovery_plan_auto_run_complete")
    promotions = _event(rows, "recovery_promotion_decision")
    deltas = _event(rows, "recovery_treatment_delta")
    bound = _event(rows, "recovery_candidate_verify_commands_bound")
    response_kinds = [row["response_kind"] for row in provider_trace]
    observation_events = preflight + bound + promotions
    checks = {
        "process_exit_zero": returncode == 0,
        "initial_failure_handoff_recorded": any(
            row.get("status") == "incomplete"
            for row in _event(rows, "recovery_prompt_saved")
        ),
        "pre_recovery_registered_observation_failed": any(
            row.get("observation_phase") == "pre_recovery"
            and row.get("status") == "fail"
            and row.get("source") == "product_visible_completion_contract"
            for row in preflight
        ),
        "registered_commands_rebound": any(
            row.get("source") == "product_visible_completion_contract"
            and row.get("registered_verify_command_count") == 1
            and row.get("recovery_verify_command_source") == "completion_contract"
            for row in bound
        ),
        "recovery_boundary_captured": any(
            row.get("status") == "captured"
            for row in _event(rows, "recovery_boundary_snapshot")
        ),
        "exactly_one_recovery_started": len(starts) == 1
        and starts[0].get("recovery_plan_auto_run_current") == 1,
        "scripted_read_write_verify_sequence": all(
            kind in response_kinds for kind in ("Read", "Write", "Bash")
        ),
        "treatment_product_delta_observed": any(
            "result.txt"
            in row.get("attempted_product_delta", {}).get("changed_paths", [])
            for row in deltas
        ),
        "post_recovery_registered_observation_passed": any(
            row.get("observation_phase") == "post_recovery"
            and row.get("status") == "pass"
            and row.get("source") == "product_visible_completion_contract"
            for row in preflight
        ),
        "treatment_promoted": len(promotions) == 1
        and promotions[0].get("decision") == "promoted",
        "recovery_completed": len(completions) == 1
        and completions[0].get("recovery_plan_auto_run_stop_reason")
        == "recovery_succeeded",
        "control_not_retained": not _event(rows, "recovery_control_retained"),
        "registered_observations_are_product_internal": all(
            row.get("external_oracle_used") is False for row in observation_events
        ),
        "final_artifact_passed": final_artifact == "recovered\n",
    }
    ready = all(checks.values())
    return {
        "schema_version": (
            "commandagent.goal_verify.recovery_deterministic_path_smoke.v1"
        ),
        "inference_role": "instrument_path_coverage_only",
        "effect_claim_allowed": False,
        "provider": "local_scripted_ollama_compatible",
        "binary_sha256": binary_sha256,
        "event_count": len(rows),
        "provider_request_count": len(provider_trace),
        "checks": checks,
        "instrument_ready": ready,
        "go_no_go": "GO" if ready else "NO-GO",
    }


def build_data_fix_report(
    *,
    rows: list[dict[str, Any]],
    returncode: int,
    final_pipeline: str | None,
    provider_trace: list[dict[str, Any]],
    binary_sha256: str,
    diagnostic_returncodes: dict[str, int],
) -> dict[str, Any]:
    preflight = _event(rows, "recovery_preflight_observation")
    promotions = _event(rows, "recovery_promotion_decision")
    completions = _event(rows, "recovery_plan_auto_run_complete")
    resumed = _event(rows, "recovery_fix_contract_resumed")
    fix_evidence = _event(rows, "fix_evidence_recorded")
    acceptances = _event(rows, "ultra_final_acceptance")
    terminals = _event(rows, "tui_command_stop")
    deltas = _event(rows, "recovery_treatment_delta")
    effect_policies = _event(rows, "recovery_observation_effect_policy_bound")
    regression_evidence = {
        row.get("binding_id"): row
        for row in fix_evidence
        if row.get("requirement_id") == "no_regression"
    }
    final_acceptance = acceptances[-1] if acceptances else {}
    terminal = terminals[-1] if terminals else {}
    observation_events = preflight + resumed + promotions + effect_policies
    checks = {
        "process_exit_zero": returncode == 0,
        "initial_reproducer_failed": any(
            row.get("requirement_id") == "before_fails"
            and row.get("executed") is True
            and row.get("outcome") == "failure"
            and row.get("binding_id") == DATA_REPRO_COMMAND
            for row in fix_evidence
        ),
        "pre_recovery_registered_observation_failed": any(
            row.get("observation_phase") == "pre_recovery"
            and row.get("status") == "fail"
            and row.get("source") == "product_visible_completion_contract"
            for row in preflight
        ),
        "registered_fix_contract_resumed": len(resumed) == 1
        and resumed[0].get("regression_source") == "completion_contract"
        and resumed[0].get("bound_regression_ids") == list(DATA_REGRESSION_IDS)
        and "pipeline_probe" in resumed[0].get("omitted_supplemental_ids", []),
        "registered_input_bound": any(
            row.get("registered_data_input_fixture") == "data/task-02.csv"
            and row.get("source") == "product_visible_completion_contract"
            for row in effect_policies
        ),
        "scripted_read_write_sequence": all(
            kind in [row["response_kind"] for row in provider_trace]
            for kind in ("Read", "Write")
        ),
        "pipeline_treatment_delta_observed": any(
            "pipeline/main.py"
            in row.get("attempted_product_delta", {}).get("changed_paths", [])
            for row in deltas
        ),
        "after_reproducer_passed": any(
            row.get("requirement_id") == "after_passes"
            and row.get("executed") is True
            and row.get("outcome") == "success"
            and row.get("binding_id") == DATA_REPRO_COMMAND
            for row in fix_evidence
        ),
        "registered_regressions_executed_successfully": all(
            regression_evidence.get(binding_id, {}).get("executed") is True
            and regression_evidence.get(binding_id, {}).get("outcome") == "success"
            and regression_evidence.get(binding_id, {}).get("reason") == ""
            for binding_id in DATA_REGRESSION_IDS
        ),
        "full_fix_acceptance": final_acceptance.get("verdict") == "full"
        and final_acceptance.get("external_contract_ok") is True
        and final_acceptance.get("requirement_statuses")
        == {
            "after_passes": "passed",
            "before_fails": "passed",
            "no_regression": "passed",
        },
        "post_recovery_registered_observation_passed": any(
            row.get("observation_phase") == "post_recovery"
            and row.get("status") == "pass"
            and row.get("source") == "product_visible_completion_contract"
            and row.get("verify_command_count") == 3
            for row in preflight
        ),
        "treatment_promoted": len(promotions) == 1
        and promotions[0].get("decision") == "promoted",
        "discarded_valid_treatment_zero": not _event(rows, "recovery_control_retained"),
        "recovery_completed": len(completions) == 1
        and completions[0].get("recovery_plan_auto_run_stop_reason")
        == "recovery_succeeded",
        "terminal_full_success": terminal.get("completion_status") == "complete"
        and terminal.get("final_acceptance_status") == "full_success"
        and terminal.get("assurance_level") == "full"
        and terminal.get("ok") is True,
        "registered_observations_are_product_internal": all(
            row.get("external_oracle_used") is False for row in observation_events
        ),
        "final_pipeline_repaired": final_pipeline is not None
        and '"used_rows": len(valid_rows),' in final_pipeline,
        "diagnostic_commands_passed": diagnostic_returncodes
        == {
            DATA_REPRO_COMMAND: 0,
            DATA_REGRESSION_COMMANDS[0]: 0,
            DATA_REGRESSION_COMMANDS[1]: 0,
        },
    }
    ready = all(checks.values())
    return {
        "schema_version": (
            "commandagent.goal_verify.recovery_data_fix_deterministic_smoke.v1"
        ),
        "inference_role": "instrument_path_coverage_only",
        "effect_claim_allowed": False,
        "provider": "local_scripted_ollama_compatible",
        "scenario": "data-fix",
        "binary_sha256": binary_sha256,
        "event_count": len(rows),
        "provider_request_count": len(provider_trace),
        "diagnostic_returncodes": diagnostic_returncodes,
        "checks": checks,
        "instrument_ready": ready,
        "go_no_go": "GO" if ready else "NO-GO",
    }


def build_generic_fix_report(
    *,
    rows: list[dict[str, Any]],
    returncode: int,
    final_app: str | None,
    provider_trace: list[dict[str, Any]],
    binary_sha256: str,
    diagnostic_returncode: int,
) -> dict[str, Any]:
    preflight = _event(rows, "recovery_preflight_observation")
    scopes = _event(rows, "step_obligation_scope")
    safety = _event(rows, "recovery_fix_safety_verification")
    host_final = _event(rows, "recovery_host_final_success_verification_passed")
    promotions = _event(rows, "recovery_promotion_decision")
    completions = _event(rows, "recovery_plan_auto_run_complete")
    deltas = _event(rows, "recovery_treatment_delta")
    fix_evidence = _event(rows, "fix_evidence_recorded")
    checks = {
        "process_exit_zero": returncode == 0,
        "initial_reproducer_failed": any(
            row.get("requirement_id") == "before_fails"
            and row.get("binding_id") == GENERIC_FIX_REPRO_COMMAND
            and row.get("executed") is True
            and row.get("outcome") == "failure"
            for row in fix_evidence
        ),
        "pre_recovery_registered_observation_failed": any(
            row.get("observation_phase") == "pre_recovery"
            and row.get("status") == "fail"
            and row.get("source") == "product_visible_completion_contract"
            for row in preflight
        ),
        "recovery_implement_contract_verification_deferred": any(
            row.get("step_kind") == "implement"
            and row.get("completion_contract_verification_enabled") is False
            for row in scopes
        ),
        "scripted_read_write_sequence": all(
            kind in [row["response_kind"] for row in provider_trace]
            for kind in ("Read", "Write")
        ),
        "app_treatment_delta_observed": any(
            "app.py" in row.get("attempted_product_delta", {}).get("changed_paths", [])
            for row in deltas
        ),
        "fix_safety_verification_passed": any(
            row.get("ok") is True
            and row.get("registered_verify_commands") == [GENERIC_FIX_REPRO_COMMAND]
            for row in safety
        ),
        "host_final_registered_reproducer_passed": any(
            row.get("model_execution_skipped") is True
            and row.get("registered_verify_commands") == [GENERIC_FIX_REPRO_COMMAND]
            for row in host_final
        ),
        "after_reproducer_passed": any(
            row.get("requirement_id") == "after_passes"
            and row.get("binding_id") == GENERIC_FIX_REPRO_COMMAND
            and row.get("executed") is True
            and row.get("outcome") == "success"
            for row in fix_evidence
        ),
        "post_recovery_registered_observation_passed": any(
            row.get("observation_phase") == "post_recovery"
            and row.get("status") == "pass"
            and row.get("source") == "product_visible_completion_contract"
            for row in preflight
        ),
        "treatment_promoted": len(promotions) == 1
        and promotions[0].get("decision") == "promoted",
        "recovery_completed": len(completions) == 1
        and completions[0].get("recovery_plan_auto_run_stop_reason")
        == "recovery_succeeded",
        "final_app_repaired": final_app is not None
        and 'item.get("amount", 0)' in final_app,
        "diagnostic_reproducer_passed": diagnostic_returncode == 0,
    }
    ready = all(checks.values())
    return {
        "schema_version": (
            "commandagent.goal_verify.recovery_generic_fix_deterministic_smoke.v1"
        ),
        "inference_role": "instrument_path_coverage_only",
        "effect_claim_allowed": False,
        "provider": "local_scripted_ollama_compatible",
        "scenario": "generic-fix",
        "binary_sha256": binary_sha256,
        "event_count": len(rows),
        "provider_request_count": len(provider_trace),
        "diagnostic_returncode": diagnostic_returncode,
        "checks": checks,
        "instrument_ready": ready,
        "go_no_go": "GO" if ready else "NO-GO",
    }


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while chunk := handle.read(64 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def run_smoke(
    *,
    commandagent_bin: Path,
    run_dir: Path,
    execution_root: Path | None = None,
    timeout_sec: int = 60,
    scenario: str = "generic-create",
) -> dict[str, Any]:
    if scenario not in {"generic-create", "generic-fix", "data-fix"}:
        raise ValueError(f"unsupported deterministic Recovery scenario:{scenario}")
    commandagent_bin = commandagent_bin.resolve()
    if not commandagent_bin.is_file():
        raise ValueError(f"commandagent binary is missing:{commandagent_bin}")
    run_dir = run_dir.resolve()
    run_dir.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(
        prefix="commandagent-recovery-deterministic-",
        dir=execution_root,
    ) as temporary:
        workspace = Path(temporary)
        if scenario == "data-fix":
            initial_plan, completion_contract, corrected_pipeline = (
                _write_data_fix_fixture(workspace)
            )
            provider = ScriptedDataFixRecoveryProvider(corrected_pipeline)
            intent = "fix"
            profile = "data"
            model = "scripted-data-recovery"
        elif scenario == "generic-fix":
            initial_plan, completion_contract, corrected_app = (
                _write_generic_fix_fixture(workspace)
            )
            provider = ScriptedGenericFixRecoveryProvider(corrected_app)
            intent = "fix"
            profile = "generic"
            model = "scripted-generic-fix-recovery"
        else:
            initial_plan, completion_contract = _write_fixture(workspace)
            provider = ScriptedRecoveryProvider()
            intent = "create"
            profile = "generic"
            model = "scripted-recovery"
        server = _ProviderServer(provider)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            host, port = server.server_address
            argv = [
                str(commandagent_bin),
                "--cwd",
                str(workspace),
                "--state-dir",
                str(workspace / ".commandagent-state"),
                "--offline",
                "--yes",
                "--intent",
                intent,
                "--profile",
                profile,
                "--provider",
                "ollama",
                "--model",
                model,
                "--ollama-host",
                f"http://{host}:{port}",
                "--completion-contract-json",
                str(completion_contract),
                "--recovery-plan-auto-runs",
                "1",
                "--run-ultra-plan",
                str(initial_plan),
            ]
            completed = subprocess.run(
                argv,
                cwd=workspace,
                stdin=subprocess.DEVNULL,
                text=True,
                capture_output=True,
                timeout=timeout_sec,
                check=False,
            )
            event_paths = sorted(workspace.glob(".commandagent/runs/*/events.jsonl"))
            if len(event_paths) != 1:
                raise ValueError(
                    f"expected exactly one product event log, found {len(event_paths)}"
                )
            events_path = event_paths[0]
            rows = _rows(events_path)
            shutil.copyfile(events_path, run_dir / "events.jsonl")
            (run_dir / "stdout.txt").write_text(completed.stdout, encoding="utf-8")
            (run_dir / "stderr.txt").write_text(completed.stderr, encoding="utf-8")
            (run_dir / "provider-trace.json").write_text(
                json.dumps(provider.trace, indent=2, sort_keys=True) + "\n",
                encoding="utf-8",
            )
            evidence_names = [
                "events.jsonl",
                "provider-trace.json",
                "stderr.txt",
                "stdout.txt",
            ]
            if scenario == "data-fix":
                diagnostics = {
                    DATA_REPRO_COMMAND: subprocess.run(
                        ["python3", "scripts/repro.py", "data/task-02.csv"],
                        cwd=workspace,
                        stdin=subprocess.DEVNULL,
                        text=True,
                        capture_output=True,
                        timeout=timeout_sec,
                        check=False,
                    ).returncode,
                    DATA_REGRESSION_COMMANDS[0]: subprocess.run(
                        ["python3", "-m", "pytest", "-q", "tests"],
                        cwd=workspace,
                        stdin=subprocess.DEVNULL,
                        text=True,
                        capture_output=True,
                        timeout=timeout_sec,
                        check=False,
                    ).returncode,
                    DATA_REGRESSION_COMMANDS[1]: subprocess.run(
                        ["python3", "scripts/contract_check.py"],
                        cwd=workspace,
                        stdin=subprocess.DEVNULL,
                        text=True,
                        capture_output=True,
                        timeout=timeout_sec,
                        check=False,
                    ).returncode,
                }
                (run_dir / "diagnostic-returncodes.json").write_text(
                    json.dumps(diagnostics, indent=2, sort_keys=True) + "\n",
                    encoding="utf-8",
                )
                evidence_names.append("diagnostic-returncodes.json")
                pipeline_path = workspace / "pipeline/main.py"
                report = build_data_fix_report(
                    rows=rows,
                    returncode=completed.returncode,
                    final_pipeline=(
                        pipeline_path.read_text(encoding="utf-8")
                        if pipeline_path.is_file()
                        else None
                    ),
                    provider_trace=provider.trace,
                    binary_sha256=_sha256(commandagent_bin),
                    diagnostic_returncodes=diagnostics,
                )
            elif scenario == "generic-fix":
                diagnostic_returncode = subprocess.run(
                    ["python3", "app.py", "fixture/task-02.json"],
                    cwd=workspace,
                    stdin=subprocess.DEVNULL,
                    text=True,
                    capture_output=True,
                    timeout=timeout_sec,
                    check=False,
                ).returncode
                (run_dir / "diagnostic-returncode.json").write_text(
                    json.dumps(
                        {GENERIC_FIX_REPRO_COMMAND: diagnostic_returncode},
                        indent=2,
                        sort_keys=True,
                    )
                    + "\n",
                    encoding="utf-8",
                )
                evidence_names.append("diagnostic-returncode.json")
                app_path = workspace / "app.py"
                report = build_generic_fix_report(
                    rows=rows,
                    returncode=completed.returncode,
                    final_app=(
                        app_path.read_text(encoding="utf-8")
                        if app_path.is_file()
                        else None
                    ),
                    provider_trace=provider.trace,
                    binary_sha256=_sha256(commandagent_bin),
                    diagnostic_returncode=diagnostic_returncode,
                )
            else:
                artifact_path = workspace / "result.txt"
                report = build_report(
                    rows=rows,
                    returncode=completed.returncode,
                    final_artifact=(
                        artifact_path.read_text(encoding="utf-8")
                        if artifact_path.is_file()
                        else None
                    ),
                    provider_trace=provider.trace,
                    binary_sha256=_sha256(commandagent_bin),
                )
            report["evidence_sha256"] = {
                name: _sha256(run_dir / name) for name in evidence_names
            }
            (run_dir / "report.json").write_text(
                json.dumps(report, indent=2, sort_keys=True) + "\n",
                encoding="utf-8",
            )
            return report
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)
