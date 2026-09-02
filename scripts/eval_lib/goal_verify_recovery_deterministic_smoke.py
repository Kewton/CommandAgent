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
                response, response_kind = self._execution_response()
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

    def _execution_response(self) -> tuple[dict[str, Any], str]:
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
) -> dict[str, Any]:
    commandagent_bin = commandagent_bin.resolve()
    if not commandagent_bin.is_file():
        raise ValueError(f"commandagent binary is missing:{commandagent_bin}")
    run_dir = run_dir.resolve()
    run_dir.mkdir(parents=True, exist_ok=False)
    provider = ScriptedRecoveryProvider()
    server = _ProviderServer(provider)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        with tempfile.TemporaryDirectory(
            prefix="commandagent-recovery-deterministic-",
            dir=execution_root,
        ) as temporary:
            workspace = Path(temporary)
            initial_plan, completion_contract = _write_fixture(workspace)
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
                "create",
                "--profile",
                "generic",
                "--provider",
                "ollama",
                "--model",
                "scripted-recovery",
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
            artifact_path = workspace / "result.txt"
            final_artifact = (
                artifact_path.read_text(encoding="utf-8")
                if artifact_path.is_file()
                else None
            )
            shutil.copyfile(events_path, run_dir / "events.jsonl")
            (run_dir / "stdout.txt").write_text(completed.stdout, encoding="utf-8")
            (run_dir / "stderr.txt").write_text(completed.stderr, encoding="utf-8")
            (run_dir / "provider-trace.json").write_text(
                json.dumps(provider.trace, indent=2, sort_keys=True) + "\n",
                encoding="utf-8",
            )
            report = build_report(
                rows=rows,
                returncode=completed.returncode,
                final_artifact=final_artifact,
                provider_trace=provider.trace,
                binary_sha256=_sha256(commandagent_bin),
            )
            report["evidence_sha256"] = {
                name: _sha256(run_dir / name)
                for name in (
                    "events.jsonl",
                    "provider-trace.json",
                    "stderr.txt",
                    "stdout.txt",
                )
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
