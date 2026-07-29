#!/usr/bin/env python3
"""Execute reproducible CommandAgent UAT measurement suites."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import time
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any

import tomllib
from id_vocabulary import INTERRUPTED_ENVIRONMENT

HARNESS_VERSION = "0.1"
TAIL_BYTES = 64 * 1024
MAX_SCRUB_FILE_BYTES = 10 * 1024 * 1024
TERMINAL_RUN_STATUSES = {
    "blocked",
    "completed",
    "dry-run-ready",
    INTERRUPTED_ENVIRONMENT,
}
WRAPPER_TOKENS = {"env", "nice", "nohup", "timeout"}
EVENT_SEARCH_PATTERNS = {
    "intent_resolved": re.compile(r"^intent_resolved$"),
    "host_env_normalized": re.compile(r"^host_env_normalized$"),
    "fix_reproducer_suggested": re.compile(r"^fix_reproducer_suggested$"),
    "*_plan_synthesized": re.compile(r"^[a-z0-9_]+_plan_synthesized$"),
    "*_adjudicated": re.compile(r"^[a-z0-9_]+_adjudicated$"),
}
SECRET_VALUE_PATTERNS = (
    re.compile(r"sk-[A-Za-z0-9]{16,}"),
    re.compile(r"AIza[0-9A-Za-z_-]{35}"),
    re.compile(r"ghp_[A-Za-z0-9]{36,}"),
    re.compile(r"xox[baprs]-[A-Za-z0-9-]{10,}"),
    re.compile(r"AKIA[0-9A-Z]{16}"),
    re.compile(r"eyJ[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{10,}"),
    re.compile(r"BEGIN [A-Z ]*PRIVATE KEY"),
)
NAME_VALUE_PATTERN = re.compile(
    r"(?i)(api[_-]?key|secret|token|authorization)\s*[:=]\s*[\"']?"
    r"([A-Za-z0-9+/_\-.]{16,})[\"']?"
)
ENV_DUMP_PATTERN = re.compile(r"^[A-Z][A-Z0-9_]*=.+$")
ENV_DUMP_ALLOW = {"NODE_ENV", "PATH", "HOME", "PWD", "OLDPWD", "SHELL", "USER"}


class BenchError(RuntimeError):
    """Raised when a harness protocol requirement is not satisfied."""


@dataclass(frozen=True)
class SourceSpec:
    set_id: str
    path: str
    copy: tuple[str, ...]
    input_sha256: dict[str, str]
    precheck_cmd: str
    precheck_expect: str
    precheck_pattern: str


@dataclass(frozen=True)
class RunSpec:
    name: str
    set_id: str | None
    goal_id: str
    executor: str


@dataclass(frozen=True)
class SuiteDefinition:
    path: Path
    suite_id: str
    profile: str
    intent: str
    plan_preset: str
    context_budget: int
    planner_model: str
    planner_provider: str
    provider: str
    min_head: str | None
    workspace_mode: str
    goals: dict[str, str]
    sources: tuple[SourceSpec, ...]
    runs: tuple[RunSpec, ...]
    scrub_allow: tuple[dict[str, str], ...]

    def source_for(self, set_id: str | None) -> SourceSpec:
        for source in self.sources:
            if source.set_id == set_id:
                return source
        raise BenchError(f"unknown source set: {set_id}")

    def source_for_run(self, run: RunSpec) -> SourceSpec | None:
        if self.workspace_mode == "empty":
            return None
        return self.source_for(run.set_id)


@dataclass(frozen=True)
class ProcurementResult:
    ok: bool
    reason: str | None
    observed_sha256: dict[str, str | None]
    precheck: dict[str, Any] | None
    workspace_integrity: dict[str, Any] | None = None


@dataclass(frozen=True)
class ScrubResult:
    ok: bool
    findings: tuple[dict[str, Any], ...]
    allows: tuple[dict[str, str], ...]


@dataclass(frozen=True)
class ProductResult:
    start_epoch: int
    end_epoch: int
    exit_code: int | None
    stdout_tail: str
    stderr_tail: str


class ProductInterrupted(KeyboardInterrupt):
    def __init__(self, result: ProductResult) -> None:
        super().__init__()
        self.result = result


def repository_root() -> Path:
    return Path(__file__).resolve().parents[3]


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _masked(value: str) -> str:
    return f"{value[:2]}…({len(value)} chars)"


def scrub_path(path: Path, scrub_allow: Sequence[dict[str, str]] = ()) -> ScrubResult:
    """Detect secrets, dangerous files, dumps, and bulky derivatives.

    The target is secret existence, not vocabulary: name mentions without a
    value are allowed. This is an E2-style precision refinement, not a
    relaxation; unconditional real-value patterns remain failures.
    """
    findings: list[dict[str, Any]] = []
    allow_records = tuple(scrub_allow)
    allow_patterns = [
        (re.compile(item["pattern"]), item["reason"]) for item in allow_records
    ]

    def add(kind: str, file_path: Path, detail: str, line: int | None = None) -> None:
        relative = (
            str(file_path.relative_to(path)) if file_path != path else file_path.name
        )
        if any(pattern.search(detail) for pattern, _ in allow_patterns):
            return
        record: dict[str, Any] = {"kind": kind, "path": relative, "detail": detail}
        if line is not None:
            record["line"] = line
        findings.append(record)

    files = (
        [path]
        if path.is_file()
        else [item for item in path.rglob("*") if item.is_file()]
    )
    for file_path in files:
        relative = (
            file_path.relative_to(path) if file_path != path else Path(file_path.name)
        )
        parts = set(relative.parts)
        if any(part in {"node_modules", ".next", "target"} for part in parts):
            add("derived", file_path, str(relative))
        if file_path.stat().st_size > MAX_SCRUB_FILE_BYTES:
            add("oversize", file_path, f"{file_path.stat().st_size} bytes")
        if (
            file_path.name == ".env"
            or file_path.name.startswith(".env.")
            or file_path.suffix == ".pem"
            or file_path.name.startswith("id_rsa")
        ):
            add("dangerous_file", file_path, str(relative))
        try:
            text = file_path.read_text(encoding="utf-8", errors="replace")
        except OSError as error:
            add("unreadable", file_path, str(error))
            continue
        text_lines = text.splitlines()
        dump_streak = 0
        for number, line in enumerate(text_lines, start=1):
            real_match = next(
                (
                    pattern.search(line)
                    for pattern in SECRET_VALUE_PATTERNS
                    if pattern.search(line)
                ),
                None,
            )
            if real_match and not any(
                pattern.search(real_match.group(0)) for pattern, _ in allow_patterns
            ):
                add("secret_value", file_path, _masked(real_match.group(0)), number)
            name_match = NAME_VALUE_PATTERN.search(line)
            if name_match:
                if not any(
                    pattern.search(name_match.group(2)) for pattern, _ in allow_patterns
                ):
                    add("named_value", file_path, _masked(name_match.group(2)), number)
            elif number < len(text_lines):
                next_match = NAME_VALUE_PATTERN.search(line + "\n" + text_lines[number])
                if next_match and next_match.group(2):
                    add(
                        "named_adjacent_value",
                        file_path,
                        _masked(next_match.group(2)),
                        number,
                    )
            if (
                ENV_DUMP_PATTERN.match(line)
                and line.split("=", 1)[0] not in ENV_DUMP_ALLOW
            ):
                dump_streak += 1
            else:
                dump_streak = 0
            if dump_streak >= 20:
                add(
                    "environment_dump",
                    file_path,
                    "20 consecutive uppercase assignments",
                    number - 19,
                )
                dump_streak = 0
    return ScrubResult(not findings, tuple(findings), allow_records)


def _require_table(parent: dict[str, Any], key: str) -> dict[str, Any]:
    value = parent.get(key)
    if not isinstance(value, dict):
        raise BenchError(f"TOML [{key}] table is required")
    return value


def _required_str(table: dict[str, Any], key: str, context: str) -> str:
    value = table.get(key)
    if not isinstance(value, str) or not value.strip():
        raise BenchError(f"{context}.{key} must be a non-empty string")
    return value


def _safe_relative_path(value: str, context: str) -> PurePosixPath:
    path = PurePosixPath(value)
    if (
        path.is_absolute()
        or not path.parts
        or any(part in {"", ".", ".."} for part in path.parts)
    ):
        raise BenchError(f"{context} must be a normalized relative path: {value!r}")
    if ".git" in path.parts:
        raise BenchError(f"{context} may not contain .git: {value!r}")
    return path


def _reject_unknown_keys(
    table: dict[str, Any], allowed: set[str], context: str
) -> None:
    unknown = sorted(set(table) - allowed)
    if unknown:
        raise BenchError(f"unknown {context} keys: {', '.join(unknown)}")


def load_suite(path: Path) -> SuiteDefinition:
    try:
        with path.open("rb") as handle:
            document = tomllib.load(handle)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise BenchError(f"cannot load suite {path}: {error}") from error

    suite_table = _require_table(document, "suite")
    goals_table = _require_table(document, "goals")
    _reject_unknown_keys(
        suite_table,
        {
            "id",
            "profile",
            "intent",
            "plan_preset",
            "context_budget",
            "planner_model",
            "planner_provider",
            "provider",
            "min_head",
            "workspace_mode",
            "scrub_allow",
        },
        "suite",
    )
    suite_id = _required_str(suite_table, "id", "suite")
    if not re.fullmatch(r"[a-z0-9][a-z0-9-]*", suite_id):
        raise BenchError("suite.id must use lowercase letters, digits, and hyphens")
    profile = _required_str(suite_table, "profile", "suite")
    intent = _required_str(suite_table, "intent", "suite")
    if intent not in {"create", "fix", "investigate"}:
        raise BenchError("suite.intent must be create, fix, or investigate")
    plan_preset = _required_str(suite_table, "plan_preset", "suite")
    if plan_preset not in {"default", "profile", "none"}:
        raise BenchError("suite.plan_preset must be default, profile, or none")
    context_budget = suite_table.get("context_budget")
    if not isinstance(context_budget, int) or context_budget <= 0:
        raise BenchError("suite.context_budget must be a positive integer")
    workspace_mode = suite_table.get("workspace_mode", "sourced")
    if not isinstance(workspace_mode, str) or workspace_mode not in {
        "empty",
        "sourced",
    }:
        raise BenchError("suite.workspace_mode must be empty or sourced")
    min_head_value = suite_table.get("min_head")
    if min_head_value is not None and (
        not isinstance(min_head_value, str) or not min_head_value.strip()
    ):
        raise BenchError("suite.min_head must be a non-empty string when present")
    raw_allow = suite_table.get("scrub_allow", [])
    if not isinstance(raw_allow, list):
        raise BenchError("suite.scrub_allow must be an array of tables")
    scrub_allow: list[dict[str, str]] = []
    for index, item in enumerate(raw_allow):
        if not isinstance(item, dict) or set(item) != {"pattern", "reason"}:
            raise BenchError(f"suite.scrub_allow[{index}] requires pattern and reason")
        pattern = _required_str(item, "pattern", f"suite.scrub_allow[{index}]")
        reason = _required_str(item, "reason", f"suite.scrub_allow[{index}]")
        try:
            re.compile(pattern)
        except re.error as error:
            raise BenchError(f"invalid suite.scrub_allow pattern: {error}") from error
        scrub_allow.append({"pattern": pattern, "reason": reason})

    goals: dict[str, str] = {}
    for goal_id, goal in goals_table.items():
        if not isinstance(goal_id, str) or not re.fullmatch(r"[a-z0-9_-]+", goal_id):
            raise BenchError(f"invalid goal id: {goal_id!r}")
        if not isinstance(goal, str) or not goal.strip():
            raise BenchError(f"goal {goal_id!r} must be a non-empty string")
        goals[goal_id] = goal
    if not goals:
        raise BenchError("suite must define at least one goal")

    raw_sources = document.get("sources")
    if workspace_mode == "empty" and raw_sources is not None:
        raise BenchError(
            "suite.workspace_mode empty may not define [[sources]] tables"
        )
    if workspace_mode == "sourced" and (
        not isinstance(raw_sources, list) or not raw_sources
    ):
        raise BenchError("suite must define at least one [[sources]] table")
    if raw_sources is None:
        raw_sources = []
    sources: list[SourceSpec] = []
    source_ids: set[str] = set()
    for index, raw_source in enumerate(raw_sources):
        context = f"sources[{index}]"
        if not isinstance(raw_source, dict):
            raise BenchError(f"{context} must be a table")
        _reject_unknown_keys(
            raw_source,
            {
                "set",
                "path",
                "copy",
                "input_sha256",
                "precheck_cmd",
                "precheck_expect",
                "precheck_pattern",
            },
            context,
        )
        set_id = _required_str(raw_source, "set", context)
        if set_id in source_ids:
            raise BenchError(f"duplicate source set: {set_id}")
        if not re.fullmatch(r"[a-z0-9][a-z0-9-]*", set_id):
            raise BenchError(f"invalid source set: {set_id!r}")
        source_ids.add(set_id)
        source_path = _required_str(raw_source, "path", context)
        _safe_relative_path(source_path, f"{context}.path")
        raw_copy = raw_source.get("copy")
        if not isinstance(raw_copy, list) or not raw_copy:
            raise BenchError(f"{context}.copy must be a non-empty list")
        copy_items: list[str] = []
        for item in raw_copy:
            if not isinstance(item, str):
                raise BenchError(f"{context}.copy entries must be strings")
            _safe_relative_path(item, f"{context}.copy")
            if item in copy_items:
                raise BenchError(f"duplicate {context}.copy entry: {item}")
            copy_items.append(item)
        raw_hashes = raw_source.get("input_sha256")
        if not isinstance(raw_hashes, dict) or not raw_hashes:
            raise BenchError(f"{context}.input_sha256 must be a non-empty table")
        input_hashes: dict[str, str] = {}
        for relative, expected in raw_hashes.items():
            if not isinstance(relative, str) or not isinstance(expected, str):
                raise BenchError(f"{context}.input_sha256 entries must be strings")
            _safe_relative_path(relative, f"{context}.input_sha256")
            if not re.fullmatch(r"[0-9a-f]{64}", expected):
                raise BenchError(
                    f"{context}.input_sha256[{relative!r}] must be lowercase SHA-256"
                )
            input_hashes[relative] = expected
        precheck_expect = _required_str(raw_source, "precheck_expect", context)
        if precheck_expect not in {"zero_exit", "nonzero_exit"}:
            raise BenchError(
                f"{context}.precheck_expect must be zero_exit or nonzero_exit"
            )
        precheck_pattern = _required_str(raw_source, "precheck_pattern", context)
        try:
            re.compile(precheck_pattern)
        except re.error as error:
            raise BenchError(f"invalid {context}.precheck_pattern: {error}") from error
        sources.append(
            SourceSpec(
                set_id=set_id,
                path=source_path,
                copy=tuple(copy_items),
                input_sha256=input_hashes,
                precheck_cmd=_required_str(raw_source, "precheck_cmd", context),
                precheck_expect=precheck_expect,
                precheck_pattern=precheck_pattern,
            )
        )

    raw_runs = document.get("runs")
    if not isinstance(raw_runs, list) or not raw_runs:
        raise BenchError("suite must define at least one [[runs]] table")
    runs: list[RunSpec] = []
    run_names: set[str] = set()
    for index, raw_run in enumerate(raw_runs):
        context = f"runs[{index}]"
        if not isinstance(raw_run, dict):
            raise BenchError(f"{context} must be a table")
        _reject_unknown_keys(raw_run, {"name", "set", "goal", "executor"}, context)
        name = _required_str(raw_run, "name", context)
        if not re.fullmatch(r"[a-z0-9][a-z0-9_-]*", name):
            raise BenchError(f"invalid run name: {name!r}")
        if name in run_names:
            raise BenchError(f"duplicate run name: {name}")
        run_names.add(name)
        if workspace_mode == "empty":
            if "set" in raw_run:
                raise BenchError(
                    f"{context}.set may not be defined for workspace_mode empty"
                )
            set_id = None
        else:
            set_id = _required_str(raw_run, "set", context)
        goal_id = _required_str(raw_run, "goal", context)
        if workspace_mode == "sourced" and set_id not in source_ids:
            raise BenchError(f"{context} references unknown source set: {set_id}")
        if goal_id not in goals:
            raise BenchError(f"{context} references unknown goal: {goal_id}")
        runs.append(
            RunSpec(
                name=name,
                set_id=set_id,
                goal_id=goal_id,
                executor=_required_str(raw_run, "executor", context),
            )
        )

    return SuiteDefinition(
        path=path.resolve(),
        suite_id=suite_id,
        profile=profile,
        intent=intent,
        plan_preset=plan_preset,
        context_budget=context_budget,
        planner_model=_required_str(suite_table, "planner_model", "suite"),
        planner_provider=_required_str(suite_table, "planner_provider", "suite"),
        provider=_required_str(suite_table, "provider", "suite"),
        min_head=min_head_value,
        workspace_mode=workspace_mode,
        goals=goals,
        sources=tuple(sources),
        runs=tuple(runs),
        scrub_allow=tuple(scrub_allow),
    )


def build_command(suite: SuiteDefinition, run: RunSpec) -> list[str]:
    command = [
        "commandagent",
        "--yes",
        "--intent",
        suite.intent,
        "--context-budget",
        str(suite.context_budget),
        "--model",
        run.executor,
        "--provider",
        suite.provider,
        "--planner-model",
        suite.planner_model,
        "--planner-provider",
        suite.planner_provider,
    ]
    if suite.plan_preset != "default":
        command.extend(["--plan-preset", suite.plan_preset])
    command.extend(
        [
            "--ultra-plan-run",
            "--profile",
            suite.profile,
            suite.goals[run.goal_id],
        ]
    )
    verify_unwrapped_command(command)
    return command


def verify_unwrapped_command(command: Sequence[str]) -> None:
    if not command or command[0] != "commandagent":
        raise BenchError("product argv must start directly with commandagent")
    mixed_wrappers = [token for token in command if token in WRAPPER_TOKENS]
    if mixed_wrappers:
        raise BenchError(f"wrapper token found in product argv: {mixed_wrappers[0]}")
    if any(token in {"|", ">", ">>", "<", "&&", "||", ";"} for token in command):
        raise BenchError("shell control token found in product argv")


def format_command(command: Sequence[str]) -> str:
    return shlex.join(command)


def _tail_bytes(path: Path) -> str:
    try:
        size = path.stat().st_size
        with path.open("rb") as handle:
            if size > TAIL_BYTES:
                handle.seek(-TAIL_BYTES, os.SEEK_END)
            return handle.read().decode("utf-8", errors="replace")
    except FileNotFoundError:
        return ""


def _tail_text(value: str) -> str:
    encoded = value.encode("utf-8", errors="replace")
    return encoded[-TAIL_BYTES:].decode("utf-8", errors="replace")


def _run_capture(argv: Sequence[str], cwd: Path) -> dict[str, Any]:
    started = int(time.time())
    try:
        result = subprocess.run(
            list(argv),
            cwd=cwd,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            check=False,
        )
    except OSError as error:
        return {
            "command_argv": list(argv),
            "command": format_command(argv),
            "start_epoch": started,
            "end_epoch": int(time.time()),
            "exit_code": None,
            "stdout_tail": "",
            "stderr_tail": str(error),
        }
    return {
        "command_argv": list(argv),
        "command": format_command(argv),
        "start_epoch": started,
        "end_epoch": int(time.time()),
        "exit_code": result.returncode,
        "stdout_tail": _tail_text(result.stdout),
        "stderr_tail": _tail_text(result.stderr),
    }


def _require_success(record: dict[str, Any], label: str) -> None:
    if record["exit_code"] != 0:
        detail = record["stderr_tail"] or record["stdout_tail"]
        raise BenchError(f"preflight {label} failed: {detail.strip()}")


def _check_git_clean(
    record: dict[str, Any], repo_root: Path, allowed_prefix: Path | None
) -> int:
    status_text = record["stdout_tail"]
    dirty = [line for line in status_text.splitlines() if line.strip()]
    allowed = 0
    unexpected: list[str] = []
    for line in dirty:
        relative = line[3:] if len(line) >= 3 else ""
        if allowed_prefix is not None and relative.startswith(
            str(allowed_prefix).rstrip("/") + "/"
        ):
            allowed += 1
        else:
            unexpected.append(line)
    record["dirty_entries"] = len(dirty)
    record["self_output_entries_allowed"] = allowed
    record["unexpected_dirty_entries"] = unexpected
    if unexpected:
        raise BenchError(
            "preflight git status failed: repository has unexpected changes:\n"
            + "\n".join(unexpected)
        )
    return allowed


def perform_preflight(
    repo_root: Path,
    min_head: str | None,
    skip_suite_tests: bool,
    allowed_output_dir: Path | None = None,
) -> tuple[dict[str, Any], list[dict[str, str]]]:
    records: dict[str, Any] = {"started_epoch": int(time.time())}
    deviations: list[dict[str, str]] = []

    git_status = _run_capture(
        ["git", "status", "--porcelain", "--untracked-files=all"], repo_root
    )
    records["git_status"] = git_status
    _require_success(git_status, "git status")
    allowed_prefix = None
    if allowed_output_dir is not None:
        try:
            allowed_prefix = allowed_output_dir.resolve().relative_to(
                repo_root.resolve()
            )
        except ValueError:
            allowed_prefix = None
    allowed_count = _check_git_clean(git_status, repo_root, allowed_prefix)
    print(f"preflight: git status clean (self-output entries allowed: {allowed_count})")

    head = _run_capture(["git", "rev-parse", "HEAD"], repo_root)
    records["head"] = head
    _require_success(head, "HEAD resolution")
    records["head_sha"] = head["stdout_tail"].strip()
    head_log = _run_capture(["git", "log", "-1", "--oneline"], repo_root)
    records["head_log"] = head_log
    _require_success(head_log, "HEAD log")
    print(f"preflight: HEAD {head_log['stdout_tail'].strip()}")

    records["min_head"] = min_head
    if min_head:
        ancestor = _run_capture(
            ["git", "merge-base", "--is-ancestor", min_head, "HEAD"], repo_root
        )
        records["ancestor"] = ancestor
        _require_success(ancestor, f"ancestor {min_head}")
        print(f"preflight: ancestor {min_head} verified")

    if skip_suite_tests:
        records["cargo_test"] = {
            "command_argv": ["cargo", "test"],
            "command": "cargo test",
            "skipped": True,
        }
        deviations.append(
            {
                "code": "preflight_suite_tests_skipped",
                "detail": "cargo test omitted by --skip-suite-tests",
            }
        )
        print("preflight: cargo test skipped (deviation recorded)")
    else:
        cargo_test = _run_capture(["cargo", "test"], repo_root)
        records["cargo_test"] = cargo_test
        _require_success(cargo_test, "cargo test")
        print("preflight: cargo test green")

    release_build = _run_capture(["cargo", "build", "--release"], repo_root)
    records["release_build"] = release_build
    _require_success(release_build, "release build")
    print("preflight: release build green")

    built_binary = repo_root / "target" / "release" / "commandagent"
    if not built_binary.is_file():
        raise BenchError(f"preflight release binary missing: {built_binary}")
    install_dir = Path.home() / ".local" / "bin"
    try:
        install_dir.mkdir(parents=True, exist_ok=True)
    except OSError as error:
        raise BenchError(f"preflight install directory failed: {error}") from error
    installed_binary = install_dir / "commandagent"
    install = _run_capture(
        ["install", "-m", "755", str(built_binary), str(installed_binary)], repo_root
    )
    records["install"] = install
    _require_success(install, "release install")
    records["binary_sha256"] = {
        "built": sha256_file(built_binary),
        "installed": sha256_file(installed_binary),
    }
    if records["binary_sha256"]["built"] != records["binary_sha256"]["installed"]:
        raise BenchError(
            "preflight installed binary SHA-256 differs from release build"
        )

    resolved_binary = shutil.which("commandagent")
    records["path_commandagent"] = resolved_binary
    if (
        resolved_binary is None
        or Path(resolved_binary).resolve() != installed_binary.resolve()
    ):
        raise BenchError(
            "preflight PATH commandagent does not resolve to ~/.local/bin/commandagent"
        )
    version = _run_capture(["commandagent", "--version"], repo_root)
    records["version"] = version
    _require_success(version, "installed --version")
    version_text = (version["stdout_tail"] + version["stderr_tail"]).strip()
    records["version_text"] = version_text
    if "+dirty" in version_text:
        raise BenchError(f"preflight dirty release version rejected: {version_text}")
    print(f"preflight: installed {version_text}")

    node_env = _run_capture(["printenv", "NODE_ENV"], repo_root)
    records["node_env"] = {
        **node_env,
        "value": node_env["stdout_tail"].rstrip("\n")
        if node_env["exit_code"] == 0
        else None,
    }
    print(f"preflight: NODE_ENV={records['node_env']['value']!r}")
    records["completed_epoch"] = int(time.time())
    return records, deviations


def _source_root(repo_root: Path, source: SourceSpec) -> Path:
    candidate = (repo_root / source.path).resolve()
    try:
        candidate.relative_to(repo_root.resolve())
    except ValueError as error:
        raise BenchError(f"source escapes repository: {source.path}") from error
    return candidate


def _assert_copy_source_safe(path: Path) -> None:
    candidates = [path]
    if path.is_dir():
        candidates.extend(path.rglob("*"))
    for candidate in candidates:
        if candidate.is_symlink():
            raise BenchError(f"source copy contains a symlink: {candidate}")
        if ".git" in candidate.parts:
            raise BenchError(f"source copy contains .git: {candidate}")


def _hash_relative_paths(
    directory: Path, relatives: Sequence[str]
) -> dict[str, str | None]:
    hashes: dict[str, str | None] = {}
    for relative in relatives:
        candidate = directory / relative
        hashes[relative] = sha256_file(candidate) if candidate.is_file() else None
    return hashes


def _empty_workspace_entries(run_dir: Path) -> tuple[str, ...]:
    return tuple(sorted(item.name for item in run_dir.iterdir()))


def _procure_empty_run(run_dir: Path) -> ProcurementResult:
    integrity: dict[str, Any] = {
        "workspace_mode": "empty",
        "created": False,
        "checked": False,
        "empty": False,
        "entry_count": None,
        "entries": [],
    }
    try:
        run_dir.mkdir(parents=True, exist_ok=False)
        integrity["created"] = True
        entries = _empty_workspace_entries(run_dir)
        integrity.update(
            {
                "checked": True,
                "empty": not entries,
                "entry_count": len(entries),
                "entries": list(entries),
            }
        )
        if entries:
            return ProcurementResult(
                False,
                f"empty workspace integrity check failed: entries={list(entries)}",
                {},
                None,
                integrity,
            )
        return ProcurementResult(True, None, {}, None, integrity)
    except OSError as error:
        return ProcurementResult(
            False,
            f"empty workspace creation failed: {error}",
            {},
            None,
            integrity,
        )


def procure_run(
    suite: SuiteDefinition, run: RunSpec, repo_root: Path, run_dir: Path
) -> ProcurementResult:
    if suite.workspace_mode == "empty":
        return _procure_empty_run(run_dir)
    source = suite.source_for(run.set_id)
    observed: dict[str, str | None] = {}
    try:
        source_root = _source_root(repo_root, source)
        if not source_root.is_dir():
            raise BenchError(f"source set does not exist: {source.path}")
        run_dir.mkdir(parents=True, exist_ok=False)
        for relative in source.copy:
            source_item = source_root / relative
            if not source_item.exists():
                raise BenchError(f"source copy item does not exist: {source_item}")
            _assert_copy_source_safe(source_item)
            destination = run_dir / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            if source_item.is_dir():
                shutil.copytree(source_item, destination)
            else:
                shutil.copy2(source_item, destination)
        observed = _hash_relative_paths(run_dir, list(source.input_sha256))
        mismatches = [
            relative
            for relative, expected in source.input_sha256.items()
            if observed.get(relative) != expected
        ]
        if mismatches:
            detail = ", ".join(
                f"{relative}: expected {source.input_sha256[relative]}, "
                f"observed {observed.get(relative)}"
                for relative in mismatches
            )
            return ProcurementResult(
                False, f"SHA-256 mismatch: {detail}", observed, None
            )

        precheck_argv = shlex.split(source.precheck_cmd)
        if not precheck_argv:
            raise BenchError("precheck command is empty after lexical parsing")
        precheck = _run_capture(precheck_argv, run_dir)
        (run_dir / "precheck.stdout.log").write_text(
            precheck["stdout_tail"], encoding="utf-8"
        )
        (run_dir / "precheck.stderr.log").write_text(
            precheck["stderr_tail"], encoding="utf-8"
        )
        (run_dir / "precheck.exit-code.txt").write_text(
            f"{precheck['exit_code']}\n", encoding="utf-8"
        )
        exit_matches = (
            precheck["exit_code"] == 0
            if source.precheck_expect == "zero_exit"
            else precheck["exit_code"] is not None and precheck["exit_code"] != 0
        )
        combined = precheck["stdout_tail"] + "\n" + precheck["stderr_tail"]
        pattern_matches = re.search(source.precheck_pattern, combined) is not None
        precheck["expectation"] = source.precheck_expect
        precheck["pattern"] = source.precheck_pattern
        precheck["exit_matches"] = exit_matches
        precheck["pattern_matches"] = pattern_matches
        if not exit_matches or not pattern_matches:
            return ProcurementResult(
                False,
                "precheck mismatch: "
                f"expect={source.precheck_expect} exit={precheck['exit_code']} "
                f"pattern={source.precheck_pattern!r} matched={pattern_matches}",
                observed,
                precheck,
            )
        return ProcurementResult(True, None, observed, precheck)
    except (BenchError, OSError) as error:
        return ProcurementResult(False, str(error), observed, None)


def run_product(
    command: Sequence[str], run_dir: Path, console_path: Path, start_epoch: int
) -> ProductResult:
    verify_unwrapped_command(command)
    stdout_path = run_dir / ".bench-product-stdout"
    stderr_path = run_dir / ".bench-product-stderr"
    process: subprocess.Popen[bytes] | None = None
    interrupted = False
    exit_code: int | None = None
    try:
        with console_path.open("w", encoding="utf-8") as console:
            console.write(f"start_epoch: {start_epoch}\n")
            console.write(f"command: {format_command(command)}\n")
        with (
            stdout_path.open("wb") as stdout_handle,
            stderr_path.open("wb") as stderr_handle,
        ):
            process = subprocess.Popen(
                list(command),
                cwd=run_dir,
                stdout=stdout_handle,
                stderr=stderr_handle,
            )
            try:
                exit_code = process.wait()
            except KeyboardInterrupt:
                interrupted = True
                if process.poll() is None:
                    process.terminate()
                    try:
                        exit_code = process.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        exit_code = process.wait()
        end_epoch = int(time.time())
        stdout_tail = _tail_bytes(stdout_path)
        stderr_tail = _tail_bytes(stderr_path)
        with console_path.open("a", encoding="utf-8") as console:
            console.write(f"end_epoch: {end_epoch}\n")
            console.write(f"product_exit: {exit_code}\n")
            console.write("--- stdout tail ---\n")
            console.write(stdout_tail)
            if stdout_tail and not stdout_tail.endswith("\n"):
                console.write("\n")
            console.write("--- stderr tail ---\n")
            console.write(stderr_tail)
            if stderr_tail and not stderr_tail.endswith("\n"):
                console.write("\n")
        result = ProductResult(
            start_epoch=start_epoch,
            end_epoch=end_epoch,
            exit_code=exit_code,
            stdout_tail=stdout_tail,
            stderr_tail=stderr_tail,
        )
        if interrupted:
            raise ProductInterrupted(result)
        return result
    finally:
        for temporary in (stdout_path, stderr_path):
            try:
                temporary.unlink()
            except FileNotFoundError:
                pass


def _copy_to_artifact(source: Path, destination: Path) -> None:
    if not source.exists() and not source.is_symlink():
        return
    destination.parent.mkdir(parents=True, exist_ok=True)
    if source.is_dir() and not source.is_symlink():
        shutil.copytree(source, destination, dirs_exist_ok=True, symlinks=True)
    else:
        shutil.copy2(source, destination, follow_symlinks=False)


def archive_run(
    source: SourceSpec | None, run_dir: Path, artifact_dir: Path
) -> None:
    artifact_dir.mkdir(parents=True, exist_ok=True)
    if source is None:
        for item in sorted(run_dir.iterdir(), key=lambda path: path.name):
            _copy_to_artifact(item, artifact_dir / item.name)
        return
    _copy_to_artifact(run_dir / ".anvil", artifact_dir / ".anvil")
    for relative in source.copy:
        _copy_to_artifact(run_dir / relative, artifact_dir / relative)
    for name in (
        "precheck.stdout.log",
        "precheck.stderr.log",
        "precheck.exit-code.txt",
        "uat-console.log",
    ):
        _copy_to_artifact(run_dir / name, artifact_dir / name)


def _summary_value(text: str, prefixes: Sequence[str]) -> tuple[str | None, str | None]:
    for prefix in prefixes:
        match = re.search(rf"(?m)^{re.escape(prefix)}(.*)$", text)
        if match:
            return match.group(1).strip(), prefix.rstrip()
    return None, None


def collect_observations(artifact_dir: Path) -> dict[str, Any]:
    counts = {label: 0 for label in EVENT_SEARCH_PATTERNS}
    exact_events: dict[str, int] = {}
    event_files: list[str] = []
    parse_errors: list[str] = []
    for events_path in sorted(artifact_dir.rglob("events.jsonl")):
        relative_parts = events_path.relative_to(artifact_dir).parts
        if ".anvil" not in relative_parts or "runs" not in relative_parts:
            continue
        event_files.append(str(events_path.relative_to(artifact_dir)))
        with events_path.open(encoding="utf-8", errors="replace") as handle:
            for line_number, line in enumerate(handle, start=1):
                try:
                    event = json.loads(line)
                except json.JSONDecodeError as error:
                    parse_errors.append(
                        f"{events_path.relative_to(artifact_dir)}:{line_number}: {error}"
                    )
                    continue
                event_name = event.get("event")
                if not isinstance(event_name, str):
                    continue
                exact_events[event_name] = exact_events.get(event_name, 0) + 1
                for label, pattern in EVENT_SEARCH_PATTERNS.items():
                    if pattern.fullmatch(event_name):
                        counts[label] += 1

    summaries = [
        path
        for path in artifact_dir.rglob("summary.md")
        if ".anvil" in path.relative_to(artifact_dir).parts
        and "runs" in path.relative_to(artifact_dir).parts
    ]
    summary_path = (
        max(summaries, key=lambda path: path.stat().st_mtime) if summaries else None
    )
    verdict = assurance = terminal_reason = None
    verdict_source = assurance_source = terminal_reason_source = None
    if summary_path is not None:
        summary_text = summary_path.read_text(encoding="utf-8", errors="replace")
        verdict, verdict_source = _summary_value(
            summary_text, ("Verdict:", "Task status:", "Status:")
        )
        assurance, assurance_source = _summary_value(summary_text, ("Assurance:",))
        terminal_reason, terminal_reason_source = _summary_value(
            summary_text, ("Stop reason:",)
        )
    return {
        "event_search": {
            "file_glob": ".anvil/runs/**/events.jsonl",
            "field": "event",
            "patterns": {
                label: pattern.pattern
                for label, pattern in EVENT_SEARCH_PATTERNS.items()
            },
            "files": event_files,
            "parse_errors": parse_errors,
        },
        "event_counts": counts,
        "exact_event_counts": dict(sorted(exact_events.items())),
        "summary_path": str(summary_path.relative_to(artifact_dir))
        if summary_path
        else None,
        "verdict": verdict,
        "verdict_source": verdict_source,
        "assurance": assurance,
        "assurance_source": assurance_source,
        "terminal_reason": terminal_reason,
        "terminal_reason_source": terminal_reason_source,
    }


def write_metadata(path: Path, metadata: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.tmp")
    temporary.write_text(
        json.dumps(metadata, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    temporary.replace(path)


def _suite_metadata(suite: SuiteDefinition) -> dict[str, Any]:
    metadata = {
        "id": suite.suite_id,
        "path": str(suite.path),
        "sha256": sha256_file(suite.path),
        "profile": suite.profile,
        "intent": suite.intent,
        "plan_preset": suite.plan_preset,
        "context_budget": suite.context_budget,
        "planner_model": suite.planner_model,
        "planner_provider": suite.planner_provider,
        "provider": suite.provider,
        "min_head": suite.min_head,
        "scrub_allow": list(suite.scrub_allow),
    }
    if suite.workspace_mode != "sourced":
        metadata["workspace_mode"] = suite.workspace_mode
    return metadata


def _new_run_metadata(suite: SuiteDefinition, run: RunSpec) -> dict[str, Any]:
    source = suite.source_for_run(run)
    record = {"name": run.name}
    if run.set_id is not None:
        record["set"] = run.set_id
    record.update(
        {
            "goal": run.goal_id,
            "executor": run.executor,
            "status": "pending",
            "command_argv": build_command(suite, run),
            "command": format_command(build_command(suite, run)),
            "input_sha256_expected": source.input_sha256 if source else {},
        }
    )
    return record


def new_metadata(
    suite: SuiteDefinition,
    campaign_id: str,
    mode: str,
    repo_root: Path,
    preflight: dict[str, Any],
    deviations: list[dict[str, str]],
) -> dict[str, Any]:
    return {
        "schema_version": "1",
        "harness_version": HARNESS_VERSION,
        "campaign_id": campaign_id,
        "mode": mode,
        "repository_root": str(repo_root),
        "suite": _suite_metadata(suite),
        "preflight": preflight,
        "deviations": deviations,
        "created_epoch": int(time.time()),
        "runs": [_new_run_metadata(suite, run) for run in suite.runs],
    }


def _metadata_run(metadata: dict[str, Any], name: str) -> dict[str, Any]:
    for record in metadata["runs"]:
        if record["name"] == name:
            return record
    raise BenchError(f"metadata is missing run: {name}")


def _record_procurement(record: dict[str, Any], result: ProcurementResult) -> None:
    record["input_sha256_observed"] = result.observed_sha256
    record["precheck"] = result.precheck
    if result.workspace_integrity is not None:
        record["workspace_integrity"] = result.workspace_integrity
    if result.reason:
        record["protocol_reason"] = result.reason


def _finish_run_record(
    record: dict[str, Any],
    run_dir: Path,
    source: SourceSpec | None,
    artifact_dir: Path,
    scrub_allow: Sequence[dict[str, str]] = (),
) -> None:
    expected_hashes = source.input_sha256 if source else {}
    record["final_sha256"] = _hash_relative_paths(run_dir, list(expected_hashes))
    archive_run(source, run_dir, artifact_dir)
    try:
        from acceptance_sheet import generate as generate_acceptance_sheet

        (artifact_dir / "acceptance-sheet.md").write_text(
            generate_acceptance_sheet(artifact_dir), encoding="utf-8"
        )
        record["sheet_generated"] = True
    except (ImportError, OSError, ValueError) as error:
        record["sheet_generated"] = False
        record["sheet_generation_failed"] = str(error)
        print(f"warning: acceptance sheet failed for {record['name']}: {error}")
    scrub = scrub_path(artifact_dir, scrub_allow)
    record["scrub"] = {
        "ok": scrub.ok,
        "findings": list(scrub.findings),
        "allow": list(scrub.allows),
    }
    if not scrub.ok:
        record["scrub_failed"] = True
        print(f"warning: scrub findings for {record['name']}: {len(scrub.findings)}")
    record.update(collect_observations(artifact_dir))


def normalize_interrupted_runs(
    suite: SuiteDefinition,
    campaign_dir: Path,
    metadata: dict[str, Any],
    metadata_path: Path,
) -> None:
    changed = False
    for run in suite.runs:
        record = _metadata_run(metadata, run.name)
        if record.get("status") not in {"running", "starting"}:
            continue
        record["status"] = INTERRUPTED_ENVIRONMENT
        record["end_epoch"] = int(time.time())
        record["interruption_reason"] = (
            "resume observed a run without a recorded product terminal"
        )
        run_dir = campaign_dir / "workspaces" / run.name
        artifact_dir = campaign_dir / "artifacts" / run.name
        if run_dir.exists():
            _finish_run_record(
                record,
                run_dir,
                suite.source_for_run(run),
                artifact_dir,
                suite.scrub_allow,
            )
        changed = True
    if changed:
        metadata.setdefault("resume_notes", []).append(
            f"Interrupted runs were recorded as {INTERRUPTED_ENVIRONMENT} and were not rerun. "
            "A one-time rerun in a new directory requires review adjudication."
        )
        write_metadata(metadata_path, metadata)


def process_runs(
    suite: SuiteDefinition,
    repo_root: Path,
    campaign_dir: Path,
    metadata: dict[str, Any],
    *,
    dry_run: bool,
    resume: bool,
) -> None:
    metadata_path = campaign_dir / "uat-meta.json"
    if resume:
        normalize_interrupted_runs(suite, campaign_dir, metadata, metadata_path)
    for run in suite.runs:
        record = _metadata_run(metadata, run.name)
        command = build_command(suite, run)
        if resume and record.get("status") in TERMINAL_RUN_STATUSES:
            print(f"resume: skip {run.name} ({record['status']})")
            continue
        print(f"plan: {run.name}: {format_command(command)}")
        run_dir = campaign_dir / "workspaces" / run.name
        artifact_dir = campaign_dir / "artifacts" / run.name
        source = suite.source_for_run(run)
        procurement = procure_run(suite, run, repo_root, run_dir)
        _record_procurement(record, procurement)
        if not procurement.ok:
            record["status"] = "blocked"
            if run_dir.exists():
                _finish_run_record(
                    record,
                    run_dir,
                    source,
                    artifact_dir,
                    suite.scrub_allow,
                )
            write_metadata(metadata_path, metadata)
            print(f"blocked: {run.name}: {procurement.reason}")
            continue
        if dry_run:
            record["status"] = "dry-run-ready"
            _finish_run_record(
                record,
                run_dir,
                source,
                artifact_dir,
                suite.scrub_allow,
            )
            write_metadata(metadata_path, metadata)
            print(f"dry-run: ready {run.name}")
            continue

        start_epoch = int(time.time())
        record["status"] = "running"
        record["start_epoch"] = start_epoch
        write_metadata(metadata_path, metadata)
        try:
            product = run_product(
                command, run_dir, run_dir / "uat-console.log", start_epoch
            )
        except ProductInterrupted as error:
            product = error.result
            record["status"] = INTERRUPTED_ENVIRONMENT
            record["interruption_reason"] = "harness received KeyboardInterrupt"
            record["end_epoch"] = product.end_epoch
            record["duration_seconds"] = product.end_epoch - product.start_epoch
            record["product_exit"] = product.exit_code
            _finish_run_record(
                record,
                run_dir,
                source,
                artifact_dir,
                suite.scrub_allow,
            )
            metadata.setdefault("resume_notes", []).append(
                f"{run.name} was {INTERRUPTED_ENVIRONMENT} and must not be rerun. "
                "A one-time rerun in a new directory requires review adjudication."
            )
            write_metadata(metadata_path, metadata)
            raise
        except OSError as error:
            record["status"] = "blocked"
            record["end_epoch"] = int(time.time())
            record["protocol_reason"] = f"product start failed: {error}"
            _finish_run_record(
                record,
                run_dir,
                source,
                artifact_dir,
                suite.scrub_allow,
            )
            write_metadata(metadata_path, metadata)
            print(f"blocked: {run.name}: product start failed: {error}")
            continue
        record["status"] = "completed"
        record["end_epoch"] = product.end_epoch
        record["duration_seconds"] = product.end_epoch - product.start_epoch
        record["product_exit"] = product.exit_code
        record["stdout_tail"] = product.stdout_tail
        record["stderr_tail"] = product.stderr_tail
        _finish_run_record(
            record,
            run_dir,
            source,
            artifact_dir,
            suite.scrub_allow,
        )
        write_metadata(metadata_path, metadata)
        print(
            f"completed: {run.name}: product_exit={product.exit_code} "
            f"duration={record['duration_seconds']}s"
        )
    metadata["updated_epoch"] = int(time.time())
    write_metadata(metadata_path, metadata)


def _markdown_cell(value: Any) -> str:
    if value is None:
        return "—"
    return str(value).replace("|", "\\|").replace("\n", "<br>")


def generate_report(campaign_dir: Path, metadata: dict[str, Any]) -> Path:
    try:
        from calibration_corpus import append

        append([campaign_dir])
    except (ImportError, OSError, ValueError):
        pass
    lines = [
        f"# bench report skeleton: {metadata['campaign_id']}",
        "",
        (
            "This skeleton transfers mechanical observations only. A human reviewer must "
            "decide UAT pass/fail, failure class, retry consumption, and settlement."
        ),
        "",
        "## Preflight record",
        "",
        f"- HEAD: `{metadata['preflight'].get('head_sha', 'unknown')}`",
        f"- minimum ancestor: `{metadata['suite'].get('min_head') or 'not specified'}`",
        f"- NODE_ENV: `{metadata['preflight'].get('node_env', {}).get('value')}`",
        f"- deviations: `{len(metadata.get('deviations', []))}`",
        "",
        "## Event search method",
        "",
        (
            "The harness recursively parses JSON lines from each run artifact using file "
            "glob `.anvil/runs/**/events.jsonl`, reads the exact `event` field, and applies "
            "these regular-expression patterns:"
        ),
        "",
    ]
    try:
        from classify_runs import classify_campaign, render

        classification = render(classify_campaign(campaign_dir)).splitlines()
    except (ImportError, OSError, ValueError):
        classification = [
            "分類器を読み込めませんでした。UNKNOWNとして人手確認が必要です。"
        ]
    lines.extend(
        ["", "## Failure class display (non-adjudicating)", ""] + classification
    )
    for label, pattern in EVENT_SEARCH_PATTERNS.items():
        lines.append(f"- `{label}`: `{pattern.pattern}`")
    lines.extend(
        [
            "",
            "## Run matrix (mechanical transfer)",
            "",
            (
                "| run | harness status | product exit | seconds | verdict transfer | "
                "assurance transfer |"
            ),
            "|---|---|---:|---:|---|---|",
        ]
    )
    if metadata["suite"].get("scrub_allow"):
        lines.extend(["", "## Scrub allow-list", ""])
        for item in metadata["suite"]["scrub_allow"]:
            lines.append(f"- `{item['pattern']}` — {item['reason']}")
    for record in metadata["runs"]:
        lines.append(
            "| "
            + " | ".join(
                _markdown_cell(value)
                for value in (
                    record["name"],
                    record.get("status"),
                    record.get("product_exit"),
                    record.get("duration_seconds"),
                    record.get("verdict"),
                    record.get("assurance"),
                )
            )
            + " |"
        )
        if record.get("scrub_failed"):
            lines.append("  - scrub: FAIL (findings are recorded in uat-meta.json)")
    links = [
        f"- `{r['name']}`: `artifacts/{r['name']}/acceptance-sheet.md`"
        for r in metadata["runs"]
        if r.get("sheet_generated")
    ]
    lines.extend(
        ["", "## Acceptance sheets", ""]
        + (links or ["- 記録なし（生成失敗はuat-meta.json参照）"])
    )
    lines.extend(
        [
            "",
            "## Event firing counts",
            "",
            (
                "| run | intent_resolved | host_env_normalized | "
                "fix_reproducer_suggested | *_plan_synthesized | *_adjudicated |"
            ),
            "|---|---:|---:|---:|---:|---:|",
        ]
    )
    for record in metadata["runs"]:
        counts = record.get("event_counts", {})
        lines.append(
            f"| {_markdown_cell(record['name'])} | "
            f"{counts.get('intent_resolved', 0)} | "
            f"{counts.get('host_env_normalized', 0)} | "
            f"{counts.get('fix_reproducer_suggested', 0)} | "
            f"{counts.get('*_plan_synthesized', 0)} | "
            f"{counts.get('*_adjudicated', 0)} |"
        )
    lines.extend(["", "## Terminal reasons (verbatim transfer)", ""])
    for record in metadata["runs"]:
        lines.extend([f"### {record['name']}", ""])
        terminal_reason = record.get("terminal_reason")
        if terminal_reason is None:
            lines.append("No `Stop reason:` line was found.")
        else:
            lines.extend(["````text", terminal_reason, "````"])
        lines.append("")
    interrupted = [
        record["name"]
        for record in metadata["runs"]
        if record.get("status") == INTERRUPTED_ENVIRONMENT
    ]
    if interrupted:
        lines.extend(
            [
                "## Interrupted runs requiring review",
                "",
                "The following runs were not rerun: " + ", ".join(interrupted) + ".",
                (
                    "A one-time rerun must use a new directory and requires review "
                    "adjudication."
                ),
                "",
            ]
        )
    lines.extend(
        [
            "## Human review fields",
            "",
            "- UAT pass/fail: ",
            "- Failure class / attribution: ",
            "- Retry-consumption decision: ",
            "- Settlement comment: ",
            "",
        ]
    )
    report_path = campaign_dir / "report-skeleton.md"
    report_path.write_text("\n".join(lines), encoding="utf-8")
    return report_path


def resolve_suite_path(repo_root: Path, value: str) -> Path:
    candidate = Path(value)
    if candidate.suffix == ".toml" or candidate.parent != Path("."):
        path = candidate if candidate.is_absolute() else repo_root / candidate
    else:
        path = (
            repo_root
            / "workspace"
            / "management"
            / "bench"
            / "suites"
            / f"{value}.toml"
        )
    if not path.is_file():
        raise BenchError(f"suite not found: {path}")
    return path.resolve()


def _campaign_id(suite_id: str, dry_run: bool) -> str:
    marker = "-dry-run" if dry_run else ""
    return f"{suite_id}{marker}-{time.strftime('%Y%m%d-%H%M%S', time.gmtime())}"


def create_campaign(workspace_root: Path, suite_id: str, dry_run: bool) -> Path:
    base_id = _campaign_id(suite_id, dry_run)
    for suffix in [""] + [f"-{index:02d}" for index in range(1, 100)]:
        candidate = workspace_root / f"{base_id}{suffix}"
        try:
            candidate.mkdir(parents=True, exist_ok=False)
        except FileExistsError:
            continue
        return candidate
    raise BenchError(f"could not allocate campaign directory under {workspace_root}")


def find_resume_campaign(workspace_root: Path, suite: SuiteDefinition) -> Path:
    candidates: list[Path] = []
    for candidate in workspace_root.glob(f"{suite.suite_id}-*"):
        metadata_path = candidate / "uat-meta.json"
        if not metadata_path.is_file():
            continue
        try:
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            continue
        if (
            metadata.get("mode") == "run"
            and metadata.get("suite", {}).get("id") == suite.suite_id
        ):
            candidates.append(candidate)
    if not candidates:
        raise BenchError(f"no resumable campaign found for suite {suite.suite_id}")
    return max(candidates, key=lambda path: path.name)


def load_resume_metadata(campaign_dir: Path, suite: SuiteDefinition) -> dict[str, Any]:
    metadata_path = campaign_dir / "uat-meta.json"
    try:
        metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise BenchError(f"cannot read resume metadata: {error}") from error
    if metadata.get("suite", {}).get("sha256") != sha256_file(suite.path):
        raise BenchError("resume refused: suite TOML SHA-256 changed")
    expected_names = [run.name for run in suite.runs]
    actual_names = [record.get("name") for record in metadata.get("runs", [])]
    if actual_names != expected_names:
        raise BenchError("resume refused: run matrix differs from suite")
    return metadata


def _validate_workspace_root(workspace_root: Path, repo_root: Path) -> None:
    try:
        workspace_root.resolve().relative_to(repo_root.resolve())
    except ValueError:
        return
    raise BenchError("--workspace-root must be outside the repository")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="subcommand", required=True)
    run_parser = subparsers.add_parser("run", help="run or validate a bench suite")
    run_parser.add_argument("--suite", required=True, help="suite id or TOML path")
    run_parser.add_argument("--workspace-root", required=True, type=Path)
    run_parser.add_argument("--dry-run", action="store_true")
    run_parser.add_argument("--resume", action="store_true")
    run_parser.add_argument("--min-head")
    run_parser.add_argument("--skip-suite-tests", action="store_true")
    scrub_parser = subparsers.add_parser("scrub", help="scan a report or artifact tree")
    scrub_parser.add_argument("--path", required=True, type=Path)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.subcommand == "scrub":
        result = scrub_path(args.path.expanduser().resolve())
        print(
            json.dumps(
                {"ok": result.ok, "findings": list(result.findings)},
                ensure_ascii=False,
                indent=2,
            )
        )
        return 0 if result.ok else 3
    if args.dry_run and args.resume:
        parser.error("--dry-run and --resume cannot be combined")
    repo_root = repository_root()
    try:
        suite_path = resolve_suite_path(repo_root, args.suite)
        suite = load_suite(suite_path)
        workspace_root = args.workspace_root.expanduser().resolve()
        _validate_workspace_root(workspace_root, repo_root)
        min_head = args.min_head or suite.min_head
        if args.resume:
            campaign_dir = find_resume_campaign(workspace_root, suite)
            preflight, deviations = perform_preflight(
                repo_root, min_head, args.skip_suite_tests, campaign_dir
            )
            metadata = load_resume_metadata(campaign_dir, suite)
            metadata["preflight"] = preflight
            metadata["deviations"] = deviations
            metadata.setdefault("resume_epochs", []).append(int(time.time()))
        else:
            campaign_dir = create_campaign(workspace_root, suite.suite_id, args.dry_run)
            preflight, deviations = perform_preflight(
                repo_root, min_head, args.skip_suite_tests, campaign_dir
            )
            metadata = new_metadata(
                suite,
                campaign_dir.name,
                "dry-run" if args.dry_run else "run",
                repo_root,
                preflight,
                deviations,
            )
            write_metadata(campaign_dir / "uat-meta.json", metadata)
        print(f"campaign: {campaign_dir}")
        process_runs(
            suite,
            repo_root,
            campaign_dir,
            metadata,
            dry_run=args.dry_run,
            resume=args.resume,
        )
        report_path = generate_report(campaign_dir, metadata)
        print(f"metadata: {campaign_dir / 'uat-meta.json'}")
        print(f"report skeleton: {report_path}")
        if any(record.get("status") == "blocked" for record in metadata["runs"]):
            return 3
        return 0
    except ProductInterrupted:
        print(
            f"bench: {INTERRUPTED_ENVIRONMENT}; the run will not be retried",
            file=sys.stderr,
        )
        return 130
    except BenchError as error:
        print(f"bench: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
