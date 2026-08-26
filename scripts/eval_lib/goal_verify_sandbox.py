from __future__ import annotations

import os
import platform
import subprocess
import time
from pathlib import Path
from typing import Any

from eval_lib.goal_verify_v2 import _plan_hash

SANDBOX_EXEC = Path("/usr/bin/sandbox-exec")
MAX_CAPTURE_BYTES = 1024 * 1024


def sandbox_backend_status() -> dict[str, Any]:
    present = platform.system() == "Darwin" and SANDBOX_EXEC.is_file()
    available = False
    reason = "backend_missing"
    if present:
        probe = subprocess.run(
            [str(SANDBOX_EXEC), "-p", "(version 1) (allow default)", "/usr/bin/true"],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
        available = probe.returncode == 0
        reason = "available" if available else "sandbox_apply_denied"
    return {
        "backend": "macos_sandbox_exec",
        "available": available,
        "reason": reason,
        "network": "denied",
        "writes": "workspace_only",
        "fallback": "fail_closed",
    }


def _sandbox_profile(workspace_root: Path) -> str:
    escaped = str(workspace_root).replace("\\", "\\\\").replace('"', '\\"')
    return "\n".join(
        (
            "(version 1)",
            "(deny default)",
            "(allow process*)",
            "(allow file-read*)",
            "(allow sysctl-read)",
            "(allow mach-lookup)",
            f'(allow file-write* (subpath "{escaped}"))',
            '(allow file-write* (literal "/dev/null"))',
            "(deny network*)",
        )
    )


def _minimal_environment(workspace_root: Path) -> dict[str, str]:
    private_home = workspace_root / ".commandagent-eval-home"
    private_tmp = workspace_root / ".commandagent-eval-tmp"
    private_home.mkdir(exist_ok=True)
    private_tmp.mkdir(exist_ok=True)
    environment = {
        "HOME": str(private_home),
        "TMPDIR": str(private_tmp),
        "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
        "LANG": "C.UTF-8",
        "LC_ALL": "C.UTF-8",
        "NO_COLOR": "1",
    }
    for name in ("RUSTUP_HOME", "CARGO_HOME"):
        if value := os.environ.get(name):
            environment[name] = value
    return environment


def run_macos_sandbox(plan: dict[str, Any]) -> dict[str, Any]:
    """Execute a hash-bound host plan in a no-network, workspace-write sandbox."""
    status = sandbox_backend_status()
    if not status["available"]:
        return {"runner_error": "sandbox_backend_unavailable", "runtime_ms": 0}
    if plan.get("source") != "frozen_host_adapter" or plan.get(
        "raw_provider_argv_used"
    ):
        return {"runner_error": "untrusted_command_plan", "runtime_ms": 0}
    if plan.get("plan_sha256") != _plan_hash(plan):
        return {"runner_error": "command_plan_integrity_failed", "runtime_ms": 0}
    workspace_root = Path(plan["workspace_root"]).resolve()
    cwd = Path(plan["cwd"]).resolve()
    if not cwd.is_relative_to(workspace_root) or not cwd.is_dir():
        return {"runner_error": "command_cwd_outside_workspace", "runtime_ms": 0}
    argv = plan.get("argv")
    if not isinstance(argv, list) or not argv:
        return {"runner_error": "command_argv_invalid", "runtime_ms": 0}

    command = [str(SANDBOX_EXEC), "-p", _sandbox_profile(workspace_root), *argv]
    started = time.monotonic_ns()
    try:
        completed = subprocess.run(
            command,
            cwd=cwd,
            env=_minimal_environment(workspace_root),
            stdin=subprocess.DEVNULL,
            capture_output=True,
            timeout=plan["timeout_ms"] / 1000,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        return {
            "timed_out": True,
            "stdout": (error.stdout or b"")[:MAX_CAPTURE_BYTES].decode(
                "utf-8", errors="replace"
            ),
            "stderr": (error.stderr or b"")[:MAX_CAPTURE_BYTES].decode(
                "utf-8", errors="replace"
            ),
            "runtime_ms": (time.monotonic_ns() - started) // 1_000_000,
        }
    stdout = completed.stdout[:MAX_CAPTURE_BYTES]
    stderr = completed.stderr[:MAX_CAPTURE_BYTES]
    truncated = (
        len(completed.stdout) > MAX_CAPTURE_BYTES
        or len(completed.stderr) > MAX_CAPTURE_BYTES
    )
    return {
        "exit_code": completed.returncode,
        "stdout": stdout.decode("utf-8", errors="replace"),
        "stderr": stderr.decode("utf-8", errors="replace"),
        "timed_out": False,
        "output_truncated": truncated,
        "runtime_ms": (time.monotonic_ns() - started) // 1_000_000,
    }
