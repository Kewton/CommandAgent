from __future__ import annotations

import hashlib
import json
import os
import subprocess
import time
import urllib.error
import urllib.request
from collections.abc import Callable
from pathlib import Path
from typing import Any

from eval_lib.goal_verify_sandbox import run_macos_sandbox
from eval_lib.goal_verify_v2 import _plan_hash

CommandRunner = Callable[[list[str], Path, int], dict[str, Any]]


def run_command(argv: list[str], cwd: Path, timeout_ms: int) -> dict[str, Any]:
    plan = {
        "schema_version": "commandagent.goal_verify.command_plan.v3",
        "oracle_id": "v3-registered-executor",
        "source": "frozen_host_adapter",
        "workspace_root": str(cwd.resolve()),
        "cwd": str(cwd.resolve()),
        "argv": argv,
        "timeout_ms": timeout_ms,
        "observation": {"kind": "exit_code", "expected": 0},
        "raw_provider_argv_used": False,
    }
    plan["plan_sha256"] = _plan_hash(plan)
    result = run_macos_sandbox(plan)
    return {**result, "executed": not bool(result.get("runner_error"))}


def _safe_argv(argv: Any) -> list[str]:
    if (
        not isinstance(argv, list)
        or not argv
        or any(not isinstance(item, str) or not item or "\x00" in item for item in argv)
        or Path(argv[0]).is_absolute()
    ):
        raise ValueError("unsafe registered argv")
    return argv


def _command_result(
    executor: dict[str, Any], workspace: Path, runner: CommandRunner
) -> dict[str, Any]:
    result = runner(
        _safe_argv(executor["argv"]),
        workspace,
        int(executor.get("timeout_ms", 30_000)),
    )
    if result.get("runner_error"):
        return {
            **result,
            "executed": False,
            "result": "oracle_error",
            "reason": str(result["runner_error"]),
        }
    if result.get("timed_out"):
        return {**result, "result": "blocked", "reason": "timeout"}
    blocked_patterns = executor.get("blocked_patterns", [])
    if not isinstance(blocked_patterns, list) or any(
        not isinstance(pattern, str) or not pattern for pattern in blocked_patterns
    ):
        raise ValueError("blocked_patterns must be a string array")
    output = f"{result.get('stdout', '')}\n{result.get('stderr', '')}".casefold()
    matched_blocker = next(
        (pattern for pattern in blocked_patterns if pattern.casefold() in output), None
    )
    if matched_blocker is not None:
        return {
            **result,
            "result": "blocked",
            "reason": "dependency_unavailable",
            "matched_blocked_pattern": matched_blocker,
        }
    expected = executor["observation"].get("expected")
    actual = result.get(executor["observation"]["kind"])
    passed = actual == expected
    return {
        **result,
        "result": "pass" if passed else "fail",
        "reason": "observation_match" if passed else "observation_mismatch",
        "actual": actual,
    }


def execute_registered(
    executor: dict[str, Any], *, workspace: Path, runner: CommandRunner = run_command
) -> dict[str, Any]:
    kind = executor.get("kind")
    if kind == "unavailable":
        return {
            "executed": False,
            "result": "unverified",
            "reason": "executor_unavailable",
        }
    if kind in {"sandbox_command", "stage_command"}:
        return _command_result(executor, workspace, runner)
    if kind == "fixture_hash_command":
        fixture = executor["registered_fixture"]
        path = (workspace / fixture["path"]).resolve()
        if not path.is_relative_to(workspace.resolve()) or not path.is_file():
            return {
                "executed": False,
                "result": "oracle_error",
                "reason": "fixture_missing",
            }
        actual_hash = hashlib.sha256(path.read_bytes()).hexdigest()
        if fixture.get("sha256") and fixture["sha256"] != actual_hash:
            return {
                "executed": False,
                "result": "fail",
                "reason": "fixture_hash_mismatch",
                "fixture_sha256": actual_hash,
            }
        return {
            **_command_result(executor, workspace, runner),
            "fixture_sha256": actual_hash,
        }
    if kind == "file_content":
        return _file_content_result(executor, workspace)
    if kind == "regression_set":
        rows = []
        for row in executor["registered"]:
            outcome = runner(
                _safe_argv(row["argv"]),
                workspace,
                int(executor.get("timeout_ms", 60_000)),
            )
            rows.append({"id": row["id"], **outcome})
        runner_errors = [row for row in rows if row.get("runner_error")]
        if runner_errors:
            return {
                "executed": False,
                "result": "oracle_error",
                "reason": "registered_executor_error",
                "registered_results": rows,
                "runtime_ms": sum(int(row.get("runtime_ms", 0)) for row in rows),
            }
        passed = len(rows) == len(executor["registered"]) and all(
            row.get("exit_code") == 0 and not row.get("timed_out") for row in rows
        )
        return {
            "executed": True,
            "result": "pass" if passed else "fail",
            "reason": "full_regression_set_pass" if passed else "regression_set_failed",
            "registered_results": rows,
            "actual": 0 if passed else 1,
            "runtime_ms": sum(int(row.get("runtime_ms", 0)) for row in rows),
        }
    if kind == "http_get":
        return _execute_http(executor, workspace, runner)
    if kind == "playwright_script":
        return _execute_playwright(executor, workspace, runner)
    raise ValueError(f"unsupported v3 executor: {kind}")


def _file_content_result(
    executor: dict[str, Any], workspace: Path
) -> dict[str, Any]:
    relative = Path(executor.get("path", ""))
    if (
        not relative.parts
        or relative.is_absolute()
        or ".." in relative.parts
        or relative.as_posix().startswith(".")
    ):
        return {
            "executed": False,
            "result": "oracle_error",
            "reason": "file_content_path_invalid",
        }
    path = (workspace / relative).resolve()
    if not path.is_relative_to(workspace.resolve()) or not path.is_file():
        return {
            "executed": False,
            "result": "oracle_error",
            "reason": "file_content_path_missing",
        }
    pattern = executor.get("pattern")
    polarity = executor.get("polarity")
    if not isinstance(pattern, str) or not pattern or polarity not in {
        "present",
        "absent",
    }:
        return {
            "executed": False,
            "result": "oracle_error",
            "reason": "file_content_predicate_invalid",
        }
    try:
        content = path.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        return {
            "executed": False,
            "result": "oracle_error",
            "reason": "file_content_not_utf8",
        }
    present = pattern in content
    passed = present if polarity == "present" else not present
    return {
        "executed": True,
        "result": "pass" if passed else "fail",
        "reason": "predicate_match" if passed else "predicate_mismatch",
        "path": relative.as_posix(),
        "pattern_sha256": hashlib.sha256(pattern.encode()).hexdigest(),
        "polarity": polarity,
        "actual_present": present,
    }


def _wait_for_url(url: str, timeout_ms: int) -> bool:
    deadline = time.monotonic() + timeout_ms / 1000
    while time.monotonic() < deadline:
        try:
            with urllib.request.urlopen(url, timeout=1):
                return True
        except (OSError, urllib.error.URLError):
            time.sleep(0.1)
    return False


def _with_server(
    executor: dict[str, Any], workspace: Path, action: Callable[[], dict[str, Any]]
) -> dict[str, Any]:
    server = executor["server"]
    if not server.get("loopback_only"):
        raise ValueError("server must be loopback-only")
    process = subprocess.Popen(
        _safe_argv(server["argv"]),
        cwd=workspace,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    try:
        ready = f"http://127.0.0.1:{server['port']}{server['ready_path']}"
        if not _wait_for_url(ready, int(executor.get("timeout_ms", 30_000))):
            return {
                "executed": False,
                "result": "blocked",
                "reason": "server_not_ready",
            }
        return action()
    finally:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()


def _execute_http(
    executor: dict[str, Any], workspace: Path, runner: CommandRunner
) -> dict[str, Any]:
    del runner

    def action() -> dict[str, Any]:
        with urllib.request.urlopen(executor["url"], timeout=5) as response:
            status = response.status
        expected = executor["observation"]["expected"]
        return {
            "executed": True,
            "result": "pass" if status == expected else "fail",
            "reason": "observation_match"
            if status == expected
            else "observation_mismatch",
            "actual": status,
        }

    return _with_server(executor, workspace, action)


def _execute_playwright(
    executor: dict[str, Any], workspace: Path, runner: CommandRunner
) -> dict[str, Any]:
    script = """
const { chromium } = require('playwright-core');
(async () => {
  const ops = JSON.parse(process.argv[1]); const base = process.argv[2]; const executablePath = process.argv[3];
  const browser = await chromium.launch({headless:true, executablePath, args:['--disable-background-networking','--disable-component-update','--disable-crash-reporter','--disable-crashpad-for-testing','--disable-gpu','--single-process','--no-zygote','--no-first-run']}); const page = await browser.newPage();
  const allowedOrigin = new URL(base).origin;
  await page.route('**/*', route => { const url = new URL(route.request().url()); if (url.origin === allowedOrigin || url.protocol === 'data:' || url.protocol === 'about:') return route.continue(); return route.abort('blockedbyclient'); });
  const values = [];
  for (const op of ops) {
    if (op.goto) await page.goto(base + op.goto);
    else if (op.click) await page.click(op.click);
    else if (op.read_text) values.push({selector:op.read_text,text:await page.textContent(op.read_text)});
    else if (op.computed_style) values.push({selector:op.computed_style[0],computed:op.computed_style[1],value:await page.$eval(op.computed_style[0],(e,p)=>getComputedStyle(e).getPropertyValue(p),op.computed_style[1])});
  }
  console.log(JSON.stringify(values)); await browser.close();
})().catch(e => { console.error(e); process.exit(2); });
""".strip()

    def action() -> dict[str, Any]:
        port = executor["server"]["port"]
        browser_relative = Path(executor["browser_executable"])
        if browser_relative.is_absolute() or ".." in browser_relative.parts:
            return {
                "executed": False,
                "result": "oracle_error",
                "reason": "browser_path_invalid",
            }
        browser = (workspace / browser_relative).resolve()
        if not browser.is_file() or _sha256_file(browser) != executor["browser_sha256"]:
            return {
                "executed": False,
                "result": "oracle_error",
                "reason": "browser_hash_mismatch",
            }
        command = [
            "node",
            "-e",
            script,
            json.dumps(executor["script"]),
            f"http://127.0.0.1:{port}",
            str(browser),
        ]
        outcome = (
            _run_registered_browser_command(
                command, workspace, int(executor.get("timeout_ms", 60_000))
            )
            if runner is run_command
            else runner(
                command,
                workspace,
                int(executor.get("timeout_ms", 60_000)),
            )
        )
        if outcome.get("runner_error"):
            return {
                **outcome,
                "executed": False,
                "result": "oracle_error",
                "reason": str(outcome["runner_error"]),
            }
        if outcome.get("exit_code") != 0:
            return {
                **outcome,
                "result": "blocked",
                "reason": "playwright_unavailable_or_failed",
            }
        values = json.loads(outcome["stdout"])
        expected = executor["observation"]
        if expected["kind"] == "interaction":
            passed = (
                bool(values)
                and values[-1].get("text", "").strip() == expected["expected"]
            )
        else:
            passed = all(_dom_check(values, check) for check in expected["checks"])
        return {
            **outcome,
            "result": "pass" if passed else "fail",
            "reason": "observation_match" if passed else "observation_mismatch",
            "actual": values,
        }

    return _with_server(executor, workspace, action)


def _run_registered_browser_command(
    argv: list[str], workspace: Path, timeout_ms: int
) -> dict[str, Any]:
    private_home = workspace / ".commandagent-browser-home"
    private_tmp = workspace / ".commandagent-browser-tmp"
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
    started = time.monotonic_ns()
    try:
        completed = subprocess.run(
            argv,
            cwd=workspace,
            env=environment,
            stdin=subprocess.DEVNULL,
            text=True,
            capture_output=True,
            timeout=timeout_ms / 1000,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        return {
            "timed_out": True,
            "stdout": error.stdout or "",
            "stderr": error.stderr or "",
            "runtime_ms": (time.monotonic_ns() - started) // 1_000_000,
        }
    return {
        "exit_code": completed.returncode,
        "stdout": completed.stdout,
        "stderr": completed.stderr,
        "timed_out": False,
        "runtime_ms": (time.monotonic_ns() - started) // 1_000_000,
    }


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _dom_check(values: list[dict[str, Any]], check: dict[str, Any]) -> bool:
    matches = [row for row in values if row.get("selector") == check["selector"]]
    if "text" in check:
        return any(row.get("text", "").strip() == check["text"] for row in matches)
    return any(row.get("value", "").strip() in check["expected_any"] for row in matches)
