#!/usr/bin/env python3
"""Eval trace helpers.

This module is eval-only. It turns runtime events and workspace snapshots into
causal debugging artifacts without changing CommandAgent runtime behavior.
"""

from __future__ import annotations

import difflib
import hashlib
import json
import shutil
import subprocess
from pathlib import Path


TRACE_SCHEMA_VERSION = "1.0"

TRACE_FIELD_NAMES = [
    "first_divergence_event_id",
    "first_divergence_phase_id",
    "first_divergence_step_id",
    "last_successful_contract",
    "last_successful_action",
    "last_successful_artifact",
    "planner_requests",
    "worker_requests",
    "model_requests",
    "tool_calls",
    "artifact_changes",
    "verifier_runs",
    "recovery_attempts",
    "input_tokens",
    "output_tokens",
    "observed_active_job",
    "derived_active_job",
    "rechecked_active_job",
    "observed_target_path",
    "derived_target_path",
    "rechecked_target_path",
    "observed_attempt_outcome",
    "derived_attempt_outcome",
    "rechecked_attempt_outcome",
]

PLAN_QUALITY_FIELD_NAMES = [
    "plan_quality_status",
    "plan_quality_plan_files",
    "plan_quality_phase_count",
    "plan_quality_step_count",
    "plan_quality_mutation_step_count",
    "plan_quality_verify_step_count",
    "plan_quality_expected_path_count",
    "plan_quality_owned_required_artifact_count",
    "plan_quality_missing_owner_count",
    "plan_quality_multi_owner_unit_count",
    "plan_quality_verify_mixed_with_mutation_count",
    "plan_quality_empty_instruction_count",
    "plan_quality_avg_instruction_chars",
    "plan_quality_responsibility_score",
    "plan_quality_clarity_score",
    "plan_quality_granularity_score",
    "plan_quality_verifier_separation_score",
    "plan_quality_overall_score",
    "plan_quality_notes",
]

DERIVED_LOG_SPECS = {
    "plans.jsonl": {
        "plan_generation.started",
        "plan_generation.finished",
        "plan.saved",
        "ultra_phase.started",
        "ultra_phase.finished",
        "ultra_phase.failed",
    },
    "steps.jsonl": {"step.started", "step.finished", "step.failed"},
    "model_calls.jsonl": {
        "model_request.started",
        "model_response.received",
        "parser_feedback.sent",
        "guard_feedback.sent",
    },
    "tool_calls.jsonl": {
        "tool_call.started",
        "tool_call.finished",
        "tool_result.truncated",
    },
    "artifacts.jsonl": {"artifact.status"},
    "verifier_runs.jsonl": {"verifier.started", "verifier.finished"},
    "recoveries.jsonl": {
        "recovery_task.started",
        "repair_attempt.started",
        "repair.exhausted",
    },
}

PLAN_QUALITY_APPLICABLE_MODES = {
    "plan-only",
    "ultra-plan-only",
    "plan-run",
    "ultra-plan-run",
}

MUTATION_STEP_KINDS = {"create", "edit", "setup", "repair"}

HEAVY_VERIFIER_MARKERS = [
    "cargo check",
    "cargo test",
    "cargo build",
    "npm run build",
    "next build",
    "pytest",
    "python -m pytest",
    "tsc",
    "go test",
    "mvn test",
    "gradle test",
]


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def stable_json(data) -> str:
    return json.dumps(data, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def write_json(path: Path, data) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def append_jsonl(path: Path, rows) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as handle:
        for row in rows:
            handle.write(json.dumps(row, sort_keys=True, ensure_ascii=False) + "\n")


def read_jsonl(path: Path) -> list[dict]:
    if not path.exists():
        return []
    rows = []
    with path.open(encoding="utf-8") as handle:
        for line in handle:
            line = line.strip()
            if not line:
                continue
            try:
                rows.append(json.loads(line))
            except json.JSONDecodeError:
                rows.append(
                    {
                        "schema_version": TRACE_SCHEMA_VERSION,
                        "event_type": "trace.parse_error",
                        "payload": {"line": line[:240]},
                    }
                )
    return rows


def empty_plan_quality(status: str, notes: str = "") -> dict:
    row = {name: "" for name in PLAN_QUALITY_FIELD_NAMES}
    row["plan_quality_status"] = status
    row["plan_quality_notes"] = notes
    for name in [
        "plan_quality_plan_files",
        "plan_quality_phase_count",
        "plan_quality_step_count",
        "plan_quality_mutation_step_count",
        "plan_quality_verify_step_count",
        "plan_quality_expected_path_count",
        "plan_quality_owned_required_artifact_count",
        "plan_quality_missing_owner_count",
        "plan_quality_multi_owner_unit_count",
        "plan_quality_verify_mixed_with_mutation_count",
        "plan_quality_empty_instruction_count",
        "plan_quality_avg_instruction_chars",
        "plan_quality_responsibility_score",
        "plan_quality_clarity_score",
        "plan_quality_granularity_score",
        "plan_quality_verifier_separation_score",
        "plan_quality_overall_score",
    ]:
        row[name] = "0"
    return row


def yaml_scalar(value: str) -> str:
    value = value.strip()
    if value in {"[]", "{}"}:
        return ""
    if len(value) >= 2 and value[0] == value[-1] and value[0] in {'"', "'"}:
        value = value[1:-1]
    return value.replace(r"\"", '"').replace(r"\'", "'").strip()


def parse_inline_list(value: str) -> list[str] | None:
    value = value.strip()
    if not (value.startswith("[") and value.endswith("]")):
        return None
    inner = value[1:-1].strip()
    if not inner:
        return []
    return [yaml_scalar(part) for part in inner.split(",") if yaml_scalar(part)]


def parse_plan_yaml_for_quality(path: Path) -> dict:
    text = path.read_text(encoding="utf-8", errors="replace")
    parsed = {
        "path": path.as_posix(),
        "required_artifacts": [],
        "steps": [],
        "phases": [],
    }
    current_section = ""
    current_list: tuple[str, str] | None = None
    current_step = None
    current_phase = None
    for raw in text.splitlines():
        line = raw.rstrip()
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        if stripped == "steps:":
            current_section = "steps"
            current_list = None
            continue
        if stripped == "phases:":
            current_section = "phases"
            current_list = None
            continue
        if stripped == "required_artifacts:":
            current_list = ("root", "required_artifacts")
            continue
        if stripped.startswith("required_artifacts:"):
            inline = parse_inline_list(stripped.split(":", 1)[1])
            if inline is not None:
                parsed["required_artifacts"].extend(inline)
            current_list = None
            continue
        if stripped.startswith("- id:"):
            value = yaml_scalar(stripped.split(":", 1)[1])
            if current_section == "phases":
                current_phase = {
                    "id": value,
                    "goal": "",
                    "owned_artifacts": [],
                    "preserve_artifacts": [],
                    "verify_only_artifacts": [],
                }
                parsed["phases"].append(current_phase)
                current_step = None
            else:
                current_step = {
                    "id": value,
                    "kind": "",
                    "instruction": "",
                    "expected_paths": [],
                    "verify": [],
                }
                parsed["steps"].append(current_step)
                current_phase = None
                current_section = "steps"
            current_list = None
            continue
        if stripped.startswith("- ") and current_list:
            value = yaml_scalar(stripped[2:])
            if not value:
                continue
            owner, field = current_list
            if owner == "root":
                parsed[field].append(value)
            elif owner == "step" and current_step is not None:
                current_step[field].append(value)
            elif owner == "phase" and current_phase is not None:
                current_phase[field].append(value)
            continue
        if current_section == "steps" and current_step is not None:
            if stripped.startswith("kind:"):
                current_step["kind"] = yaml_scalar(stripped.split(":", 1)[1])
                current_list = None
                continue
            if stripped.startswith("instruction:"):
                current_step["instruction"] = yaml_scalar(stripped.split(":", 1)[1])
                current_list = None
                continue
            if stripped.startswith("expected_paths:"):
                inline = parse_inline_list(stripped.split(":", 1)[1])
                if inline is not None:
                    current_step["expected_paths"].extend(inline)
                    current_list = None
                else:
                    current_list = ("step", "expected_paths")
                continue
            if stripped.startswith("verify:"):
                inline = parse_inline_list(stripped.split(":", 1)[1])
                if inline is not None:
                    current_step["verify"].extend(inline)
                    current_list = None
                else:
                    current_list = ("step", "verify")
                continue
        if current_section == "phases" and current_phase is not None:
            if stripped.startswith("goal:"):
                current_phase["goal"] = yaml_scalar(stripped.split(":", 1)[1])
                current_list = None
                continue
            for field in [
                "owned_artifacts",
                "preserve_artifacts",
                "verify_only_artifacts",
            ]:
                if stripped.startswith(f"{field}:"):
                    inline = parse_inline_list(stripped.split(":", 1)[1])
                    if inline is not None:
                        current_phase[field].extend(inline)
                        current_list = None
                    else:
                        current_list = ("phase", field)
                    break
            else:
                pass
            if current_list and current_list[0] == "phase":
                continue
    return parsed


def contains_heavy_verifier(command: str) -> bool:
    lower = command.lower()
    return any(marker in lower for marker in HEAVY_VERIFIER_MARKERS)


def clamp_score(value: int) -> int:
    return max(0, min(100, value))


def average_score(values: list[int]) -> int:
    if not values:
        return 0
    return round(sum(values) / len(values))


def plan_quality_from_workspace(
    workdir: Path, mode: str, expected_artifacts: list[str]
) -> dict:
    if mode not in PLAN_QUALITY_APPLICABLE_MODES:
        return empty_plan_quality("not_applicable", "mode_has_no_generated_plan")

    plan_dir = workdir / ".commandagent" / "plans"
    plan_files = sorted(plan_dir.glob("*.yaml")) if plan_dir.is_dir() else []
    if not plan_files:
        return empty_plan_quality("missing_plan", "no_plan_yaml_under_commandagent_plans")

    parsed_plans = []
    parse_errors = []
    for path in plan_files:
        try:
            parsed_plans.append(parse_plan_yaml_for_quality(path))
        except OSError as exc:
            parse_errors.append(f"{path.name}:{exc}")

    required = set(expected_artifacts)
    plan_required = set()
    owned_paths = set()
    expected_paths = []
    instruction_lengths = []
    empty_instruction_count = 0
    step_count = 0
    phase_count = 0
    mutation_step_count = 0
    verify_step_count = 0
    verifier_command_count = 0
    multi_owner_unit_count = 0
    verify_mixed_with_mutation_count = 0

    for plan in parsed_plans:
        plan_required.update(plan["required_artifacts"])
        for step in plan["steps"]:
            step_count += 1
            kind = step.get("kind", "")
            instruction = step.get("instruction", "")
            paths = step.get("expected_paths", [])
            verifiers = step.get("verify", [])
            expected_paths.extend(paths)
            verifier_command_count += len(verifiers)
            if instruction:
                instruction_lengths.append(len(instruction))
            else:
                empty_instruction_count += 1
            if kind in MUTATION_STEP_KINDS:
                mutation_step_count += 1
                owned_paths.update(paths)
                if any(contains_heavy_verifier(command) for command in verifiers):
                    verify_mixed_with_mutation_count += 1
            if kind == "verify":
                verify_step_count += 1
            if len(paths) >= 3:
                multi_owner_unit_count += 1
        for phase in plan["phases"]:
            phase_count += 1
            goal = phase.get("goal", "")
            owned = phase.get("owned_artifacts", [])
            owned_paths.update(owned)
            if goal:
                instruction_lengths.append(len(goal))
            else:
                empty_instruction_count += 1
            if len(owned) >= 4:
                multi_owner_unit_count += 1

    if not owned_paths and expected_paths:
        owned_paths.update(expected_paths)

    owned_required = sorted(path for path in required if path in owned_paths)
    missing_owner = sorted(path for path in required if path not in owned_paths)
    if required:
        responsibility_score = round(100 * len(owned_required) / len(required))
    else:
        responsibility_score = 100

    avg_instruction_chars = (
        round(sum(instruction_lengths) / len(instruction_lengths))
        if instruction_lengths
        else 0
    )
    if not instruction_lengths:
        clarity_score = 0
    else:
        clarity_score = 100
        clarity_score -= empty_instruction_count * 20
        if avg_instruction_chars < 20:
            clarity_score -= 20
        if avg_instruction_chars < 10:
            clarity_score -= 20
        clarity_score = clamp_score(clarity_score)

    if step_count == 0 and phase_count == 0:
        granularity_score = 0
    else:
        granularity_score = 100 - multi_owner_unit_count * 20
        if mode.startswith("ultra") and len(expected_artifacts) > 1 and phase_count < 2:
            granularity_score -= 15
        if step_count > 12:
            granularity_score -= 10
        granularity_score = clamp_score(granularity_score)

    if step_count == 0:
        verifier_score = 100
    else:
        verifier_score = 100 - verify_mixed_with_mutation_count * 25
        if expected_artifacts and verify_step_count == 0 and verifier_command_count == 0:
            verifier_score -= 15
        verifier_score = clamp_score(verifier_score)

    overall_score = average_score(
        [
            responsibility_score,
            clarity_score,
            granularity_score,
            verifier_score,
        ]
    )
    notes = []
    if missing_owner:
        notes.append("missing_owner=" + ",".join(missing_owner))
    if parse_errors:
        notes.append("parse_error=" + ",".join(parse_errors))
    if multi_owner_unit_count:
        notes.append("large_step_or_phase")
    if verify_mixed_with_mutation_count:
        notes.append("heavy_verifier_in_mutation_step")
    if not notes:
        notes.append("ok")
    if missing_owner or responsibility_score < 100 or overall_score < 60:
        status = "fail"
    elif (
        overall_score < 80
        or multi_owner_unit_count
        or verify_mixed_with_mutation_count
        or empty_instruction_count
    ):
        status = "warn"
    else:
        status = "pass"

    return {
        "plan_quality_status": status,
        "plan_quality_plan_files": str(len(plan_files)),
        "plan_quality_phase_count": str(phase_count),
        "plan_quality_step_count": str(step_count),
        "plan_quality_mutation_step_count": str(mutation_step_count),
        "plan_quality_verify_step_count": str(verify_step_count),
        "plan_quality_expected_path_count": str(len(set(expected_paths))),
        "plan_quality_owned_required_artifact_count": str(len(owned_required)),
        "plan_quality_missing_owner_count": str(len(missing_owner)),
        "plan_quality_multi_owner_unit_count": str(multi_owner_unit_count),
        "plan_quality_verify_mixed_with_mutation_count": str(
            verify_mixed_with_mutation_count
        ),
        "plan_quality_empty_instruction_count": str(empty_instruction_count),
        "plan_quality_avg_instruction_chars": str(avg_instruction_chars),
        "plan_quality_responsibility_score": str(responsibility_score),
        "plan_quality_clarity_score": str(clarity_score),
        "plan_quality_granularity_score": str(granularity_score),
        "plan_quality_verifier_separation_score": str(verifier_score),
        "plan_quality_overall_score": str(overall_score),
        "plan_quality_notes": ";".join(notes),
    }


def file_snapshot(root: Path) -> dict:
    files = []
    if not root.exists():
        return {"schema_version": TRACE_SCHEMA_VERSION, "root": str(root), "files": files}
    for path in sorted(p for p in root.rglob("*") if p.is_file()):
        rel = path.relative_to(root).as_posix()
        try:
            stat = path.stat()
            files.append(
                {
                    "path": rel,
                    "size": stat.st_size,
                    "sha256": sha256_file(path),
                }
            )
        except OSError as exc:
            files.append({"path": rel, "error": str(exc)})
    return {"schema_version": TRACE_SCHEMA_VERSION, "root": str(root), "files": files}


def snapshot_index(snapshot: dict) -> dict[str, dict]:
    return {entry["path"]: entry for entry in snapshot.get("files", []) if "path" in entry}


def artifact_changes(before: dict, after: dict) -> list[dict]:
    before_idx = snapshot_index(before)
    after_idx = snapshot_index(after)
    changes = []
    for path in sorted(set(before_idx) | set(after_idx)):
        old = before_idx.get(path)
        new = after_idx.get(path)
        if old and not new:
            status = "deleted"
        elif new and not old:
            status = "created"
        elif old.get("sha256") != new.get("sha256"):
            status = "modified"
        else:
            continue
        changes.append(
            {
                "schema_version": TRACE_SCHEMA_VERSION,
                "path": path,
                "status": status,
                "before_sha256": old.get("sha256") if old else "",
                "after_sha256": new.get("sha256") if new else "",
                "before_size": old.get("size") if old else None,
                "after_size": new.get("size") if new else None,
            }
        )
    return changes


def text_for_diff(path: Path, max_bytes: int = 512_000) -> list[str] | None:
    try:
        data = path.read_bytes()
    except OSError:
        return None
    if len(data) > max_bytes or b"\0" in data:
        return None
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError:
        return None
    return text.splitlines(keepends=True)


def write_changes_patch(workdir: Path, changes: list[dict], before_dir: Path, path: Path) -> None:
    lines: list[str] = []
    for change in changes:
        rel = change["path"]
        before_path = before_dir / rel
        after_path = workdir / rel
        before_text = [] if change["status"] == "created" else text_for_diff(before_path)
        after_text = [] if change["status"] == "deleted" else text_for_diff(after_path)
        if before_text is None or after_text is None:
            lines.append(
                f"Binary or large file changed: {rel} "
                f"{change.get('before_sha256', '')}->{change.get('after_sha256', '')}\n"
            )
            continue
        fromfile = f"a/{rel}" if change["status"] != "created" else "/dev/null"
        tofile = f"b/{rel}" if change["status"] != "deleted" else "/dev/null"
        lines.extend(difflib.unified_diff(before_text, after_text, fromfile, tofile))
    path.write_text("".join(lines), encoding="utf-8")


def copy_workspace_snapshot(workdir: Path, dest: Path) -> None:
    if dest.exists():
        shutil.rmtree(dest)
    if workdir.exists():
        shutil.copytree(workdir, dest, dirs_exist_ok=True)
    else:
        dest.mkdir(parents=True, exist_ok=True)


def copy_cases_snapshot(cases_dir: Path, dest: Path) -> None:
    if dest.exists():
        shutil.rmtree(dest)
    shutil.copytree(cases_dir, dest)


def script_hashes(repo: Path) -> dict:
    paths = [
        "scripts/eval_agent_slice.sh",
        "scripts/eval_case_schema.py",
        "scripts/eval_failure_observation.py",
        "scripts/eval_report.py",
        "scripts/eval_runtime_job_report.py",
        "scripts/eval_signoff.py",
        "scripts/eval_trace.py",
    ]
    result = {}
    for rel in paths:
        path = repo / rel
        result[rel] = sha256_file(path) if path.exists() else "missing"
    return result


def case_hashes(cases_dir: Path) -> list[dict]:
    return [
        {
            "path": path.relative_to(cases_dir).as_posix(),
            "sha256": sha256_file(path),
        }
        for path in sorted(cases_dir.rglob("*.yaml"))
    ]


def write_root_manifest(root: Path, repo: Path, cases_dir: Path, args) -> None:
    manifest = {
        "schema_version": TRACE_SCHEMA_VERSION,
        "cases_dir": str(cases_dir),
        "case_hashes": case_hashes(cases_dir),
        "script_hashes": script_hashes(repo),
        "runner": "scripts/eval_agent_slice.sh",
        "runs": args.runs,
        "provider": args.provider,
        "model": args.model,
        "timeout_mode": "none" if args.no_timeout else "bounded",
        "proof_modes": args.proof_mode or [],
    }
    write_json(root / "manifest.json", manifest)


def split_event_logs(run_dir: Path) -> dict:
    events = read_jsonl(run_dir / "events.jsonl")
    counts = {}
    for filename, event_types in DERIVED_LOG_SPECS.items():
        rows = [event for event in events if event.get("event_type") in event_types]
        append_jsonl(run_dir / filename, rows)
        counts[filename.removesuffix(".jsonl")] = len(rows)
    return counts


def event_count(events: list[dict], event_type: str) -> int:
    return sum(1 for event in events if event.get("event_type") == event_type)


def first_event(events: list[dict], event_types: set[str]) -> dict | None:
    for event in events:
        if event.get("event_type") in event_types:
            return event
    return None


def payload_text(event: dict, *keys: str) -> str:
    payload = event.get("payload") or {}
    parts = []
    for key in keys:
        value = payload.get(key)
        if value is not None:
            parts.append(str(value))
    return "\n".join(parts)


def classify_first_divergence(events: list[dict], reason: str) -> tuple[str, dict | None]:
    for event in events:
        if event.get("event_type") not in {"tool_call.finished", "step.failed", "session.error"}:
            continue
        text = payload_text(event, "error", "message")
        lower = text.lower()
        if "bash command is not read-only" in lower:
            if any(token in lower for token in ["cargo test", "npm run build", "pytest", "cargo check"]):
                return "verifier_requested_before_mutation", event
            if "compound shell" in lower or "&&" in lower or "||" in lower or ";" in lower:
                return "compound_read_check_requested", event
            return "tool_policy_rejected_action", event
        if "write may only mutate the step target artifacts" in lower:
            return "tool_policy_rejected_action", event
        if "edit target does not exist" in lower or "edit_target_not_found" in lower:
            return "edit_requested_for_missing_target", event
        if "plan lint failed" in lower:
            return "plan_correction_exhausted", event
        if "missing artifacts" in lower:
            return "final_answer_before_required_write", event
    if reason.startswith("missing:"):
        return "artifact_owner_step_finished_without_artifact", None
    if reason.startswith("plan_lint"):
        return "plan_correction_exhausted", None
    if reason != "ok" and event_count(events, "recovery_task.started") == 0:
        return "recovery_decision_not_executed", None
    if reason != "ok" and event_count(events, "recovery_task.started") > 0:
        return "recovery_executed_without_workspace_delta", None
    return "", None


def event_identity(event: dict | None) -> dict:
    if not event:
        return {
            "first_divergence_event_id": "",
            "first_divergence_phase_id": "",
            "first_divergence_step_id": "",
        }
    return {
        "first_divergence_event_id": str(event.get("event_id") or ""),
        "first_divergence_phase_id": str(event.get("phase_id") or ""),
        "first_divergence_step_id": str(event.get("step_id") or ""),
    }


def sum_usage(events: list[dict]) -> tuple[str, str]:
    input_tokens = 0
    output_tokens = 0
    has_input = False
    has_output = False
    for event in events:
        if event.get("event_type") != "model_response.received":
            continue
        usage = (event.get("payload") or {}).get("usage") or {}
        if isinstance(usage.get("input_tokens"), int):
            input_tokens += usage["input_tokens"]
            has_input = True
        if isinstance(usage.get("output_tokens"), int):
            output_tokens += usage["output_tokens"]
            has_output = True
    return (str(input_tokens) if has_input else "", str(output_tokens) if has_output else "")


def causal_summary(events: list[dict], changes: list[dict], success: bool, reason: str) -> dict:
    first_divergence, divergence_event = classify_first_divergence(events, reason)
    input_tokens, output_tokens = sum_usage(events)
    last_step = None
    last_artifact = None
    for event in events:
        if event.get("event_type") == "step.finished":
            last_step = event
        if event.get("event_type") == "artifact.status" and (event.get("payload") or {}).get("status") == "ok":
            last_artifact = event
    verifier_finished = [event for event in events if event.get("event_type") == "verifier.finished"]
    recovery_events = [event for event in events if event.get("event_type") == "recovery_task.started"]
    return {
        "schema_version": TRACE_SCHEMA_VERSION,
        "first_actionable_divergence": first_divergence,
        **event_identity(divergence_event),
        "last_successful_contract": "verifier" if any((event.get("payload") or {}).get("ok") for event in verifier_finished) else ("step" if last_step else ""),
        "last_successful_action": str(last_step.get("step_id") if last_step else ""),
        "last_successful_artifact": str((last_artifact.get("payload") or {}).get("path") if last_artifact else ""),
        "planner_requests": str(event_count(events, "plan_generation.started")),
        "worker_requests": str(event_count(events, "model_request.started")),
        "model_requests": str(event_count(events, "model_request.started")),
        "tool_calls": str(event_count(events, "tool_call.started")),
        "artifact_changes": str(len(changes)),
        "verifier_runs": str(event_count(events, "verifier.started")),
        "recovery_attempts": str(len(recovery_events)),
        "input_tokens": input_tokens,
        "output_tokens": output_tokens,
        "funnel": {
            "case_started": True,
            "ultra_plan_generated": any(
                event.get("event_type") == "plan_generation.finished"
                and (event.get("payload") or {}).get("kind") == "ultra_plan"
                for event in events
            ),
            "ultra_plan_valid": any(
                event.get("event_type") == "plan.saved"
                and (event.get("payload") or {}).get("kind") == "ultra_plan"
                for event in events
            ),
            "phase_plan_generated": any(
                event.get("event_type") == "plan_generation.finished"
                and (event.get("payload") or {}).get("kind") in {"phase_step_plan", "step_plan"}
                for event in events
            ),
            "phase_plan_lint_passed": any(event.get("event_type") == "plan.saved" for event in events),
            "worker_started": event_count(events, "model_request.started") > 0,
            "repository_evidence_observed": event_count(events, "tool_call.finished") > 0,
            "first_mutation_applied": len(changes) > 0,
            "owned_artifact_created": any(change["status"] == "created" for change in changes),
            "step_completed": event_count(events, "step.finished") > 0,
            "verifier_started": event_count(events, "verifier.started") > 0,
            "verifier_passed": any((event.get("payload") or {}).get("ok") for event in verifier_finished),
            "profile_verification_passed": event_count(events, "profile_verification.failed") == 0,
            "job_completed": success,
            "failure_observed": reason != "ok",
            "recovery_decision_created": bool(recovery_events),
            "recovery_target_admitted": any((event.get("payload") or {}).get("target_path") for event in recovery_events),
            "recovery_task_started": bool(recovery_events),
            "recovery_patch_applied": bool(recovery_events) and len(changes) > 0,
            "original_verifier_rerun": bool(recovery_events) and event_count(events, "verifier.started") > 0,
            "recovery_passed": success and bool(recovery_events),
        },
    }


def provenance_record(value, source: str, confidence: str = "observed") -> dict:
    return {"value": value or "", "source": source, "confidence": confidence}


def build_provenance(observed: dict, derived: dict, rechecked: dict | None = None) -> dict:
    keys = sorted(set(observed) | set(derived) | set(rechecked or {}))
    fields = {}
    for key in keys:
        if observed.get(key) not in {None, ""}:
            fields[key] = provenance_record(observed.get(key), "runtime_event", "observed")
        elif rechecked and rechecked.get(key) not in {None, ""}:
            fields[key] = provenance_record(rechecked.get(key), "recheck", "derived")
        else:
            fields[key] = provenance_record(derived.get(key, ""), "eval_projection", "derived")
    return {"schema_version": TRACE_SCHEMA_VERSION, "fields": fields}
