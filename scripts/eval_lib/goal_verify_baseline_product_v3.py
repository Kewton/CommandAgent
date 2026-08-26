from __future__ import annotations

import json
import subprocess
import time
from collections.abc import Callable
from pathlib import Path
from typing import Any

Replay = Callable[[list[str], Path], dict[str, Any]]


def build_product_argv(
    *,
    commandagent_bin: Path,
    workspace: Path,
    case: dict[str, Any],
    model: str,
) -> list[str]:
    return [
        str(commandagent_bin.resolve()),
        "--cwd",
        str(workspace.resolve()),
        "--state-dir",
        str((workspace / ".commandagent-state").resolve()),
        "--offline",
        "--yes",
        "--intent",
        case["intent"],
        "--profile",
        case["profile"],
        "--provider",
        "ollama",
        "--model",
        model,
        "--plan-run",
        case["goal"],
    ]


def run_current_product_baseline(
    *,
    commandagent_bin: Path,
    workspace: Path,
    case: dict[str, Any],
    model: str,
    timeout_sec: int,
) -> dict[str, Any]:
    argv = build_product_argv(
        commandagent_bin=commandagent_bin,
        workspace=workspace,
        case=case,
        model=model,
    )
    started = time.monotonic_ns()
    try:
        completed = subprocess.run(
            argv,
            cwd=workspace,
            stdin=subprocess.DEVNULL,
            text=True,
            capture_output=True,
            timeout=timeout_sec,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        return {
            "status": "baseline_unavailable",
            "reason": "timeout",
            "argv": argv,
            "wall_time_ms": (time.monotonic_ns() - started) // 1_000_000,
            "stdout": error.stdout or "",
            "stderr": error.stderr or "",
        }
    runs_root = workspace / ".anvil" / "runs"
    run_dirs = (
        sorted(
            (path for path in runs_root.iterdir() if path.is_dir()),
            key=lambda path: path.stat().st_mtime_ns,
        )
        if runs_root.is_dir()
        else []
    )
    return {
        "status": "completed" if completed.returncode == 0 else "failed",
        "returncode": completed.returncode,
        "argv": argv,
        "wall_time_ms": (time.monotonic_ns() - started) // 1_000_000,
        "stdout": completed.stdout,
        "stderr": completed.stderr,
        "product_run_dir": str(run_dirs[-1]) if run_dirs else None,
    }


def _json_rows(path: Path) -> list[dict[str, Any]]:
    text = path.read_text(encoding="utf-8")
    try:
        value = json.loads(text)
    except json.JSONDecodeError:
        value = [json.loads(line) for line in text.splitlines() if line.strip()]
    if isinstance(value, list):
        return [row for row in value if isinstance(row, dict)]
    if isinstance(value, dict):
        for key in ("observations", "evidence", "checks"):
            if isinstance(value.get(key), list):
                return [row for row in value[key] if isinstance(row, dict)]
        return [value]
    return []


def extract_product_observations(
    run_dir: Path, *, replay: Replay | None = None, replay_cwd: Path | None = None
) -> list[dict[str, Any]]:
    observations: list[dict[str, Any]] = []
    events_path = run_dir / "events.jsonl"
    if events_path.is_file():
        for row in _json_rows(events_path):
            event = row.get("event", row.get("type"))
            # Normalization/substitution telemetry proves how a command was
            # rewritten, not that the rewritten command was executed.  Only
            # events carrying an explicit runtime result are observations.
            if event == "declarative_command_check_result":
                exit_code = row.get("observed_exit_code")
                executed = isinstance(exit_code, int) and not row.get("timed_out", False)
                observations.append(
                    {
                        "source": "events.jsonl",
                        "event": event,
                        "strategy": "command",
                        "kind": "exit_code",
                        "actual": exit_code,
                        "executed": executed,
                        "passed": executed and row.get("status") == "passed",
                        "strength": "runtime",
                        "argv": row.get("argv"),
                        "stdout": row.get("stdout"),
                        "stderr": row.get("stderr"),
                    }
                )
                continue
            if event != "runtime_bash_verify_command":
                continue
            argv = row.get("argv")
            exit_code = row.get("exit_status", row.get("exit_code"))
            observation = {
                "source": "events.jsonl",
                "event": event,
                "strategy": "command",
                "kind": "exit_code",
                "actual": exit_code,
                "executed": isinstance(exit_code, int),
                "passed": exit_code == 0,
                "strength": "runtime",
                "argv": argv,
            }
            if (
                replay is not None
                and isinstance(argv, list)
                and isinstance(exit_code, int)
            ):
                replayed = replay(argv, replay_cwd or run_dir)
                observation["replay"] = replayed
                if replayed.get("runner_error"):
                    observation["passed"] = False
                    observation["reason"] = "baseline_replay_error"
                elif replayed.get("exit_code") != exit_code:
                    observation["passed"] = False
                    observation["reason"] = "baseline_observation_inconsistent"
                else:
                    observation["stdout"] = replayed.get("stdout")
                    observation["stderr"] = replayed.get("stderr")
            observations.append(observation)
    evidence_dir = run_dir / "evidence"
    if evidence_dir.is_dir():
        for path in sorted(evidence_dir.glob("*.json")):
            family = path.stem
            for row in _json_rows(path):
                observations.append(_evidence_observation(family, row, path))
    return observations


def _evidence_observation(
    family: str, row: dict[str, Any], path: Path
) -> dict[str, Any]:
    executed = row.get("executed") is True or row.get("status") in {"pass", "passed"}
    passed = row.get("result") == "pass" or row.get("outcome") == "success" or executed
    kind = row.get("kind", row.get("observation_kind", "existing_binding"))
    if family.startswith(("browser-interaction", "browser-readiness")):
        strategy, strength = "interaction", "runtime"
    elif family.startswith("fetch-evidence"):
        strategy, strength = "http", "runtime"
    elif family.startswith(("cli-probe", "python-cli-behavior", "cli-case-binding")):
        strategy, strength = "command", "runtime"
    elif family.startswith(("fix-", "investigation-binding")):
        strategy, strength = "existing_fix_evidence", "runtime"
    elif kind == "file":
        strategy, strength = "file", "deterministic"
    else:
        strategy, strength = "command", "weak"
    return {
        "source": str(path),
        "family": family,
        "strategy": strategy,
        "kind": kind,
        "actual": row.get("actual", row.get("value", row.get("expected"))),
        "executed": executed,
        "passed": passed,
        "strength": strength,
        "raw": row,
    }


def score_baseline_observations(
    observations: list[dict[str, Any]], adapters: list[dict[str, Any]], *, case_id: str
) -> list[dict[str, Any]]:
    results = []
    for adapter in [row for row in adapters if row["case_id"] == case_id]:
        proposal = adapter["proposal"]
        matches = [
            row
            for row in observations
            if row.get("executed")
            and row.get("passed")
            and row.get("strategy") in proposal["strategies"]
            and row.get("kind") in proposal["observation_kinds"]
            and _value_matches(row, proposal)
        ]
        results.append(
            {
                "adapter_id": adapter["adapter_id"],
                "claim_id": adapter["claim_id"],
                "observation_match": bool(matches),
                "observed_strength": matches[0]["strength"] if matches else None,
                "match_count": len(matches),
            }
        )
    return results


def _value_matches(observation: dict[str, Any], proposal: dict[str, Any]) -> bool:
    if "expected_values" in proposal:
        values = {str(item) for item in proposal["expected_values"]}
        return (
            str(observation.get("actual")) in values
            or str(observation.get("stdout")) in values
        )
    if "expected_contains" in proposal:
        text = json.dumps(observation, ensure_ascii=False, sort_keys=True)
        return all(item in text for item in proposal["expected_contains"])
    return True
