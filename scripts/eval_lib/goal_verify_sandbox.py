from __future__ import annotations

import json
import os
import platform
import shutil
import subprocess
import time
import urllib.error
import urllib.request
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


def _sandbox_profile(
    workspace_root: Path,
    *,
    restricted_reads: bool = False,
    argv0: str | None = None,
    loopback: bool = False,
    extra_read_roots: tuple[Path, ...] = (),
) -> str:
    escaped = str(workspace_root).replace("\\", "\\\\").replace('"', '\\"')
    rules = [
        "(version 1)",
        "(deny default)",
        "(allow process*)",
        "(allow signal (target same-sandbox))",
    ]
    if restricted_reads:
        rules.append("(allow file-read*)")
        for protected in (
            Path.home(),
            Path("/Volumes"),
            Path("/private/tmp"),
            Path("/private/var/folders"),
        ):
            rules.append(
                f'(deny file-read-data (subpath "{_escape_profile_path(protected)}"))'
            )
        roots = [workspace_root, *_runtime_read_roots(argv0), *extra_read_roots]
        for allowed in sorted({path.resolve() for path in roots if path.exists()}):
            rules.append(
                f'(allow file-read-data (subpath "{_escape_profile_path(allowed)}"))'
            )
    else:
        rules.append("(allow file-read*)")
    rules.extend(
        (
            "(allow sysctl-read)",
            "(allow mach-lookup)",
            f'(allow file-write* (subpath "{escaped}"))',
            '(allow file-write* (literal "/dev/null"))',
            "(deny network*)",
        )
    )
    if loopback:
        rules.extend(
            (
                '(allow network-inbound (local ip "localhost:*"))',
                '(allow network-outbound (remote ip "localhost:*"))',
            )
        )
    return "\n".join(rules)


def _escape_profile_path(path: Path) -> str:
    return str(path).replace("\\", "\\\\").replace('"', '\\"')


def _runtime_read_roots(argv0: str | None) -> list[Path]:
    if not argv0:
        return []
    resolved = shutil.which(argv0)
    if not resolved:
        return []
    executable = Path(resolved).resolve()
    parts = executable.parts
    for marker in (".pyenv", ".nvm", ".rustup"):
        if marker in parts and "versions" in parts:
            version_index = parts.index("versions") + 1
            if version_index < len(parts):
                return [Path(*parts[: version_index + 1])]
    return [executable.parent]


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
        "npm_config_offline": "true",
        "NEXT_TELEMETRY_DISABLED": "1",
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
    if plan.get("source") not in {
        "frozen_host_adapter",
        "host_validated_candidate_v4",
    } or plan.get("raw_provider_argv_used"):
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

    restricted_reads = plan.get("read_scope") == "workspace_and_runtime"
    command = [
        str(SANDBOX_EXEC),
        "-p",
        _sandbox_profile(
            workspace_root, restricted_reads=restricted_reads, argv0=argv[0]
        ),
        *argv,
    ]
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


_BROWSER_SCRIPT = """
(async () => {
  const request = JSON.parse(process.argv[1]);
  const { chromium } = require(request.playwright);
  const browser = await chromium.launch({
    headless: true,
    executablePath: request.executable,
    args: ['--disable-background-networking','--disable-component-update',
      '--disable-crash-reporter','--disable-crashpad-for-testing','--disable-gpu',
      '--single-process','--no-zygote','--no-first-run']
  });
  const page = await browser.newPage();
  const origin = new URL(request.base).origin;
  await page.route('**/*', route => {
    const url = new URL(route.request().url());
    if (url.origin === origin || url.protocol === 'data:' || url.protocol === 'about:') {
      return route.continue();
    }
    return route.abort('blockedbyclient');
  });
  await page.goto(request.base + request.route);
  for (const action of request.actions) {
    for (let index = 0; index < action.repeat; index += 1) {
      await page.click(action.selector);
    }
  }
  const actual = request.property
    ? await page.$eval(request.selector, (element, property) =>
        getComputedStyle(element).getPropertyValue(property).trim(), request.property)
    : (await page.textContent(request.selector) || '').trim();
  console.log(JSON.stringify({actual}));
  await browser.close();
})().catch(error => { console.error(error); process.exit(2); });
""".strip()


def run_macos_sandbox_web_probe(plan: dict[str, Any]) -> dict[str, Any]:
    """Run a candidate-owned loopback server plus a host-fixed HTTP/browser probe."""
    status = sandbox_backend_status()
    if not status["available"]:
        return {"runner_error": "sandbox_backend_unavailable", "runtime_ms": 0}
    if plan.get("source") != "host_validated_candidate_web_v4" or plan.get(
        "raw_provider_argv_used"
    ):
        return {"runner_error": "untrusted_web_plan", "runtime_ms": 0}
    if plan.get("plan_sha256") != _plan_hash(plan):
        return {"runner_error": "web_plan_integrity_failed", "runtime_ms": 0}
    root = Path(plan["workspace_root"]).resolve()
    cwd = Path(plan["cwd"]).resolve()
    if not cwd.is_relative_to(root) or not cwd.is_dir():
        return {"runner_error": "command_cwd_outside_workspace", "runtime_ms": 0}
    server_argv = plan.get("server_argv")
    if not isinstance(server_argv, list) or not server_argv:
        return {"runner_error": "server_argv_invalid", "runtime_ms": 0}
    prepare_argv = plan.get("prepare_argv", [])
    if not isinstance(prepare_argv, list):
        return {"runner_error": "prepare_argv_invalid", "runtime_ms": 0}
    started = time.monotonic_ns()
    if prepare_argv:
        try:
            prepared = subprocess.run(
                _sandboxed_command(root, prepare_argv, loopback=False),
                cwd=cwd,
                env=_minimal_environment(root),
                stdin=subprocess.DEVNULL,
                capture_output=True,
                timeout=plan["timeout_ms"] / 1000,
                check=False,
            )
        except subprocess.TimeoutExpired:
            return {
                "executed": False,
                "result": "blocked",
                "reason": "server_prepare_timeout",
                "runtime_ms": (time.monotonic_ns() - started) // 1_000_000,
            }
        except OSError as error:
            return {
                "runner_error": f"server_prepare_failed:{type(error).__name__}",
                "runtime_ms": (time.monotonic_ns() - started) // 1_000_000,
            }
        if prepared.returncode != 0:
            return {
                "executed": True,
                "result": "fail",
                "reason": "server_prepare_failed",
                "exit_code": prepared.returncode,
                "stdout": prepared.stdout[:MAX_CAPTURE_BYTES].decode(
                    "utf-8", errors="replace"
                ),
                "stderr": prepared.stderr[:MAX_CAPTURE_BYTES].decode(
                    "utf-8", errors="replace"
                ),
                "runtime_ms": (time.monotonic_ns() - started) // 1_000_000,
            }
    server = _sandboxed_command(root, server_argv, loopback=True)
    try:
        process = subprocess.Popen(
            server,
            cwd=cwd,
            env=_minimal_environment(root),
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
    except OSError as error:
        return {
            "runner_error": f"server_start_failed:{type(error).__name__}",
            "runtime_ms": (time.monotonic_ns() - started) // 1_000_000,
        }
    try:
        base = f"http://127.0.0.1:{plan['port']}"
        remaining_ms = _remaining_timeout_ms(started, plan["timeout_ms"])
        if remaining_ms <= 0:
            return {
                "executed": False,
                "result": "blocked",
                "reason": "server_prepare_timeout",
                "runtime_ms": (time.monotonic_ns() - started) // 1_000_000,
            }
        if not _wait_for_loopback(
            base + plan["ready_path"], remaining_ms, process
        ):
            return {
                "executed": False,
                "result": "blocked",
                "reason": "server_not_ready",
                "runtime_ms": (time.monotonic_ns() - started) // 1_000_000,
            }
        if plan["kind"] == "http_probe":
            request = urllib.request.Request(base + plan["path"], method=plan["method"])
            try:
                remaining_ms = _remaining_timeout_ms(started, plan["timeout_ms"])
                if remaining_ms <= 0:
                    raise subprocess.TimeoutExpired("http_probe", 0)
                with urllib.request.urlopen(
                    request, timeout=min(5, remaining_ms / 1000)
                ) as response:
                    actual: Any = response.status
            except urllib.error.HTTPError as response:
                actual = response.code
        else:
            browser = Path(plan["browser_executable"]).resolve()
            playwright = Path(plan["playwright_module"]).resolve()
            request = {
                "base": base,
                "route": plan["route"],
                "selector": plan["selector"],
                "actions": plan["actions"],
                "property": plan.get("property"),
                "executable": str(browser),
                "playwright": str(playwright),
            }
            argv = [
                shutil.which("node") or "node",
                "-e",
                _BROWSER_SCRIPT,
                json.dumps(request),
            ]
            completed = subprocess.run(
                argv,
                cwd=cwd,
                env=_minimal_environment(root),
                stdin=subprocess.DEVNULL,
                capture_output=True,
                timeout=max(
                    _remaining_timeout_ms(started, plan["timeout_ms"]), 1
                )
                / 1000,
                check=False,
            )
            if completed.returncode != 0:
                return {
                    "executed": True,
                    "result": "fail",
                    "reason": "browser_probe_failed",
                    "exit_code": completed.returncode,
                    "stderr": completed.stderr[:MAX_CAPTURE_BYTES].decode(
                        "utf-8", errors="replace"
                    ),
                    "runtime_ms": (time.monotonic_ns() - started) // 1_000_000,
                }
            actual = json.loads(completed.stdout)["actual"]
        expected = plan["expected"]
        passed = actual == expected
        return {
            "executed": True,
            "result": "pass" if passed else "fail",
            "reason": "observation_match" if passed else "observation_mismatch",
            "actual": actual,
            "observed_strength": "runtime" if passed else None,
            "runtime_ms": (time.monotonic_ns() - started) // 1_000_000,
        }
    except (
        OSError,
        urllib.error.URLError,
        subprocess.TimeoutExpired,
        json.JSONDecodeError,
    ) as error:
        return {
            "executed": False,
            "result": "blocked",
            "reason": f"web_probe_error:{type(error).__name__}",
            "runtime_ms": (time.monotonic_ns() - started) // 1_000_000,
        }
    finally:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()


def _sandboxed_command(
    root: Path,
    argv: list[str],
    *,
    loopback: bool,
    extra_read_roots: tuple[Path, ...] = (),
) -> list[str]:
    return [
        str(SANDBOX_EXEC),
        "-p",
        _sandbox_profile(
            root,
            restricted_reads=True,
            argv0=argv[0],
            loopback=loopback,
            extra_read_roots=extra_read_roots,
        ),
        *argv,
    ]


def _wait_for_loopback(url: str, timeout_ms: int, process: subprocess.Popen) -> bool:
    deadline = time.monotonic() + timeout_ms / 1000
    while time.monotonic() < deadline and process.poll() is None:
        try:
            with urllib.request.urlopen(url, timeout=1):
                return True
        except urllib.error.HTTPError:
            return True
        except (OSError, urllib.error.URLError):
            time.sleep(0.1)
    return False


def _remaining_timeout_ms(started_ns: int, timeout_ms: int) -> int:
    elapsed_ms = (time.monotonic_ns() - started_ns) // 1_000_000
    return max(int(timeout_ms) - int(elapsed_ms), 0)
