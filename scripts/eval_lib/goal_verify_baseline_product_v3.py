from __future__ import annotations

import hashlib
import json
import os
import shlex
import subprocess
import time
from collections.abc import Callable
from pathlib import Path
from typing import Any

Replay = Callable[[list[str], Path, int], dict[str, Any]]


def build_product_argv(
    *,
    commandagent_bin: Path,
    workspace: Path,
    case: dict[str, Any],
    model: str,
    completion_contract_path: Path | None = None,
    recovery_plan_auto_runs: int | None = None,
    execution_action: str = "plan_run",
) -> list[str]:
    argv = [
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
    ]
    if completion_contract_path is not None:
        argv.extend(["--completion-contract-json", str(completion_contract_path)])
    if recovery_plan_auto_runs is not None:
        if (
            not isinstance(recovery_plan_auto_runs, int)
            or isinstance(recovery_plan_auto_runs, bool)
            or recovery_plan_auto_runs < 0
        ):
            raise ValueError("recovery_plan_auto_runs must be a non-negative integer")
        argv.extend(["--recovery-plan-auto-runs", str(recovery_plan_auto_runs)])
    action_flag = {
        "plan_run": "--plan-run",
        "ultra_plan_run": "--ultra-plan-run",
    }.get(execution_action)
    if action_flag is None:
        raise ValueError(f"unsupported execution_action: {execution_action}")
    argv.extend([action_flag, _baseline_task_input(case)])
    return argv


def run_current_product_baseline(
    *,
    commandagent_bin: Path,
    workspace: Path,
    case: dict[str, Any],
    model: str,
    timeout_sec: int,
    completion_contract: dict[str, Any] | None = None,
    recovery_plan_auto_runs: int | None = None,
    execution_action: str = "plan_run",
    capture_recovery_boundary: bool = False,
) -> dict[str, Any]:
    completion_contract_path = None
    completion_contract_sha256 = None
    if completion_contract is not None:
        contract_text = json.dumps(
            completion_contract,
            ensure_ascii=False,
            separators=(",", ":"),
            sort_keys=True,
        )
        contract_dir = workspace / ".goal-verify-baseline"
        contract_dir.mkdir(parents=True, exist_ok=True)
        completion_contract_path = contract_dir / "completion-contract.json"
        completion_contract_path.write_text(contract_text + "\n", encoding="utf-8")
        completion_contract_sha256 = hashlib.sha256(
            contract_text.encode("utf-8")
        ).hexdigest()
    argv = build_product_argv(
        commandagent_bin=commandagent_bin,
        workspace=workspace,
        case=case,
        model=model,
        completion_contract_path=completion_contract_path,
        recovery_plan_auto_runs=recovery_plan_auto_runs,
        execution_action=execution_action,
    )
    operational_constraints = case.get("task_contract", {}).get(
        "operational_constraints"
    )
    operational_constraints_sha256 = (
        hashlib.sha256(
            json.dumps(
                operational_constraints,
                ensure_ascii=False,
                separators=(",", ":"),
                sort_keys=True,
            ).encode("utf-8")
        ).hexdigest()
        if isinstance(operational_constraints, dict)
        else None
    )
    started = time.monotonic_ns()
    environment = os.environ.copy()
    if capture_recovery_boundary:
        environment["COMMANDAGENT_CAPTURE_RECOVERY_BOUNDARY"] = "1"
    try:
        completed = subprocess.run(
            argv,
            cwd=workspace,
            env=environment,
            stdin=subprocess.DEVNULL,
            text=True,
            capture_output=True,
            timeout=timeout_sec,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        run_dirs = _product_run_dirs(workspace)
        product_run_dir = run_dirs[-1] if run_dirs else None
        completion_verify = _completion_verify_status(product_run_dir)
        return {
            "status": "baseline_unavailable",
            "reason": "timeout",
            "argv": argv,
            "wall_time_ms": (time.monotonic_ns() - started) // 1_000_000,
            "stdout": error.stdout or "",
            "stderr": error.stderr or "",
            "recovery_plan_auto_runs": recovery_plan_auto_runs,
            "operational_constraints_bound": operational_constraints is not None,
            "operational_constraints_sha256": operational_constraints_sha256,
            "completion_contract_bound": completion_contract is not None,
            "completion_contract_sha256": completion_contract_sha256,
            "product_run_dir": str(product_run_dir) if product_run_dir else None,
            "product_run_namespace": _product_run_namespace(workspace, product_run_dir),
            "resource_usage": _product_resource_usage(product_run_dir),
            "recovery_plan_attempts": _recovery_plan_attempts(
                product_run_dir,
                configured_runs=recovery_plan_auto_runs,
                process_status="timed_out",
            ),
            "terminal_status": _product_terminal_status(product_run_dir),
            "recovery_boundary": _recovery_boundary(product_run_dir),
            **completion_verify,
        }
    run_dirs = _product_run_dirs(workspace)
    product_run_dir = run_dirs[-1] if run_dirs else None
    return {
        "status": "completed" if completed.returncode == 0 else "failed",
        "returncode": completed.returncode,
        "argv": argv,
        "wall_time_ms": (time.monotonic_ns() - started) // 1_000_000,
        "stdout": completed.stdout,
        "stderr": completed.stderr,
        "recovery_plan_auto_runs": recovery_plan_auto_runs,
        "operational_constraints_bound": operational_constraints is not None,
        "operational_constraints_sha256": operational_constraints_sha256,
        "product_run_dir": str(product_run_dir) if product_run_dir else None,
        "product_run_namespace": (_product_run_namespace(workspace, product_run_dir)),
        "resource_usage": _product_resource_usage(product_run_dir),
        "recovery_plan_attempts": _recovery_plan_attempts(
            product_run_dir,
            configured_runs=recovery_plan_auto_runs,
            process_status=("succeeded" if completed.returncode == 0 else "failed"),
        ),
        "terminal_status": _product_terminal_status(product_run_dir),
        "recovery_boundary": _recovery_boundary(product_run_dir),
        "completion_contract_bound": completion_contract is not None,
        "completion_contract_sha256": completion_contract_sha256,
        **_completion_verify_status(product_run_dir),
    }


def _baseline_task_input(case: dict[str, Any]) -> str:
    constraints = case.get("task_contract", {}).get("operational_constraints")
    if not isinstance(constraints, dict):
        return case["goal"]
    encoded = json.dumps(
        constraints,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    )
    return (
        f"{case['goal']}\n\n"
        "Operational constraints shared with the verification candidate "
        "(execution constraints, not additional acceptance claims):\n"
        f"{encoded}"
    )


def _product_run_dirs(workspace: Path) -> list[Path]:
    roots = [
        workspace / ".commandagent" / "runs",
        workspace / ".anvil" / "runs",
        workspace / ".commandagent-state" / "runs",
    ]
    return sorted(
        (
            path
            for root in roots
            if root.is_dir()
            for path in root.iterdir()
            if path.is_dir()
        ),
        key=lambda path: path.stat().st_mtime_ns,
    )


def _product_run_namespace(workspace: Path, run_dir: Path | None) -> str | None:
    if run_dir is None:
        return None
    return str(run_dir.parent.parent.relative_to(workspace))


def _completion_verify_status(run_dir: Path | None) -> dict[str, Any]:
    if run_dir is None:
        return {
            "completion_verify_attempt_recorded": False,
            "completion_verify_passed": False,
        }
    events_path = run_dir / "events.jsonl"
    events = _json_rows(events_path) if events_path.is_file() else []
    attempts = [row for row in events if row.get("event") == "completion_verify"]
    return {
        "completion_verify_attempt_recorded": bool(attempts),
        "completion_verify_passed": any(row.get("ok") is True for row in attempts),
    }


def _product_resource_usage(run_dir: Path | None) -> dict[str, int | None]:
    empty: dict[str, int | None] = {
        "wall_time_ms": None,
        "input_tokens": None,
        "output_tokens": None,
        "total_tokens": None,
    }
    if run_dir is None:
        return empty
    events_path = run_dir / "events.jsonl"
    if not events_path.is_file():
        return empty
    profiles = [
        row.get("profile")
        for row in _json_rows(events_path)
        if row.get("event") == "time_profile" and isinstance(row.get("profile"), dict)
    ]
    if not profiles:
        return empty
    profile = profiles[-1]
    input_tokens = profile.get("prompt_eval_count")
    output_tokens = profile.get("eval_count")
    wall_time_ms = profile.get("total_ms")
    if not all(
        isinstance(value, int) and not isinstance(value, bool) and value >= 0
        for value in (input_tokens, output_tokens, wall_time_ms)
    ):
        return empty
    return {
        "wall_time_ms": wall_time_ms,
        "input_tokens": input_tokens,
        "output_tokens": output_tokens,
        "total_tokens": input_tokens + output_tokens,
    }


def _product_terminal_status(run_dir: Path | None) -> dict[str, Any]:
    empty = {
        "recorded": False,
        "event": None,
        "ok": None,
        "status": None,
        "failure_kind": None,
        "recovery_failure_kind": None,
        "recovery_handoff_kind": None,
        "structured_blockers": [],
        "stop_reason": None,
        "next_action": None,
        "recovery_ultra_plan_path": None,
    }
    if run_dir is None:
        return empty
    events_path = run_dir / "events.jsonl"
    if not events_path.is_file():
        return empty
    rows = _json_rows(events_path)
    terminal = [
        row for row in rows if row.get("event") in {"run_stop", "tui_command_stop"}
    ]
    if not terminal:
        return empty
    tui_terminal = [row for row in terminal if row.get("event") == "tui_command_stop"]
    row = tui_terminal[-1] if tui_terminal else terminal[-1]
    recovery_events = [
        event
        for event in rows
        if event.get("event") == "recovery_prompt_saved"
        and isinstance(event.get("failure_kind"), str)
    ]
    recovery = recovery_events[-1] if recovery_events else {}
    contract_events = [
        event for event in rows if event.get("event") == "plan_final_contract"
    ]
    contract = contract_events[-1] if contract_events else {}
    structured_blockers = _structured_terminal_blockers(contract)
    return {
        "recorded": True,
        "event": row.get("event"),
        "ok": row.get("ok") if isinstance(row.get("ok"), bool) else None,
        "status": row.get("status"),
        "failure_kind": row.get("failure_kind"),
        "recovery_failure_kind": recovery.get("failure_kind"),
        "recovery_handoff_kind": contract.get("recovery_handoff_kind"),
        "structured_blockers": structured_blockers,
        "stop_reason": row.get("stop_reason"),
        "next_action": row.get("recovery_next_action", row.get("next_action")),
        "recovery_ultra_plan_path": row.get("recovery_ultra_plan_path")
        or recovery.get("recovery_ultra_plan_path")
        or contract.get("recovery_ultra_plan_path"),
    }


def _recovery_boundary(run_dir: Path | None) -> dict[str, Any]:
    empty = {
        "status": "not_recorded",
        "workspace_relative_path": None,
        "file_count": None,
        "total_bytes": None,
        "snapshot_sha256": None,
        "initial_provider_usage": _empty_usage(),
        "recovery_provider_usage": _empty_usage(),
    }
    if run_dir is None:
        return empty
    events_path = run_dir / "events.jsonl"
    if not events_path.is_file():
        return empty
    rows = _json_rows(events_path)
    snapshot_rows = [
        row for row in rows if row.get("event") == "recovery_boundary_snapshot"
    ]
    if not snapshot_rows:
        return empty
    snapshot = snapshot_rows[-1]
    start_index = next(
        (
            index
            for index, row in enumerate(rows)
            if row.get("event") == "recovery_plan_auto_run_start"
        ),
        None,
    )
    if start_index is None:
        return {
            **empty,
            "status": snapshot.get("status", "invalid"),
            "workspace_relative_path": snapshot.get("workspace_relative_path"),
            "file_count": snapshot.get("file_count"),
            "total_bytes": snapshot.get("total_bytes"),
            "snapshot_sha256": snapshot.get("snapshot_sha256"),
        }
    return {
        "status": snapshot.get("status", "invalid"),
        "workspace_relative_path": snapshot.get("workspace_relative_path"),
        "file_count": snapshot.get("file_count"),
        "total_bytes": snapshot.get("total_bytes"),
        "snapshot_sha256": snapshot.get("snapshot_sha256"),
        "initial_provider_usage": _provider_usage(rows[:start_index]),
        "recovery_provider_usage": _provider_usage(rows[start_index + 1 :]),
    }


def _empty_usage() -> dict[str, int]:
    return {
        "wall_time_ms": 0,
        "input_tokens": 0,
        "output_tokens": 0,
        "total_tokens": 0,
    }


def _provider_usage(rows: list[dict[str, Any]]) -> dict[str, int]:
    turns = [row for row in rows if row.get("event") == "provider_turn_duration"]

    def total(field: str) -> int:
        return sum(
            value
            for row in turns
            if isinstance((value := row.get(field)), int)
            and not isinstance(value, bool)
            and value >= 0
        )

    input_tokens = total("prompt_eval_count")
    output_tokens = total("eval_count")
    return {
        "wall_time_ms": total("duration_ms"),
        "input_tokens": input_tokens,
        "output_tokens": output_tokens,
        "total_tokens": input_tokens + output_tokens,
    }


def _structured_terminal_blockers(contract: dict[str, Any]) -> list[str]:
    blockers = []
    for field in (
        "missing_capabilities",
        "inconclusive_reasons",
        "release_gate_reasons",
    ):
        values = contract.get(field)
        if isinstance(values, list):
            blockers.extend(value for value in values if isinstance(value, str))
    primary = contract.get("primary_reason")
    if isinstance(primary, str) and primary:
        blockers.append(primary)
    return sorted(set(blockers))


def _recovery_plan_attempts(
    run_dir: Path | None,
    *,
    configured_runs: int | None,
    process_status: str,
) -> dict[str, Any]:
    """Project product events into explicit initial/recovery attempt telemetry."""
    events_path = run_dir / "events.jsonl" if run_dir is not None else None
    events = _json_rows(events_path) if events_path and events_path.is_file() else []
    rows = [
        row
        for row in events
        if row.get("event")
        in {
            "recovery_plan_auto_run_configured",
            "recovery_plan_auto_run_start",
            "recovery_plan_auto_run_complete",
            "recovery_plan_auto_run_stopped",
        }
    ]
    attempts: dict[int, dict[str, Any]] = {
        0: {
            "attempt_index": 0,
            "kind": "initial",
            "status": "unknown",
            "stop_reason": None,
        }
    }
    terminal_reason = "disabled" if configured_runs == 0 else None
    for row in rows:
        event = row.get("event")
        current = row.get("recovery_plan_auto_run_current")
        if not isinstance(current, int) or isinstance(current, bool) or current < 0:
            continue
        stop_reason = row.get("recovery_plan_auto_run_stop_reason")
        current_attempt = attempts.setdefault(
            current,
            {
                "attempt_index": current,
                "kind": "initial" if current == 0 else "recovery",
                "status": "unknown",
                "stop_reason": None,
            },
        )
        if event == "recovery_plan_auto_run_configured":
            current_attempt["status"] = "running"
        elif event == "recovery_plan_auto_run_start":
            previous = attempts.setdefault(
                current - 1,
                {
                    "attempt_index": current - 1,
                    "kind": "initial" if current == 1 else "recovery",
                    "status": "unknown",
                    "stop_reason": None,
                },
            )
            previous["status"] = "failed_recoverable"
            previous["stop_reason"] = "recovery_started"
            current_attempt["status"] = "running"
            current_attempt["recovery_handoff_kind"] = row.get("recovery_handoff_kind")
            current_attempt["recovery_ultra_plan_path"] = row.get(
                "recovery_ultra_plan_path"
            )
        elif event == "recovery_plan_auto_run_complete":
            current_attempt["status"] = "succeeded"
            current_attempt["stop_reason"] = stop_reason
            terminal_reason = stop_reason
        elif event == "recovery_plan_auto_run_stopped":
            current_attempt["status"] = (
                "timed_out" if process_status == "timed_out" else "failed"
            )
            current_attempt["stop_reason"] = stop_reason
            terminal_reason = stop_reason

    if not rows or (
        attempts[0]["status"] == "running" and process_status != "succeeded"
    ):
        attempts[0]["status"] = process_status
    for attempt in attempts.values():
        if attempt["status"] == "running" and process_status == "timed_out":
            attempt["status"] = "timed_out"
            attempt["stop_reason"] = "process_timeout"
            terminal_reason = "process_timeout"

    event_limits = {
        row.get("recovery_plan_auto_runs")
        for row in rows
        if isinstance(row.get("recovery_plan_auto_runs"), int)
        and not isinstance(row.get("recovery_plan_auto_runs"), bool)
    }
    executed_runs = max(
        (
            row.get("recovery_plan_auto_run_current", 0)
            for row in rows
            if row.get("event") == "recovery_plan_auto_run_start"
            and isinstance(row.get("recovery_plan_auto_run_current"), int)
            and not isinstance(row.get("recovery_plan_auto_run_current"), bool)
        ),
        default=0,
    )
    return {
        "configured_recovery_runs": configured_runs,
        "executed_recovery_runs": executed_runs,
        "event_telemetry_available": bool(rows),
        "configured_matches_events": (
            not event_limits
            if configured_runs == 0
            else event_limits == {configured_runs}
        ),
        "attempts": [attempts[index] for index in sorted(attempts)],
        "terminal_stop_reason": terminal_reason,
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
    run_dir: Path,
    *,
    replay: Replay | None = None,
    replay_cwd: Path | None = None,
    completion_contract: dict[str, Any] | None = None,
) -> list[dict[str, Any]]:
    observations: list[dict[str, Any]] = []
    events_path = run_dir / "events.jsonl"
    events = []
    if events_path.is_file():
        events = _json_rows(events_path)
        for row in events:
            event = row.get("event", row.get("type"))
            # Normalization/substitution telemetry proves how a command was
            # rewritten, not that the rewritten command was executed.  Only
            # events carrying an explicit runtime result are observations.
            if event == "declarative_command_check_result":
                exit_code = row.get("observed_exit_code")
                executed = isinstance(exit_code, int) and not row.get(
                    "timed_out", False
                )
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
                replayed = replay(argv, replay_cwd or run_dir, 30_000)
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
    observations.extend(
        _completion_contract_observations(
            events=events,
            completion_contract=completion_contract,
            replay=replay,
            replay_cwd=replay_cwd or run_dir,
        )
    )
    evidence_dirs = [run_dir / "evidence"]
    if replay_cwd is not None:
        evidence_dirs.append(replay_cwd / "evidence")
    seen_evidence = set()
    for evidence_dir in evidence_dirs:
        if not evidence_dir.is_dir():
            continue
        for path in sorted(evidence_dir.glob("*.json")):
            resolved = path.resolve()
            if resolved in seen_evidence:
                continue
            seen_evidence.add(resolved)
            family = path.stem
            for row in _json_rows(path):
                observations.append(_evidence_observation(family, row, path))
    return observations


def _completion_contract_observations(
    *,
    events: list[dict[str, Any]],
    completion_contract: dict[str, Any] | None,
    replay: Replay | None,
    replay_cwd: Path,
) -> list[dict[str, Any]]:
    commands = (
        completion_contract.get("verify_commands", [])
        if isinstance(completion_contract, dict)
        else []
    )
    completed = any(
        row.get("event") == "completion_verify" and row.get("ok") is True
        for row in events
    )
    if not completed or not isinstance(commands, list):
        return []
    observations = []
    for command in commands:
        if not isinstance(command, str):
            continue
        try:
            argv = shlex.split(command)
        except ValueError:
            continue
        if not argv or any(token in {"&&", "||", ";", "|"} for token in argv):
            continue
        replayed = replay(argv, replay_cwd, 30_000) if replay is not None else {}
        replay_ok = not replayed.get("runner_error") and not replayed.get("timed_out")
        replay_ok = replay_ok and replayed.get("exit_code", 0) == 0
        base = {
            "source": "events.jsonl",
            "event": "completion_verify",
            "argv": argv,
            "executed": True,
            "passed": replay_ok,
            "strength": "runtime",
        }
        observations.append(
            {
                **base,
                "strategy": "command",
                "kind": "exit_code",
                "actual": replayed.get("exit_code", 0),
                "replay": replayed,
                **({"reason": "baseline_replay_error"} if not replay_ok else {}),
            }
        )
        if replay_ok:
            for kind in ("stdout", "stderr"):
                value = replayed.get(kind)
                if isinstance(value, str) and value:
                    observations.append(
                        {
                            **base,
                            "strategy": kind,
                            "kind": kind,
                            "actual": value,
                            kind: value,
                            "replay": replayed,
                        }
                    )
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
    elif family.startswith("investigation-binding"):
        strategy, strength = "existing_investigation_binding", "runtime"
    elif family.startswith("fix-"):
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
    binding = proposal.get("input_binding")
    if (
        isinstance(binding, dict)
        and isinstance(binding.get("strategies"), list)
        and observation.get("strategy") not in binding["strategies"]
    ):
        binding = None
    if isinstance(binding, dict) and not _baseline_input_matches(observation, binding):
        return False
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


def _baseline_input_matches(
    observation: dict[str, Any], binding: dict[str, Any]
) -> bool:
    kind = binding.get("kind")
    if kind in {"command", "fixture_command"}:
        return observation.get("argv") == binding.get("argv")
    raw = observation.get("raw")
    if not isinstance(raw, dict):
        return False
    if kind == "http":
        return all(
            raw.get(key) == binding.get(key) for key in ("method", "port", "path")
        )
    if kind == "dom":
        return all(
            raw.get(key) == expected
            for key, expected in binding.items()
            if key not in {"kind", "strategies"}
        )
    return False
