from __future__ import annotations

import hashlib
import json
import os
import shutil
import signal
import socket
import subprocess
import tempfile
import threading
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from html.parser import HTMLParser
from pathlib import Path
from typing import Any

from eval_lib.goal_verify_recovery_deterministic_smoke import (
    DATA_REGRESSION_COMMANDS,
    DATA_REPRO_COMMAND,
    GENERIC_FIX_REPRO_COMMAND,
    ScriptedDataFixRecoveryProvider,
    ScriptedGenericFixRecoveryProvider,
    ScriptedRecoveryProvider,
    _ProviderServer,
    _write_data_fix_fixture,
    _write_generic_fix_fixture,
)

ROOT = Path(__file__).resolve().parents[2]
A26_SCHEMA_VERSION = (
    "commandagent.goal_verify.recovery_deterministic_fault_boundary_pilot.v1"
)
A26_REPORT_SCHEMA_VERSION = (
    "commandagent.goal_verify.recovery_deterministic_fault_boundary_report.v1"
)
A26_SOURCE_PATHS = (
    "scripts/eval-goal-verify-recovery-deterministic-pair.py",
    "scripts/eval_lib/goal_verify_recovery_deterministic_pair.py",
    "scripts/eval_lib/goal_verify_recovery_deterministic_smoke.py",
)

NEXTJS_REPRO_COMMAND = "node scripts/repro.mjs fixture/task-02.json"
NEXTJS_REGRESSION_COMMANDS = (
    "node scripts/regression.mjs",
    "npx next build --webpack",
)
NEXTJS_VERIFY_COMMANDS = (NEXTJS_REPRO_COMMAND, *NEXTJS_REGRESSION_COMMANDS)
NEXTJS_FIXTURE = (
    ROOT / "tests/fixtures/goal_verify_v4/a15/fix-nextjs-route-label/before"
)
NEXTJS_REFERENCE = (
    ROOT / "tests/fixtures/goal_verify_v4/a15/fix-nextjs-route-label/after"
)
NEXTJS_GOAL = (
    "Fix the existing offline Next.js App Router project so "
    "`node scripts/repro.mjs fixture/task-02.json` changes from exit 1 to "
    "exit 0 and / renders #result-02 as ready-02. Preserve the frozen Node "
    "regression and complete a production build."
)
NEXTJS_PORT = 4185

SCENARIO_ORDER = ("generic-fix", "data-fix", "nextjs-fix")
ARM_ORDER = ("control", "treatment")

_RUNTIME_PARTS = {
    ".anvil",
    ".commandagent",
    ".commandagent-state",
    ".goal-verify-tools",
    ".next",
    ".pytest_cache",
    "__pycache__",
    "node_modules",
}


@dataclass(frozen=True)
class Scenario:
    scenario_id: str
    profile: str
    target_path: str
    verify_commands: tuple[str, ...]
    protected_paths: tuple[str, ...]


SCENARIOS = {
    "generic-fix": Scenario(
        scenario_id="generic-fix",
        profile="generic",
        target_path="app.py",
        verify_commands=(GENERIC_FIX_REPRO_COMMAND,),
        protected_paths=("fixture",),
    ),
    "data-fix": Scenario(
        scenario_id="data-fix",
        profile="data",
        target_path="pipeline/main.py",
        verify_commands=(DATA_REPRO_COMMAND, *DATA_REGRESSION_COMMANDS),
        protected_paths=(
            "data",
            "scripts/repro.py",
            "scripts/contract_check.py",
            "tests",
        ),
    ),
    "nextjs-fix": Scenario(
        scenario_id="nextjs-fix",
        profile="nextjs",
        target_path="lib/label.mjs",
        verify_commands=NEXTJS_VERIFY_COMMANDS,
        protected_paths=(
            "fixture",
            "scripts/repro.mjs",
            "scripts/regression.mjs",
            "package.json",
            "package-lock.json",
        ),
    ),
}


def _sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while chunk := handle.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def _canonical_sha256(value: Any) -> str:
    payload = json.dumps(
        value, ensure_ascii=False, separators=(",", ":"), sort_keys=True
    ).encode()
    return _sha256_bytes(payload)


def _manifest(root: Path, *, exclude_runtime: bool = True) -> dict[str, str]:
    rows: dict[str, str] = {}
    for path in sorted(root.rglob("*")):
        relative = path.relative_to(root)
        if exclude_runtime and any(part in _RUNTIME_PARTS for part in relative.parts):
            continue
        key = relative.as_posix()
        if path.is_symlink():
            rows[key] = "symlink:" + os.readlink(path)
        elif path.is_file():
            rows[key] = sha256_file(path)
    return rows


def manifest_sha256(root: Path, *, exclude_runtime: bool = True) -> str:
    return _canonical_sha256(_manifest(root, exclude_runtime=exclude_runtime))


def fixture_manifest_sha256(scenario_id: str) -> str:
    roots = {
        "generic-fix": ROOT
        / "tests/fixtures/goal_verify_v4/main/fix-generic-fixtures/before",
        "data-fix": ROOT
        / "tests/fixtures/goal_verify_v4/a15/fix-data-reconciliation/before",
        "nextjs-fix": NEXTJS_FIXTURE,
    }
    try:
        fixture = roots[scenario_id]
    except KeyError as error:
        raise ValueError(f"unsupported deterministic scenario:{scenario_id}") from error
    if not fixture.is_dir():
        raise ValueError(f"deterministic fixture is missing:{fixture}")
    return manifest_sha256(fixture)


def provisioning_manifest_sha256(node_modules: Path) -> str:
    node_modules = node_modules.resolve()
    if not (node_modules / ".package-lock.json").is_file():
        raise ValueError(
            f"Next.js node_modules provisioning is incomplete:{node_modules}"
        )
    return manifest_sha256(node_modules, exclude_runtime=False)


class ScriptedNextjsFixRecoveryProvider(ScriptedRecoveryProvider):
    """Scripted Next.js repair used only to exercise the paired instrument."""

    def __init__(self, corrected_label: str) -> None:
        super().__init__()
        self.corrected_label = corrected_label
        self.initial_inspected = False

    def _planner_response(self, text: str) -> dict[str, Any]:
        if "Inspect the current workspace before changing files" in text:
            self.phase = "nextjs_recovery_inspect"
            content = self._step_plan(
                "Inspect Next.js fix",
                "inspect-state",
                "inspect",
                "Inspect lib/label.mjs",
            )
        elif "Repair the incomplete work for the failed phase" in text:
            self.phase = "nextjs_recovery_repair"
            content = self._step_plan(
                "Repair Next.js fix",
                "repair-label",
                "implement",
                "Repair lib/label.mjs",
                expected_paths=["lib/label.mjs"],
                verify=[NEXTJS_REPRO_COMMAND],
            )
        elif "Verify the recovered output with deterministic checks" in text:
            self.phase = "nextjs_recovery_verify"
            content = self._step_plan(
                "Verify Next.js fix",
                "verify-nextjs-fix",
                "verify",
                "Verify the registered Next.js commands",
                verify=list(NEXTJS_VERIFY_COMMANDS),
            )
        else:
            self.phase = "nextjs_initial"
            content = self._step_plan(
                "Inspect Next.js fix",
                "inspect-state",
                "inspect",
                "Inspect lib/label.mjs",
            )
        return {"content": content}

    @staticmethod
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

    def _execution_response(self, text: str) -> tuple[dict[str, Any], str]:
        if self.phase == "nextjs_recovery_inspect" and not self.inspected:
            self.inspected = True
            return self._tool("Read", {"path": "lib/label.mjs"}), "Read"
        if self.phase == "nextjs_recovery_repair" and not self.wrote:
            self.wrote = True
            return (
                self._tool(
                    "Write",
                    {
                        "path": "lib/label.mjs",
                        "content": self.corrected_label,
                    },
                ),
                "Write",
            )
        if self.phase == "nextjs_recovery_verify" and not self.verified:
            self.verified = True
            return self._tool("Bash", {"command": NEXTJS_REPRO_COMMAND}), "Bash"
        if "Read only the executed runtime-bound F1 failure evidence" in text:
            if not self.initial_inspected:
                self.initial_inspected = True
                return self._tool("Read", {"path": "lib/label.mjs"}), "Read"
            return {"content": "Cause isolated."}, "complete"
        if (
            "Repair the F1-diagnosed defect" in text
            or "Fix F1 failure diagnostic" in text
        ):
            return (
                {"content": "Initial repair intentionally made no edit."},
                "intentional_no_tool",
            )
        return {"content": "Scripted Next.js Recovery step complete."}, "complete"

    @staticmethod
    def _tool(name: str, arguments: dict[str, Any]) -> dict[str, Any]:
        return {
            "content": "",
            "tool_calls": [{"function": {"name": name, "arguments": arguments}}],
        }


def _write_nextjs_fix_fixture(
    workspace: Path, node_modules_source: Path
) -> tuple[Path, Path, str]:
    if not NEXTJS_FIXTURE.is_dir() or not NEXTJS_REFERENCE.is_dir():
        raise ValueError("Next.js deterministic fixture is missing")
    shutil.copytree(
        NEXTJS_FIXTURE,
        workspace,
        dirs_exist_ok=True,
        ignore=shutil.ignore_patterns(".next", "node_modules"),
    )
    shutil.copytree(node_modules_source, workspace / "node_modules")
    corrected_label = (NEXTJS_REFERENCE / "lib/label.mjs").read_text(encoding="utf-8")
    if corrected_label == (workspace / "lib/label.mjs").read_text(encoding="utf-8"):
        raise ValueError("Next.js fixture no longer contains the expected defect")
    initial_plan = workspace / "initial.yaml"
    initial_plan.write_text(
        "\n".join(
            [
                f"goal: {json.dumps(NEXTJS_GOAL)}",
                'profile: "nextjs"',
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
                '    prompt: "Verify the registered reproducer, regressions, and build."',
                "",
            ]
        ),
        encoding="utf-8",
    )
    completion_contract = workspace / "completion.json"
    completion_contract.write_text(
        json.dumps(
            {
                "goal": NEXTJS_GOAL,
                "profile": "nextjs",
                "protected_paths": list(SCENARIOS["nextjs-fix"].protected_paths),
                "required_paths": [
                    "package.json",
                    "package-lock.json",
                    "next.config.js",
                    "app/layout.js",
                    "app/page.js",
                    "lib/label.mjs",
                    "fixture/task-02.json",
                    "scripts/repro.mjs",
                    "scripts/regression.mjs",
                ],
                "verify_commands": list(NEXTJS_VERIFY_COMMANDS),
                "fix_reproducer_command": NEXTJS_REPRO_COMMAND,
                "required_capabilities": [],
                "required_evidence": [
                    "implementation_artifact",
                    "test_artifact",
                    "bound_verify_command",
                    "nextjs_route_evidence",
                    "build_command_or_dependency_missing_boundary",
                ],
                "required_obligations": [
                    "implementation",
                    "verification",
                    "acceptance_evidence",
                ],
                "deferred_verify_requirements": [],
                "evidence_hint_tokens": [
                    "fixture/task-02.json",
                    "#result-02",
                    "production-build",
                ],
                "verify_repair_cap": 1,
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    return initial_plan, completion_contract, corrected_label


def _prepare(
    scenario: Scenario, workspace: Path, node_modules_source: Path
) -> tuple[Path, Path, ScriptedRecoveryProvider, str]:
    if scenario.scenario_id == "generic-fix":
        initial, completion, corrected = _write_generic_fix_fixture(workspace)
        return (
            initial,
            completion,
            ScriptedGenericFixRecoveryProvider(corrected),
            "scripted-a26-generic-recovery",
        )
    if scenario.scenario_id == "data-fix":
        initial, completion, corrected = _write_data_fix_fixture(workspace)
        return (
            initial,
            completion,
            ScriptedDataFixRecoveryProvider(corrected),
            "scripted-a26-data-recovery",
        )
    initial, completion, corrected = _write_nextjs_fix_fixture(
        workspace, node_modules_source
    )
    return (
        initial,
        completion,
        ScriptedNextjsFixRecoveryProvider(corrected),
        "scripted-a26-nextjs-recovery",
    )


def _rows(path: Path) -> list[dict[str, Any]]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]


def _events(rows: list[dict[str, Any]], name: str) -> list[dict[str, Any]]:
    return [row for row in rows if row.get("event") == name]


def _path_manifest(root: Path, relative_texts: tuple[str, ...]) -> dict[str, str]:
    rows: dict[str, str] = {}
    for relative_text in relative_texts:
        relative = Path(relative_text)
        if relative.is_absolute() or ".." in relative.parts:
            raise ValueError(f"unsafe protected path:{relative_text}")
        target = root / relative
        if target.is_file():
            rows[relative.as_posix()] = sha256_file(target)
        elif target.is_dir():
            for path in sorted(target.rglob("*")):
                if path.is_file() and not path.is_symlink():
                    rows[path.relative_to(root).as_posix()] = sha256_file(path)
    return rows


def _command_result(
    command: str, *, workspace: Path, timeout_sec: int, env: dict[str, str]
) -> dict[str, Any]:
    completed = subprocess.run(
        command.split(),
        cwd=workspace,
        env=env,
        stdin=subprocess.DEVNULL,
        text=True,
        capture_output=True,
        timeout=timeout_sec,
        check=False,
    )
    return {
        "command": command,
        "returncode": completed.returncode,
        "stdout_sha256": _sha256_bytes(completed.stdout.encode()),
        "stderr_sha256": _sha256_bytes(completed.stderr.encode()),
        "stdout_tail": completed.stdout[-1000:],
        "stderr_tail": completed.stderr[-1000:],
    }


class _TargetTextParser(HTMLParser):
    def __init__(self, target_id: str) -> None:
        super().__init__()
        self.target_id = target_id
        self.depth = 0
        self.parts: list[str] = []

    def handle_starttag(self, _tag: str, attrs: list[tuple[str, str | None]]) -> None:
        if self.depth:
            self.depth += 1
        elif dict(attrs).get("id") == self.target_id:
            self.depth = 1

    def handle_endtag(self, _tag: str) -> None:
        if self.depth:
            self.depth -= 1

    def handle_data(self, data: str) -> None:
        if self.depth:
            self.parts.append(data)

    def text(self) -> str | None:
        value = "".join(self.parts).strip()
        return value or None


def _route_observation(
    workspace: Path, *, timeout_sec: int, env: dict[str, str]
) -> dict[str, Any]:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as probe:
        probe.bind(("127.0.0.1", NEXTJS_PORT))
    process = subprocess.Popen(
        ["npx", "next", "start", "-p", str(NEXTJS_PORT)],
        cwd=workspace,
        env=env,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        start_new_session=True,
    )
    html = None
    error = None
    deadline = time.monotonic() + min(timeout_sec, 30)
    try:
        while time.monotonic() < deadline:
            if process.poll() is not None:
                error = f"next_start_exited:{process.returncode}"
                break
            try:
                with urllib.request.urlopen(
                    f"http://127.0.0.1:{NEXTJS_PORT}/", timeout=1
                ) as response:
                    html = response.read().decode("utf-8", errors="replace")
                break
            except (urllib.error.URLError, TimeoutError):
                time.sleep(0.2)
        if html is None and error is None:
            error = "next_start_readiness_timeout"
    finally:
        if process.poll() is None:
            os.killpg(process.pid, signal.SIGTERM)
        try:
            stdout, stderr = process.communicate(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            stdout, stderr = process.communicate(timeout=5)
    parser = _TargetTextParser("result-02")
    if html is not None:
        parser.feed(html)
    return {
        "error": error,
        "http_observed": html is not None,
        "target_id": "result-02",
        "target_text": parser.text() if html is not None else None,
        "html_sha256": _sha256_bytes(html.encode()) if html is not None else None,
        "stdout_sha256": _sha256_bytes(stdout.encode()),
        "stderr_sha256": _sha256_bytes(stderr.encode()),
        "stderr_tail": stderr[-1000:],
    }


def _boundary_signature(
    rows: list[dict[str, Any]], scenario: Scenario
) -> dict[str, Any]:
    before = [
        {
            "requirement_id": row.get("requirement_id"),
            "binding_id": row.get("binding_id"),
            "executed": row.get("executed"),
            "outcome": row.get("outcome"),
        }
        for row in _events(rows, "fix_evidence_recorded")
        if row.get("requirement_id") == "before_fails"
    ]
    prompts = [
        {"status": row.get("status"), "failed_phase": row.get("failed_phase")}
        for row in _events(rows, "recovery_prompt_saved")
    ]
    initial_acceptance = _events(rows, "ultra_final_acceptance")[:1]
    acceptance = [
        {
            "ok": row.get("ok"),
            "verdict": row.get("verdict"),
            "final_acceptance_status": row.get("final_acceptance_status"),
            "assurance_level": row.get("assurance_level"),
        }
        for row in initial_acceptance
    ]
    return {
        "scenario_id": scenario.scenario_id,
        "profile": scenario.profile,
        "registered_verify_commands": list(scenario.verify_commands),
        "before_fails": before,
        "recovery_prompts": prompts,
        "initial_acceptance": acceptance,
    }


def _build_arm_report(
    *,
    scenario: Scenario,
    arm: str,
    recovery_auto_runs: int,
    rows: list[dict[str, Any]],
    process_returncode: int,
    provider_trace: list[dict[str, Any]],
    input_manifest: dict[str, str],
    final_manifest: dict[str, str],
    input_protected_manifest: dict[str, str],
    final_protected_manifest: dict[str, str],
    diagnostics: list[dict[str, Any]],
    route: dict[str, Any] | None,
    binary_sha256: str,
) -> dict[str, Any]:
    config = _events(rows, "recovery_plan_auto_run_configured")
    starts = _events(rows, "recovery_plan_auto_run_start")
    deltas = _events(rows, "recovery_treatment_delta")
    promotions = _events(rows, "recovery_promotion_decision")
    completions = _events(rows, "recovery_plan_auto_run_complete")
    preflight = _events(rows, "recovery_preflight_observation")
    fix_evidence = _events(rows, "fix_evidence_recorded")
    safety = _events(rows, "recovery_fix_safety_verification")
    host_final = _events(rows, "recovery_host_final_success_verification_passed")
    target = scenario.target_path
    before_fails = any(
        row.get("requirement_id") == "before_fails"
        and row.get("binding_id") == scenario.verify_commands[0]
        and row.get("executed") is True
        and row.get("outcome") == "failure"
        for row in fix_evidence
    )
    diagnostics_by_command = {row["command"]: row["returncode"] for row in diagnostics}
    route_required = scenario.scenario_id == "nextjs-fix"
    route_success = not route_required or (
        route is not None
        and route.get("http_observed") is True
        and route.get("target_text") == "ready-02"
    )
    route_failure = not route_required or (
        route is not None
        and route.get("http_observed") is True
        and route.get("target_text") == "stale-02"
    )
    target_unchanged = input_manifest.get(target) == final_manifest.get(target)
    target_changed = (
        target in input_manifest
        and target in final_manifest
        and input_manifest[target] != final_manifest[target]
    )
    product_configured_runs = (
        config[-1].get("recovery_plan_auto_runs") if config else None
    )
    common_checks = {
        "initial_registered_reproducer_failed": before_fails,
        "protected_paths_unchanged": (
            input_protected_manifest == final_protected_manifest
        ),
        "binary_bound": len(binary_sha256) == 64,
    }
    if arm == "control":
        checks = {
            **common_checks,
            "recovery_disabled_by_harness": recovery_auto_runs == 0,
            "product_reports_no_enabled_recovery": product_configured_runs in (None, 0),
            "no_recovery_started": not starts,
            "no_treatment_delta": not deltas,
            "no_treatment_promotion": not promotions,
            "repair_target_unchanged": target_unchanged,
            "registered_reproducer_remained_failed": (
                diagnostics_by_command.get(scenario.verify_commands[0]) == 1
            ),
            "registered_regressions_passed": all(
                diagnostics_by_command.get(command) == 0
                for command in scenario.verify_commands[1:]
            ),
            "route_endpoint_remained_failed": route_failure,
        }
        endpoint_success = False
    else:
        registered_commands = list(scenario.verify_commands)
        after_passed = any(
            row.get("requirement_id") == "after_passes"
            and row.get("binding_id") == scenario.verify_commands[0]
            and row.get("executed") is True
            and row.get("outcome") == "success"
            for row in fix_evidence
        )
        product_observations = preflight + promotions
        checks = {
            **common_checks,
            "one_recovery_enabled_by_harness": recovery_auto_runs == 1,
            "product_records_one_recovery_enabled": product_configured_runs == 1,
            "exactly_one_recovery_started": len(starts) == 1,
            "recovery_boundary_captured": any(
                row.get("status") == "captured"
                for row in _events(rows, "recovery_boundary_snapshot")
            ),
            "scripted_read_write_sequence": all(
                kind in [row.get("response_kind") for row in provider_trace]
                for kind in ("Read", "Write")
            ),
            "repair_target_changed": target_changed,
            "registered_fix_safety_passed": any(
                row.get("ok") is True
                and row.get("registered_verify_commands") == registered_commands
                for row in safety
            ),
            "host_final_registered_commands_passed": any(
                row.get("model_execution_skipped") is True
                and row.get("registered_verify_commands") == registered_commands
                for row in host_final
            ),
            "after_registered_reproducer_passed": after_passed,
            "post_recovery_registered_observation_passed": any(
                row.get("observation_phase") == "post_recovery"
                and row.get("status") == "pass"
                and row.get("source") == "product_visible_completion_contract"
                for row in preflight
            ),
            "product_observations_did_not_use_host_oracle": all(
                row.get("external_oracle_used") is False for row in product_observations
            ),
            "treatment_promoted": len(promotions) == 1
            and promotions[0].get("decision") == "promoted",
            "recovery_completed": len(completions) == 1
            and completions[0].get("recovery_plan_auto_run_stop_reason")
            == "recovery_succeeded",
            "all_registered_commands_passed": all(
                diagnostics_by_command.get(command) == 0
                for command in scenario.verify_commands
            ),
            "route_endpoint_passed": route_success,
        }
        endpoint_success = checks["all_registered_commands_passed"] and route_success
    return {
        "arm": arm,
        "assigned_recovery_auto_runs": recovery_auto_runs,
        "scenario_id": scenario.scenario_id,
        "profile": scenario.profile,
        "binary_sha256": binary_sha256,
        "process_returncode": process_returncode,
        "process_returncode_is_endpoint": False,
        "input_snapshot_sha256": _canonical_sha256(input_manifest),
        "final_snapshot_sha256": _canonical_sha256(final_manifest),
        "boundary_signature": _boundary_signature(rows, scenario),
        "boundary_signature_sha256": _canonical_sha256(
            _boundary_signature(rows, scenario)
        ),
        "registered_endpoint_success": endpoint_success,
        "diagnostic_returncodes": diagnostics_by_command,
        "route_observation": route,
        "event_count": len(rows),
        "provider_request_count": len(provider_trace),
        "checks": checks,
        "arm_valid": all(checks.values()),
    }


def _run_arm(
    *,
    commandagent_bin: Path,
    scenario: Scenario,
    arm: str,
    output_dir: Path,
    execution_root: Path | None,
    node_modules_source: Path,
    timeout_sec: int,
) -> dict[str, Any]:
    output_dir.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(
        prefix=f"commandagent-a26-{scenario.scenario_id}-{arm}-",
        dir=execution_root,
    ) as temporary:
        workspace = Path(temporary)
        initial_plan, completion_contract, provider, model = _prepare(
            scenario, workspace, node_modules_source
        )
        input_manifest = _manifest(workspace)
        input_protected = _path_manifest(workspace, scenario.protected_paths)
        (output_dir / "input-manifest.json").write_text(
            json.dumps(input_manifest, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        server = _ProviderServer(provider)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        env = os.environ.copy()
        env.update(
            {
                "npm_config_offline": "true",
                "NEXT_TELEMETRY_DISABLED": "1",
                "HTTP_PROXY": "http://127.0.0.1:9",
                "HTTPS_PROXY": "http://127.0.0.1:9",
                "ALL_PROXY": "http://127.0.0.1:9",
                "NO_PROXY": "127.0.0.1,localhost",
            }
        )
        try:
            host, port = server.server_address
            completed = subprocess.run(
                [
                    str(commandagent_bin),
                    "--cwd",
                    str(workspace),
                    "--state-dir",
                    str(workspace / ".commandagent-state"),
                    "--offline",
                    "--yes",
                    "--intent",
                    "fix",
                    "--profile",
                    scenario.profile,
                    "--provider",
                    "ollama",
                    "--model",
                    model,
                    "--ollama-host",
                    f"http://{host}:{port}",
                    "--completion-contract-json",
                    str(completion_contract),
                    "--recovery-plan-auto-runs",
                    "0" if arm == "control" else "1",
                    "--run-ultra-plan",
                    str(initial_plan),
                ],
                cwd=workspace,
                env=env,
                stdin=subprocess.DEVNULL,
                text=True,
                capture_output=True,
                timeout=timeout_sec,
                check=False,
            )
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)
        event_paths = sorted(workspace.glob(".commandagent/runs/*/events.jsonl"))
        if len(event_paths) != 1:
            raise ValueError(
                f"expected exactly one event log for {scenario.scenario_id}/{arm}, "
                f"found {len(event_paths)}"
            )
        events_path = event_paths[0]
        rows = _rows(events_path)
        shutil.copyfile(events_path, output_dir / "events.jsonl")
        (output_dir / "stdout.txt").write_text(completed.stdout, encoding="utf-8")
        (output_dir / "stderr.txt").write_text(completed.stderr, encoding="utf-8")
        (output_dir / "provider-trace.json").write_text(
            json.dumps(provider.trace, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        diagnostics = [
            _command_result(
                command, workspace=workspace, timeout_sec=timeout_sec, env=env
            )
            for command in scenario.verify_commands
        ]
        route = (
            _route_observation(workspace, timeout_sec=timeout_sec, env=env)
            if scenario.scenario_id == "nextjs-fix"
            and diagnostics[-1]["returncode"] == 0
            else None
        )
        (output_dir / "diagnostics.json").write_text(
            json.dumps(
                {"commands": diagnostics, "route_observation": route},
                indent=2,
                sort_keys=True,
            )
            + "\n",
            encoding="utf-8",
        )
        final_manifest = _manifest(workspace)
        final_protected = _path_manifest(workspace, scenario.protected_paths)
        report = _build_arm_report(
            scenario=scenario,
            arm=arm,
            recovery_auto_runs=0 if arm == "control" else 1,
            rows=rows,
            process_returncode=completed.returncode,
            provider_trace=provider.trace,
            input_manifest=input_manifest,
            final_manifest=final_manifest,
            input_protected_manifest=input_protected,
            final_protected_manifest=final_protected,
            diagnostics=diagnostics,
            route=route,
            binary_sha256=sha256_file(commandagent_bin),
        )
        (output_dir / "arm-report.json").write_text(
            json.dumps(report, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        return report


def contract_errors(contract: dict[str, Any]) -> list[str]:
    errors = []
    if contract.get("schema_version") != A26_SCHEMA_VERSION:
        errors.append("schema_version_invalid")
    if contract.get("status") != "frozen":
        errors.append("contract_not_frozen")
    if contract.get("effect_claim_allowed") is not False:
        errors.append("effect_claim_must_be_forbidden")
    for field in (
        "generalization_claim_allowed",
        "default_rollout_allowed",
        "full_collection_allowed",
    ):
        if contract.get(field) is not False:
            errors.append(f"{field}_must_be_forbidden")
    authorization = contract.get("authorization", {})
    if authorization.get("pilot_collection_authorized") is not True:
        errors.append("pilot_collection_not_authorized")
    if authorization.get("confirmatory_collection_authorized") is not False:
        errors.append("confirmatory_collection_must_be_forbidden")
    design = contract.get("design", {})
    if design.get("scenario_order") != list(SCENARIO_ORDER):
        errors.append("scenario_order_invalid")
    if design.get("arm_order") != list(ARM_ORDER):
        errors.append("arm_order_invalid")
    if design.get("recovery_auto_runs") != {"control": 0, "treatment": 1}:
        errors.append("arm_assignment_invalid")
    if design.get("required_profiles") != ["generic", "data", "nextjs"]:
        errors.append("required_profiles_invalid")
    for field in (
        "fresh_workspace_per_arm",
        "same_input_snapshot_required_within_pair",
        "same_initial_failure_boundary_required_within_pair",
        "same_binary_required_for_all_arms",
    ):
        if design.get(field) is not True:
            errors.append(f"{field}_must_be_required")
    if design.get("process_returncode_is_endpoint") is not False:
        errors.append("process_returncode_must_not_be_endpoint")
    estimand = contract.get("estimand", {})
    for field in (
        "natural_operation_effect",
        "cross_profile_generalization",
        "confirmatory_effect_estimate_in_a26",
    ):
        if estimand.get(field) is not False:
            errors.append(f"estimand_{field}_must_be_false")
    configured = contract.get("scenarios")
    if not isinstance(configured, list) or [
        row.get("scenario_id") for row in configured if isinstance(row, dict)
    ] != list(SCENARIO_ORDER):
        errors.append("scenarios_invalid")
    else:
        for row in configured:
            scenario = SCENARIOS[row["scenario_id"]]
            if (
                row.get("profile") != scenario.profile
                or row.get("target_path") != scenario.target_path
                or row.get("protected_paths") != list(scenario.protected_paths)
                or row.get("verify_commands") != list(scenario.verify_commands)
                or row.get("fixture_manifest_sha256")
                != fixture_manifest_sha256(scenario.scenario_id)
            ):
                errors.append(f"scenario_binding_invalid:{scenario.scenario_id}")
            route = row.get("host_route_endpoint")
            if scenario.scenario_id == "nextjs-fix":
                if route != {
                    "path": "/",
                    "port": NEXTJS_PORT,
                    "selector": "#result-02",
                    "control_expected_text": "stale-02",
                    "treatment_expected_text": "ready-02",
                }:
                    errors.append("nextjs_route_endpoint_invalid")
            elif route is not None:
                errors.append(f"unexpected_route_endpoint:{scenario.scenario_id}")
    source_hashes = contract.get("authoritative_source_sha256")
    if not isinstance(source_hashes, dict):
        errors.append("authoritative_source_hashes_missing")
    else:
        for relative in A26_SOURCE_PATHS:
            path = ROOT / relative
            if not path.is_file() or source_hashes.get(relative) != sha256_file(path):
                errors.append(f"authoritative_source_hash_invalid:{relative}")
    evidence_relative = contract.get("exact_sha_ci_evidence")
    if not isinstance(evidence_relative, str):
        errors.append("exact_sha_ci_evidence_missing")
    else:
        evidence_path = ROOT / evidence_relative
        if not evidence_path.is_file():
            errors.append("exact_sha_ci_evidence_missing")
        else:
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            workflows = evidence.get("workflows", [])
            if evidence.get("head_sha") != contract.get("code_sha") or not all(
                row.get("head_sha") == contract.get("code_sha")
                and row.get("status") == "completed"
                and row.get("conclusion") == "success"
                for row in workflows
                if isinstance(row, dict)
            ):
                errors.append("exact_sha_ci_evidence_invalid")
            names = {row.get("name") for row in workflows if isinstance(row, dict)}
            if names != {"CI", "acceptance"}:
                errors.append("exact_sha_ci_workflows_invalid")
    return errors


def build_pilot_report(
    *, contract: dict[str, Any], arm_reports: list[dict[str, Any]]
) -> dict[str, Any]:
    by_scenario: dict[str, dict[str, dict[str, Any]]] = {}
    for row in arm_reports:
        by_scenario.setdefault(str(row.get("scenario_id")), {})[str(row.get("arm"))] = (
            row
        )
    pair_rows = []
    for scenario_id in SCENARIO_ORDER:
        arms = by_scenario.get(scenario_id, {})
        control = arms.get("control", {})
        treatment = arms.get("treatment", {})
        pair_checks = {
            "both_arms_present": set(arms) == set(ARM_ORDER),
            "both_arms_valid": control.get("arm_valid") is True
            and treatment.get("arm_valid") is True,
            "same_input_snapshot": control.get("input_snapshot_sha256")
            == treatment.get("input_snapshot_sha256"),
            "same_failure_boundary": control.get("boundary_signature_sha256")
            == treatment.get("boundary_signature_sha256"),
            "same_binary": control.get("binary_sha256")
            == treatment.get("binary_sha256")
            == contract.get("binary_sha256"),
            "control_endpoint_failed": control.get("registered_endpoint_success")
            is False,
            "treatment_endpoint_passed": treatment.get("registered_endpoint_success")
            is True,
        }
        pair_rows.append(
            {
                "scenario_id": scenario_id,
                "profile": SCENARIOS[scenario_id].profile,
                "checks": pair_checks,
                "pair_valid": all(pair_checks.values()),
                "control_endpoint_success": control.get("registered_endpoint_success"),
                "treatment_endpoint_success": treatment.get(
                    "registered_endpoint_success"
                ),
                "observed_endpoint_contrast": (
                    1
                    if control.get("registered_endpoint_success") is False
                    and treatment.get("registered_endpoint_success") is True
                    else None
                ),
            }
        )
    checks = {
        "exactly_six_preregistered_arms": len(arm_reports) == 6,
        "one_valid_pair_per_required_profile": all(
            row["pair_valid"] for row in pair_rows
        ),
        "all_control_endpoints_failed": all(
            row["control_endpoint_success"] is False for row in pair_rows
        ),
        "all_treatment_endpoints_passed": all(
            row["treatment_endpoint_success"] is True for row in pair_rows
        ),
        "all_pairs_share_input_and_failure_boundary": all(
            row["checks"]["same_input_snapshot"]
            and row["checks"]["same_failure_boundary"]
            for row in pair_rows
        ),
        "all_arms_use_pinned_binary": all(
            row.get("binary_sha256") == contract.get("binary_sha256")
            for row in arm_reports
        ),
    }
    ready = all(checks.values())
    return {
        "schema_version": A26_REPORT_SCHEMA_VERSION,
        "contract_id": contract.get("contract_id"),
        "inference_role": "paired_instrument_validation_only",
        "effect_claim_allowed": False,
        "effect_claim_ready": False,
        "conditional_effect_estimate_reported": False,
        "arm_count": len(arm_reports),
        "pair_count": len(pair_rows),
        "pairs": pair_rows,
        "checks": checks,
        "instrument_ready": ready,
        "pilot_go_no_go": "GO" if ready else "NO-GO",
        "next_design_decision": (
            "request_owner_review_before_a27_confirmatory_preregistration"
            if ready
            else "a26_invalid_requires_forward_only_diagnosis"
        ),
    }


def run_pilot(
    *,
    contract: dict[str, Any],
    contract_path: Path,
    commandagent_bin: Path,
    node_modules_source: Path,
    run_dir: Path,
    execution_root: Path | None = None,
    timeout_sec: int = 180,
) -> dict[str, Any]:
    errors = contract_errors(contract)
    if errors:
        raise ValueError("invalid A26 contract:" + ",".join(errors))
    commandagent_bin = commandagent_bin.resolve()
    if not commandagent_bin.is_file():
        raise ValueError(f"commandagent binary is missing:{commandagent_bin}")
    actual_binary_sha = sha256_file(commandagent_bin)
    if actual_binary_sha != contract.get("binary_sha256"):
        raise ValueError("commandagent binary SHA-256 does not match A26 contract")
    node_modules_source = node_modules_source.resolve()
    if provisioning_manifest_sha256(node_modules_source) != contract.get(
        "nextjs_node_modules_manifest_sha256"
    ):
        raise ValueError("Next.js provisioning manifest does not match A26 contract")
    run_dir = run_dir.resolve()
    if run_dir.name != contract.get("run_id"):
        raise ValueError("A26 run directory name does not match the frozen run id")
    run_dir.mkdir(parents=True, exist_ok=False)
    (run_dir / "contract-copy.json").write_text(
        contract_path.read_text(encoding="utf-8"), encoding="utf-8"
    )
    arm_reports = []
    for scenario_id in SCENARIO_ORDER:
        scenario = SCENARIOS[scenario_id]
        for arm in ARM_ORDER:
            arm_reports.append(
                _run_arm(
                    commandagent_bin=commandagent_bin,
                    scenario=scenario,
                    arm=arm,
                    output_dir=run_dir / "arms" / scenario_id / arm,
                    execution_root=execution_root,
                    node_modules_source=node_modules_source,
                    timeout_sec=timeout_sec,
                )
            )
    report = build_pilot_report(contract=contract, arm_reports=arm_reports)
    report.update(
        {
            "code_sha": contract.get("code_sha"),
            "binary_sha256": actual_binary_sha,
            "contract_sha256": sha256_file(contract_path),
            "nextjs_node_modules_manifest_sha256": contract.get(
                "nextjs_node_modules_manifest_sha256"
            ),
        }
    )
    evidence = {
        path.relative_to(run_dir).as_posix(): sha256_file(path)
        for path in sorted(run_dir.rglob("*"))
        if path.is_file() and path.name != "report.json"
    }
    report["evidence_sha256"] = evidence
    (run_dir / "report.json").write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    return report
