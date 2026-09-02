from __future__ import annotations

import hashlib
import json
import os
import shlex
import signal
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
from html.parser import HTMLParser
from pathlib import Path
from typing import Any

from eval_lib.goal_verify_workspaces_v3 import (
    prepare_workspace_stage,
    validate_provisioning,
    validate_workspace_registry,
    workspace_by_case,
)

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_TASK_REGISTRY = (
    ROOT / "eval/goal_verify/v0/phase6-task-contracts-v4-a15-a5.json"
)
DEFAULT_WORKSPACE_REGISTRY = (
    ROOT / "eval/goal_verify/v0/phase6-real-workspaces-v4-a15.json"
)

CASE_SPECS = (
    {
        "case_id": "phase6-main-c07-task-02",
        "workspace_case_id": "main-fix-generic-fixtures",
        "profile": "generic",
        "immutable_paths": ("fixture",),
        "required_delta_path": "app.py",
    },
    {
        "case_id": "phase6-main-c13-task-02",
        "workspace_case_id": "a15-fix-data-reconciliation",
        "profile": "data",
        "immutable_paths": (
            "data",
            "scripts/repro.py",
            "scripts/contract_check.py",
            "tests",
        ),
        "required_delta_path": "pipeline/main.py",
    },
    {
        "case_id": "phase6-main-c14-task-08",
        "workspace_case_id": "a15-fix-nextjs-route-label",
        "profile": "nextjs",
        "immutable_paths": (
            "fixture",
            "scripts/repro.mjs",
            "scripts/regression.mjs",
            "package.json",
            "package-lock.json",
        ),
        "required_delta_path": "lib/label.mjs",
    },
)

GENERIC_REFERENCE_APP = """#!/usr/bin/env python3
import json
import sys


def main(argv):
    if len(argv) != 2:
        return 2
    with open(argv[1], encoding="utf-8") as handle:
        payload = json.load(handle)
    print(sum(item["amount"] if "amount" in item else item["value"] for item in payload["items"]))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
"""

_EXCLUDED_MANIFEST_PARTS = {
    ".goal-verify-tools",
    ".next",
    ".pytest_cache",
    "__pycache__",
    "node_modules",
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


def _sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while chunk := handle.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def _load_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise TypeError(f"expected JSON object:{path}")
    return value


def _command_record(
    argv: list[str],
    *,
    cwd: Path,
    timeout_sec: int,
    env: dict[str, str] | None = None,
) -> dict[str, Any]:
    completed = subprocess.run(
        argv,
        cwd=cwd,
        env=env,
        stdin=subprocess.DEVNULL,
        text=True,
        capture_output=True,
        timeout=timeout_sec,
        check=False,
    )
    return {
        "argv": argv,
        "returncode": completed.returncode,
        "stdout_sha256": _sha256_bytes(completed.stdout.encode()),
        "stderr_sha256": _sha256_bytes(completed.stderr.encode()),
        "stdout_tail": completed.stdout[-1000:],
        "stderr_tail": completed.stderr[-1000:],
    }


def _manifest(root: Path) -> dict[str, str]:
    rows: dict[str, str] = {}
    for path in sorted(root.rglob("*")):
        if not path.is_file() or path.is_symlink():
            continue
        relative = path.relative_to(root)
        if any(part in _EXCLUDED_MANIFEST_PARTS for part in relative.parts):
            continue
        rows[relative.as_posix()] = _sha256_file(path)
    return rows


def _path_manifest(root: Path, relative_text: str) -> dict[str, str]:
    relative = Path(relative_text)
    if relative.is_absolute() or ".." in relative.parts:
        raise ValueError(f"unsafe immutable path:{relative_text}")
    target = root / relative
    if target.is_file():
        return {relative.as_posix(): _sha256_file(target)}
    if not target.is_dir():
        return {}
    return {
        path.relative_to(root).as_posix(): _sha256_file(path)
        for path in sorted(target.rglob("*"))
        if path.is_file()
        and not path.is_symlink()
        and not any(
            part in _EXCLUDED_MANIFEST_PARTS for part in path.relative_to(root).parts
        )
    }


def _immutable_manifest(root: Path, paths: tuple[str, ...]) -> dict[str, str]:
    rows: dict[str, str] = {}
    for relative in paths:
        rows.update(_path_manifest(root, relative))
    return rows


def _delta(before: dict[str, str], after: dict[str, str]) -> dict[str, list[str]]:
    return {
        "added_paths": sorted(after.keys() - before.keys()),
        "changed_paths": sorted(
            path for path in before.keys() & after.keys() if before[path] != after[path]
        ),
        "removed_paths": sorted(before.keys() - after.keys()),
    }


def _extract_target_text(html: str, target_id: str) -> str | None:
    parser = _TargetTextParser(target_id)
    parser.feed(html)
    return parser.text()


def _route_observation(
    workspace: Path,
    *,
    port: int,
    target_id: str,
    timeout_sec: int,
    env: dict[str, str],
) -> dict[str, Any]:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as probe:
        probe.bind(("127.0.0.1", port))
    argv = ["npx", "next", "start", "-p", str(port)]
    process = subprocess.Popen(
        argv,
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
                    f"http://127.0.0.1:{port}/", timeout=1
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
    return {
        "argv": argv,
        "error": error,
        "http_observed": html is not None,
        "target_id": target_id,
        "target_text": (
            _extract_target_text(html, target_id) if html is not None else None
        ),
        "html_sha256": _sha256_bytes(html.encode()) if html is not None else None,
        "stdout_sha256": _sha256_bytes(stdout.encode()),
        "stderr_sha256": _sha256_bytes(stderr.encode()),
        "stderr_tail": stderr[-1000:],
    }


def _task_by_id(registry: dict[str, Any]) -> dict[str, dict[str, Any]]:
    cases = registry.get("cases")
    if not isinstance(cases, list):
        raise TypeError("task registry cases are missing")
    return {
        row["case_id"]: row
        for row in cases
        if isinstance(row, dict) and isinstance(row.get("case_id"), str)
    }


def _prepare_case_workspaces(
    *,
    root: Path,
    spec: dict[str, Any],
    workspace_contract: dict[str, Any],
    scratch: Path,
    provisioned_root: Path,
) -> tuple[Path, Path]:
    profile = spec["profile"]
    before = prepare_workspace_stage(
        root=root,
        workspace=workspace_contract,
        stage="before",
        destination=scratch / profile / "before",
        provisioned_root=provisioned_root,
    )
    if profile == "generic":
        reference = prepare_workspace_stage(
            root=root,
            workspace=workspace_contract,
            stage="before",
            destination=scratch / profile / "reference",
            provisioned_root=provisioned_root,
        )
        (reference / "app.py").write_text(GENERIC_REFERENCE_APP, encoding="utf-8")
    else:
        reference = prepare_workspace_stage(
            root=root,
            workspace=workspace_contract,
            stage="after",
            destination=scratch / profile / "reference",
            provisioned_root=provisioned_root,
        )
    return before, reference


def _observe_case(
    *,
    spec: dict[str, Any],
    task: dict[str, Any],
    before: Path,
    reference: Path,
    timeout_sec: int,
) -> dict[str, Any]:
    completion = task["completion_contract"]
    operational = task["operational_constraints"]
    reproducer = operational["reproducer"]
    reproducer_argv = list(reproducer["argv"])
    verify_commands = completion["verify_commands"]
    command_argv = [shlex.split(command) for command in verify_commands]
    if command_argv[0] != reproducer_argv:
        raise ValueError(f"completion/reproducer command mismatch:{task['case_id']}")
    env = os.environ.copy()
    env["npm_config_offline"] = "true"
    env["NEXT_TELEMETRY_DISABLED"] = "1"
    env["HTTP_PROXY"] = "http://127.0.0.1:9"
    env["HTTPS_PROXY"] = "http://127.0.0.1:9"
    env["ALL_PROXY"] = "http://127.0.0.1:9"
    env["NO_PROXY"] = "127.0.0.1,localhost"
    before_commands = [
        _command_record(argv, cwd=before, timeout_sec=timeout_sec, env=env)
        for argv in command_argv
    ]
    reference_commands = [
        _command_record(argv, cwd=reference, timeout_sec=timeout_sec, env=env)
        for argv in command_argv
    ]
    route = None
    if spec["profile"] == "nextjs":
        port = int(operational["reserved_port"])
        fixture = json.loads(
            (before / operational["registered_reproducer_fixture"]).read_text(
                encoding="utf-8"
            )
        )
        target_id = f"result-{fixture['task']}"
        route = {
            "expected_text": fixture["expected"],
            "before": _route_observation(
                before,
                port=port,
                target_id=target_id,
                timeout_sec=timeout_sec,
                env=env,
            ),
            "reference": _route_observation(
                reference,
                port=port,
                target_id=target_id,
                timeout_sec=timeout_sec,
                env=env,
            ),
        }
    before_manifest = _manifest(before)
    reference_manifest = _manifest(reference)
    delta = _delta(before_manifest, reference_manifest)
    immutable_before = _immutable_manifest(before, spec["immutable_paths"])
    immutable_reference = _immutable_manifest(reference, spec["immutable_paths"])
    required_paths_present = all(
        (before / relative).exists() for relative in completion["required_paths"]
    )
    before_reproducer_failed = (
        before_commands[0]["returncode"] == reproducer["expected_exit_before"]
    )
    reference_reproducer_passed = (
        reference_commands[0]["returncode"] == reproducer["expected_exit_after"]
    )
    regressions_passed = all(
        row["returncode"] == 0 for row in before_commands[1:] + reference_commands[1:]
    )
    route_polarity = route is None or (
        route["before"]["http_observed"] is True
        and route["before"]["target_text"] != route["expected_text"]
        and route["reference"]["http_observed"] is True
        and route["reference"]["target_text"] == route["expected_text"]
    )
    checks = {
        "profile_matches_contract": completion["profile"] == spec["profile"],
        "registered_fixture_bound": (
            reproducer_argv[-1] == operational["registered_reproducer_fixture"]
        ),
        "required_paths_present_before": required_paths_present,
        "before_reproducer_failed_with_registered_exit": before_reproducer_failed,
        "reference_reproducer_passed_with_registered_exit": (
            reference_reproducer_passed
        ),
        "frozen_regressions_pass_before_and_reference": regressions_passed,
        "immutable_inputs_unchanged": immutable_before == immutable_reference,
        "reference_delta_contains_repair_target": (
            spec["required_delta_path"] in delta["changed_paths"]
        ),
        "route_render_before_fail_reference_pass": route_polarity,
    }
    return {
        "case_id": task["case_id"],
        "workspace_case_id": spec["workspace_case_id"],
        "profile": spec["profile"],
        "registered_reproducer_fixture": operational["registered_reproducer_fixture"],
        "before_commands": before_commands,
        "reference_commands": reference_commands,
        "route_render": route,
        "reference_delta": delta,
        "immutable_input_sha256": {
            "before": _sha256_bytes(
                json.dumps(immutable_before, sort_keys=True).encode()
            ),
            "reference": _sha256_bytes(
                json.dumps(immutable_reference, sort_keys=True).encode()
            ),
        },
        "checks": checks,
        "corpus_ready": all(checks.values()),
    }


def build_report(
    *,
    cases: list[dict[str, Any]],
    task_registry_sha256: str,
    workspace_registry_sha256: str,
    provisioning_sha256: str,
) -> dict[str, Any]:
    profiles = {case.get("profile") for case in cases}
    checks = {
        "exactly_one_case_per_target_profile": len(cases) == 3
        and profiles == {"generic", "data", "nextjs"},
        "every_case_candidate_visible_before_failure": all(
            case.get("checks", {}).get("before_reproducer_failed_with_registered_exit")
            is True
            for case in cases
        ),
        "every_case_reference_passes_same_reproducer": all(
            case.get("checks", {}).get(
                "reference_reproducer_passed_with_registered_exit"
            )
            is True
            for case in cases
        ),
        "every_case_regressions_and_immutability_pass": all(
            case.get("checks", {}).get("frozen_regressions_pass_before_and_reference")
            is True
            and case.get("checks", {}).get("immutable_inputs_unchanged") is True
            for case in cases
        ),
        "nextjs_route_polarity_is_distinct": any(
            case.get("profile") == "nextjs"
            and case.get("checks", {}).get("route_render_before_fail_reference_pass")
            is True
            for case in cases
        ),
        "all_cases_ready": all(case.get("corpus_ready") is True for case in cases),
    }
    ready = all(checks.values())
    return {
        "schema_version": (
            "commandagent.goal_verify.recovery_exposure_corpus_pilot.v1"
        ),
        "inference_role": "candidate_visible_failure_corpus_qualification_only",
        "effect_claim_allowed": False,
        "full_effect_execution_authorized": False,
        "task_registry_sha256": task_registry_sha256,
        "workspace_registry_sha256": workspace_registry_sha256,
        "provisioning_sha256": provisioning_sha256,
        "case_count": len(cases),
        "profiles": sorted(profile for profile in profiles if isinstance(profile, str)),
        "checks": checks,
        "corpus_ready_for_preregistration": ready,
        "go_no_go": "GO" if ready else "NO-GO",
    }


def run_pilot(
    *,
    run_dir: Path,
    execution_root: Path,
    provisioned_root: Path,
    task_registry_path: Path = DEFAULT_TASK_REGISTRY,
    workspace_registry_path: Path = DEFAULT_WORKSPACE_REGISTRY,
    timeout_sec: int = 120,
) -> dict[str, Any]:
    run_dir = run_dir.resolve()
    run_dir.mkdir(parents=True, exist_ok=False)
    execution_root = execution_root.resolve()
    provisioned_root = provisioned_root.resolve()
    task_registry_path = task_registry_path.resolve()
    workspace_registry_path = workspace_registry_path.resolve()
    task_registry = _load_json(task_registry_path)
    workspace_registry = _load_json(workspace_registry_path)
    if task_registry.get("status") != "frozen":
        raise ValueError("task registry is not frozen")
    if workspace_registry.get("status") != "frozen":
        raise ValueError("workspace registry is not frozen")
    tasks = _task_by_id(task_registry)
    workspaces = workspace_by_case(workspace_registry)
    selected_workspaces = {
        "workspaces": [workspaces[spec["workspace_case_id"]] for spec in CASE_SPECS]
    }
    workspace_errors = validate_workspace_registry(
        root=ROOT, registry=selected_workspaces, require_frozen=True
    )
    provisioning_errors = validate_provisioning(
        {
            "workspaces": [
                workspaces["a15-fix-nextjs-route-label"],
            ]
        },
        provisioned_root,
    )
    if workspace_errors or provisioning_errors:
        raise ValueError(
            "corpus inputs invalid:" + ",".join(workspace_errors + provisioning_errors)
        )
    with tempfile.TemporaryDirectory(
        prefix="commandagent-recovery-exposure-corpus-",
        dir=execution_root,
    ) as temporary:
        scratch = Path(temporary)
        cases = []
        for spec in CASE_SPECS:
            before, reference = _prepare_case_workspaces(
                root=ROOT,
                spec=spec,
                workspace_contract=workspaces[spec["workspace_case_id"]],
                scratch=scratch,
                provisioned_root=provisioned_root,
            )
            cases.append(
                _observe_case(
                    spec=spec,
                    task=tasks[spec["case_id"]],
                    before=before,
                    reference=reference,
                    timeout_sec=timeout_sec,
                )
            )
    case_evidence_path = run_dir / "case-evidence.json"
    case_evidence_path.write_text(
        json.dumps(cases, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    provisioning = workspaces["a15-fix-nextjs-route-label"]["provisioning"]
    provisioning_path = provisioned_root / provisioning["tarball"]
    report = build_report(
        cases=cases,
        task_registry_sha256=_sha256_file(task_registry_path),
        workspace_registry_sha256=_sha256_file(workspace_registry_path),
        provisioning_sha256=_sha256_file(provisioning_path),
    )
    report["evidence_sha256"] = {"case-evidence.json": _sha256_file(case_evidence_path)}
    (run_dir / "report.json").write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    return report
