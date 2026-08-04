#!/usr/bin/env python3
"""Declare and account for the P2F-0 pass-after-fix campaign."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import re
import shutil
import subprocess
import time
from collections import Counter, defaultdict
from collections.abc import Iterable
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[3]
RUNS_DIR = ROOT / "workspace/management/runs"
OUTPUT_DIR = RUNS_DIR / "p2f-0"
DECLARATION_PATH = OUTPUT_DIR / "predeclaration.json"
CENSUS_PATH = OUTPUT_DIR / "census.md"
RESULT_PATH = OUTPUT_DIR / "measurement-results.json"
SETTLEMENT_PATH = OUTPUT_DIR / "settlement.json"
REPORT_PATH = OUTPUT_DIR / "report.md"
SCHEMA_VERSION = "commandagent.p2f-predeclaration/v0"
SAMPLE_SEED = "p2f-0-stratified-v1"
TARGET_SAMPLE_SIZE = 10
EXECUTION_REVISION = "80df5e39e1a0fb39cf9f1f4d5be6de31395f63eb"
EXECUTION_BINARY = ROOT / "target/release/commandagent"
EXECUTION_BINARY_SHA256 = (
    "5998a1f53dfb74bbff9164fec8bed1f3254aad6046ba6a2691132b5ccf36cccd"
)
EXECUTION_BINARY_VERSION = "commandagent 0.1.0 80df5e39+dirty 2026-08-04T23:00:42+09:00"
EXECUTION_ROOT = Path(
    "/Users/maenokota/share/work/localwork/commandagent_mvp/01/"
    "test0805_p2f_0/p2f-0-20260805-003041"
)
PRIOR_SUCCESS = 1
PRIOR_TRIALS = 3
WILSON_Z = 1.959963984540054
JEFFREYS_ALPHA = 0.5
JEFFREYS_BETA = 0.5


@dataclass(frozen=True)
class Source:
    """One immutable campaign source included in the failed-run census."""

    label: str
    profile: str
    family_default: str
    model: str
    campaign_path: Path
    evidence_path: Path
    kind: str


@dataclass(frozen=True)
class CensusEntry:
    """One formally failed source run and its repair-route eligibility."""

    census_id: str
    source_label: str
    run_name: str
    profile: str
    family: str
    model: str
    workspace: str
    workspace_exists: bool
    source_verdict: str
    failure_class: str
    failure_stratum: str
    score: float | None
    score_band: str
    repair_route: str
    recovery_circle_applicable: bool
    recovery_circle_reason: str
    fix_continuation_applicable: bool
    fix_continuation_reason: str
    recovery_plan: str
    recovery_plan_exists: bool
    recovery_plan_sha256: str | None
    original_duration_seconds: float | None
    original_cost_usd: float | None
    source_meta_sha256: str

    @property
    def stratum(self) -> tuple[str, str]:
        return self.failure_stratum, self.score_band


def _sources() -> list[Source]:
    local = Path("/Users/maenokota/share/work/localwork/commandagent_mvp/01")
    return [
        Source(
            "bon0-001",
            "cli",
            "filter",
            "gpt-5.6-luna",
            local / "test0802_bon0/cli-filter-bon0-20260802-153030",
            RUNS_DIR / "uat-test0802-cli-bon0-001/evidence/bon-selection.json",
            "bon",
        ),
        Source(
            "bon0-002r",
            "cli",
            "filter",
            "gpt-5.6-luna",
            local / "test0803_bon0_002r/cli-filter-bon0-20260803-115414",
            RUNS_DIR / "f-bon-v-001/evidence/bon0-002r-selection.json",
            "bon",
        ),
        Source(
            "bon0-003r",
            "cli",
            "filter",
            "gpt-5.6-luna",
            local / "test0803_bon0_003r/cli-filter-bon0-20260803-134215",
            RUNS_DIR / "f-bon-v-001/evidence/bon0-003r-selection.json",
            "bon",
        ),
        Source(
            "bon0-004r",
            "cli",
            "filter",
            "gpt-5.6-luna",
            local / "test0803_bon0_004r/cli-filter-bon0-20260803-160629",
            RUNS_DIR / "f-bon-v-001/evidence/bon0-004r-selection.json",
            "bon",
        ),
        Source(
            "bon-local-001",
            "nextjs",
            "breakout",
            "qwen3.6:35b-a3b-coding-nvfp4",
            local
            / "test0804_bon_local_001"
            / "nextjs-breakout-local-bon-20260804-112413",
            RUNS_DIR / "f-bon-v-001/evidence/local-breakout-result.json",
            "local",
        ),
        Source(
            "luna-006",
            "cli",
            "mixed",
            "gpt-5.6-luna",
            local / "test0801_cli_luna6/cli-create-luna-20260802-005454",
            RUNS_DIR / "uat-test0801-cli-luna-006/evidence/campaign-summary.json",
            "luna",
        ),
        Source(
            "luna-007",
            "cli",
            "mixed",
            "gpt-5.6-luna",
            local / "test0801_cli_luna7/cli-create-luna-20260802-045807",
            RUNS_DIR / "uat-test0801-cli-luna-007/evidence/campaign-summary.json",
            "luna",
        ),
        Source(
            "luna-008",
            "cli",
            "mixed",
            "gpt-5.6-luna",
            local / "test0801_cli_luna8/cli-create-luna-20260802-082157",
            RUNS_DIR / "uat-test0801-cli-luna-008/evidence/campaign-summary.json",
            "luna",
        ),
    ]


def _read_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    assert isinstance(value, dict), f"JSON root must be an object: {path}"
    return value


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _git(*args: str) -> str:
    result = subprocess.run(
        ["git", *args],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return result.stdout.strip()


def _meta_runs(source: Source) -> tuple[dict[str, Any], dict[str, dict[str, Any]]]:
    path = source.campaign_path / "uat-meta.json"
    meta = _read_json(path)
    runs = meta.get("runs")
    assert isinstance(runs, list), f"runs missing: {path}"
    by_name: dict[str, dict[str, Any]] = {}
    for run in runs:
        assert isinstance(run, dict)
        name = run.get("name")
        assert isinstance(name, str) and name
        by_name[name] = run
    return meta, by_name


def _final_vectors() -> dict[str, dict[str, Any]]:
    path = RUNS_DIR / "f1-retrospective-001/final-vectors.jsonl"
    vectors: dict[str, dict[str, Any]] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        value = json.loads(line)
        assert isinstance(value, dict)
        run_id = value.get("run_id")
        assert isinstance(run_id, str) and run_id
        vectors[run_id] = value
    return vectors


def _recovery_plan(meta: dict[str, Any], workspace: Path) -> Path:
    stderr_tail = meta.get("stderr_tail")
    assert isinstance(stderr_tail, str), f"stderr_tail missing for {workspace}"
    match = re.search(r"Recovery UltraPlan YAML saved: ([^\n ]+)", stderr_tail)
    assert match is not None, f"recovery plan absent for {workspace}"
    return workspace / match.group(1)


def _failure_class(profile: str, terminal: str) -> tuple[str, str]:
    lower = terminal.lower()
    if profile == "nextjs":
        if lower.startswith("contract_instrumentation_missing"):
            return "contract_instrumentation_missing:restart", "nextjs_evidence"
        return "restart_or_recoverable_state_evidence_missing", "nextjs_evidence"
    phase_match = re.match(r"(?:error: )?phase ([^ ]+) failed", lower)
    if phase_match:
        phase = phase_match.group(1) if phase_match else "unknown"
        return f"phase_failure:{phase}", "phase_verification"
    if "cli_probe_polarity_violation" in lower:
        return "cli_probe_polarity_violation", "cli_polarity"
    if "cli_output_claims" in lower:
        return "cli_output_claims:observed_stdout_mismatch", "cli_claim_binding"
    if "profile_behavior_probe_error" in lower:
        return "profile_behavior_probe_error", "profile_probe"
    if "profile behavior evidence" in lower:
        return "profile_behavior_evidence_failed", "profile_probe"
    if "help_binding" in lower:
        return "help_binding_failure", "other_acceptance"
    if "path does not exist" in lower:
        return "final_acceptance_repair:path_missing", "other_acceptance"
    return "other_final_acceptance_failure", "other_acceptance"


def _score_band(score: float | None) -> str:
    if score is None:
        return "unreached"
    if score < 37.5:
        return "low:<37.5"
    if score < 75:
        return "mid:37.5-<75"
    return "high:75-<100"


def _bon_rows(source: Source) -> Iterable[dict[str, Any]]:
    evidence = _read_json(source.evidence_path)
    runs = evidence.get("runs")
    assert isinstance(runs, list)
    for run in runs:
        assert isinstance(run, dict)
        if run.get("earned_full") is True:
            continue
        vector = run.get("score_vector")
        assert isinstance(vector, dict)
        yield {
            "name": run["name"],
            "family": source.family_default,
            "score": vector.get("score"),
            "duration_seconds": run.get("duration_seconds"),
            "cost_usd": run.get("cost_usd"),
            "source_verdict": "non_full",
        }


def _local_rows(source: Source) -> Iterable[dict[str, Any]]:
    evidence = _read_json(source.evidence_path)
    runs = evidence.get("runs")
    assert isinstance(runs, list)
    for run in runs:
        assert isinstance(run, dict)
        if run.get("full") is True:
            continue
        yield {
            "name": run["run"],
            "family": source.family_default,
            "score": None,
            "duration_seconds": run.get("duration_seconds"),
            "cost_usd": None,
            "source_verdict": str(run.get("final_acceptance_status", "non_full")),
            "terminal": str(run.get("terminal_class", "unknown")),
        }


def _luna_rows(
    source: Source, vectors: dict[str, dict[str, Any]]
) -> Iterable[dict[str, Any]]:
    evidence = _read_json(source.evidence_path)
    uat_id = evidence.get("uat_id")
    assert isinstance(uat_id, str) and uat_id
    runs = evidence.get("runs")
    assert isinstance(runs, list)
    for run in runs:
        assert isinstance(run, dict)
        if run.get("verdict") == "complete":
            continue
        name = run.get("name")
        assert isinstance(name, str) and name
        vector = vectors[f"{uat_id}/{name}"]
        yield {
            "name": name,
            "family": run.get("family", source.family_default),
            "score": vector.get("score"),
            "duration_seconds": run.get("duration_seconds"),
            "cost_usd": run.get("cost_usd"),
            "source_verdict": str(run.get("verdict", "failed")),
        }


def build_census() -> list[CensusEntry]:
    """Return the complete P2F-0 failed-run census from immutable evidence."""
    vectors = _final_vectors()
    census: list[CensusEntry] = []
    for source in _sources():
        assert source.campaign_path.is_dir(), source.campaign_path
        assert source.evidence_path.is_file(), source.evidence_path
        _meta, meta_runs = _meta_runs(source)
        if source.kind == "bon":
            rows = _bon_rows(source)
        elif source.kind == "local":
            rows = _local_rows(source)
        else:
            rows = _luna_rows(source, vectors)
        source_meta_path = source.campaign_path / "uat-meta.json"
        for row in rows:
            name = row["name"]
            meta = meta_runs[name]
            workspace = source.campaign_path / "workspaces" / name
            recovery = _recovery_plan(meta, workspace)
            terminal = str(row.get("terminal", meta.get("stderr_tail", "")))
            failure_class, failure_stratum = _failure_class(source.profile, terminal)
            raw_score = row.get("score")
            score = float(raw_score) if raw_score is not None else None
            recovery_exists = recovery.is_file()
            census.append(
                CensusEntry(
                    census_id=f"{source.label}/{name}",
                    source_label=source.label,
                    run_name=name,
                    profile=source.profile,
                    family=str(row["family"]),
                    model=source.model,
                    workspace=str(workspace),
                    workspace_exists=workspace.is_dir(),
                    source_verdict=str(row["source_verdict"]),
                    failure_class=failure_class,
                    failure_stratum=failure_stratum,
                    score=score,
                    score_band=_score_band(score),
                    repair_route="fix_continuation",
                    recovery_circle_applicable=False,
                    recovery_circle_reason=(
                        "fixed recovery-circle workflows are data-profile only; "
                        f"source profile is {source.profile}"
                    ),
                    fix_continuation_applicable=recovery_exists,
                    fix_continuation_reason=(
                        "run-owned recovery UltraPlan exists"
                        if recovery_exists
                        else "run-owned recovery UltraPlan missing"
                    ),
                    recovery_plan=str(recovery),
                    recovery_plan_exists=recovery_exists,
                    recovery_plan_sha256=(
                        _sha256(recovery) if recovery_exists else None
                    ),
                    original_duration_seconds=_optional_float(
                        row.get("duration_seconds")
                    ),
                    original_cost_usd=_optional_float(row.get("cost_usd")),
                    source_meta_sha256=_sha256(source_meta_path),
                )
            )
    return sorted(census, key=lambda entry: entry.census_id)


def _optional_float(value: Any) -> float | None:
    if value is None:
        return None
    assert isinstance(value, int | float)
    return float(value)


def select_sample(
    census: Iterable[CensusEntry], size: int = TARGET_SAMPLE_SIZE
) -> list[CensusEntry]:
    """Apply the declared two-stage deterministic stratified allocation."""
    cells: dict[tuple[str, str], list[CensusEntry]] = defaultdict(list)
    for entry in census:
        cells[entry.stratum].append(entry)
    assert len(cells) <= size, "target must cover every non-empty stratum"

    def rank(entry: CensusEntry) -> tuple[str, str]:
        digest = hashlib.sha256(
            f"{SAMPLE_SEED}\0{entry.census_id}".encode()
        ).hexdigest()
        return digest, entry.census_id

    ranked = {cell: sorted(entries, key=rank) for cell, entries in cells.items()}
    selected_by_cell = {cell: 1 for cell in cells}
    while sum(selected_by_cell.values()) < size:
        eligible = [
            cell for cell in cells if selected_by_cell[cell] < len(ranked[cell])
        ]
        assert eligible, "sample size exceeds census"
        cell = min(
            eligible,
            key=lambda item: (
                -(len(ranked[item]) - selected_by_cell[item]),
                item,
            ),
        )
        selected_by_cell[cell] += 1
    selected = [
        entry
        for cell in sorted(ranked)
        for entry in ranked[cell][: selected_by_cell[cell]]
    ]
    return sorted(selected, key=lambda entry: entry.census_id)


def wilson_interval(
    successes: int, trials: int, z: float = WILSON_Z
) -> tuple[float, float]:
    """Return the two-sided Wilson score interval."""
    assert trials > 0
    p_hat = successes / trials
    denominator = 1 + z * z / trials
    center = (p_hat + z * z / (2 * trials)) / denominator
    half = (
        z
        * math.sqrt(p_hat * (1 - p_hat) / trials + z * z / (4 * trials * trials))
        / denominator
    )
    return center - half, center + half


def _log_beta(a: float, b: float) -> float:
    return math.lgamma(a) + math.lgamma(b) - math.lgamma(a + b)


def beta_binomial_probabilities(trials: int, alpha: float, beta: float) -> list[float]:
    """Return the exact beta-binomial count probability mass."""
    probabilities: list[float] = []
    for successes in range(trials + 1):
        log_probability = (
            math.lgamma(trials + 1)
            - math.lgamma(successes + 1)
            - math.lgamma(trials - successes + 1)
            + _log_beta(successes + alpha, trials - successes + beta)
            - _log_beta(alpha, beta)
        )
        probabilities.append(math.exp(log_probability))
    return probabilities


def equal_tail_count_band(
    probabilities: list[float], mass: float = 0.95
) -> tuple[int, int]:
    """Return a discrete equal-tail predictive interval."""
    tail = (1 - mass) / 2
    cumulative = 0.0
    lower = 0
    for index, probability in enumerate(probabilities):
        cumulative += probability
        if cumulative >= tail:
            lower = index
            break
    cumulative = 0.0
    upper = len(probabilities) - 1
    for index in range(len(probabilities) - 1, -1, -1):
        cumulative += probabilities[index]
        if cumulative >= tail:
            upper = index
            break
    return lower, upper


def _production_tree_pin() -> dict[str, Any]:
    paths = _git("ls-files", "src", "workflows").splitlines()
    digest = hashlib.sha256()
    for relative in paths:
        digest.update(relative.encode())
        digest.update(b"\0")
        digest.update((ROOT / relative).read_bytes())
        digest.update(b"\0")
    return {"file_count": len(paths), "sha256": digest.hexdigest()}


def _band_pins() -> dict[str, str]:
    names = [
        "band_summary.md",
        "band_summary_circle.md",
        "band_summary_cli.md",
        "band_summary_data.md",
        "band_summary_fix.md",
        "band_summary_ingest.md",
        "band_summary_investigation.md",
    ]
    return {name: _sha256(RUNS_DIR / name) for name in names}


def _tree_sha256(path: Path, *, exclude_anvil: bool = False) -> str:
    """Hash relative names, file bytes, and symlink targets in one workspace tree."""
    digest = hashlib.sha256()
    for item in sorted(path.rglob("*"), key=lambda candidate: candidate.as_posix()):
        relative = item.relative_to(path)
        if exclude_anvil and relative.parts and relative.parts[0] == ".anvil":
            continue
        digest.update(relative.as_posix().encode())
        digest.update(b"\0")
        if item.is_symlink():
            digest.update(b"L")
            digest.update(os.readlink(item).encode())
        elif item.is_file():
            digest.update(b"F")
            digest.update(item.read_bytes())
        elif item.is_dir():
            digest.update(b"D")
        else:
            digest.update(b"O")
        digest.update(b"\0")
    return digest.hexdigest()


def _load_dotenv(environment: dict[str, str]) -> None:
    """Load simple KEY=VALUE entries without logging credential values."""
    path = ROOT / ".env"
    assert path.is_file(), f"environment file missing: {path}"
    for raw_line in path.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("export "):
            line = line.removeprefix("export ").lstrip()
        key, separator, value = line.partition("=")
        if not separator or not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", key):
            continue
        value = value.strip()
        if len(value) >= 2 and value[0] == value[-1] and value[0] in {"'", '"'}:
            value = value[1:-1]
        environment.setdefault(key, value)


def _source_for_label(label: str) -> Source:
    matches = [source for source in _sources() if source.label == label]
    assert len(matches) == 1, f"source label mismatch: {label}"
    return matches[0]


def continuation_argv(entry: dict[str, Any], copied_workspace: Path) -> list[str]:
    """Convert the source run argv according to the predeclared action-only rule."""
    source = _source_for_label(str(entry["source_label"]))
    _meta, runs = _meta_runs(source)
    run = runs[str(entry["run_name"])]
    raw_argv = run.get("command_argv")
    assert isinstance(raw_argv, list) and all(
        isinstance(item, str) for item in raw_argv
    )
    argv = list(raw_argv)
    assert argv[0] == "commandagent"
    assert argv[-1] and not argv[-1].startswith("--"), "source goal missing"
    argv.pop()
    retained = [str(EXECUTION_BINARY)]
    index = 1
    while index < len(argv):
        item = argv[index]
        if item == "--intent":
            assert index + 1 < len(argv)
            index += 2
            continue
        if item == "--ultra-plan-run":
            index += 1
            continue
        retained.append(item)
        index += 1
    source_workspace = Path(str(entry["workspace"]))
    recovery_relative = Path(str(entry["recovery_plan"])).relative_to(source_workspace)
    copied_recovery = copied_workspace / recovery_relative
    assert copied_recovery.is_file(), copied_recovery
    retained.extend(["--run-ultra-plan", recovery_relative.as_posix()])
    return retained


def _read_events(path: Path) -> list[dict[str, Any]]:
    events: list[dict[str, Any]] = []
    for line_number, line in enumerate(
        path.read_text(encoding="utf-8").splitlines(), start=1
    ):
        value = json.loads(line)
        assert isinstance(value, dict), f"event is not an object: {path}:{line_number}"
        events.append(value)
    return events


def _cli_score_vector(workspace: Path, events: list[dict[str, Any]]) -> dict[str, Any]:
    keys = [
        "cli_probe",
        "help_binding",
        "cli_output_claims",
        "cli_rerun_consistency",
    ]
    probe_reached = any(
        event.get("event") == "profile_behavior_probe" for event in events
    )
    if not probe_reached:
        return {
            "atoms": [{"key": key, "state": "unobserved"} for key in keys],
            "observed_weight": 0,
            "reached": False,
            "score": None,
            "weight_sum": 4,
            "weighted_state_sum_twice": 0,
        }
    evidence_path = workspace / "evidence/cli-assurance.json"
    evidence = _read_json(evidence_path)
    payload = evidence.get("evidence")
    assert isinstance(payload, dict)
    checks = payload.get("checks")
    assert isinstance(checks, dict)
    state_by_check = {
        "pass": "pass",
        "failed": "violation",
        "claims_absent": "absent",
    }
    contribution = {"pass": 2, "violation": -1, "absent": 0}
    atoms: list[dict[str, str]] = []
    for key in keys:
        raw_state = checks.get(key)
        assert isinstance(raw_state, str) and raw_state in state_by_check
        atoms.append({"key": key, "state": state_by_check[raw_state]})
    weighted = sum(contribution[atom["state"]] for atom in atoms)
    return {
        "atoms": atoms,
        "observed_weight": 4,
        "reached": True,
        "score": weighted / 8 * 100,
        "weight_sum": 4,
        "weighted_state_sum_twice": weighted,
    }


def _after_score_vector(
    profile: str,
    workspace: Path,
    events: list[dict[str, Any]],
    full: bool,
) -> dict[str, Any]:
    if profile == "cli":
        return _cli_score_vector(workspace, events)
    assert profile == "nextjs"
    return {
        "mapping": "verdict_mapping",
        "reached": full,
        "score": 100.0 if full else None,
        "atoms": [],
    }


def _before_score_vector(entry: dict[str, Any]) -> dict[str, Any]:
    source = _source_for_label(str(entry["source_label"]))
    if source.kind == "bon":
        document = _read_json(source.evidence_path)
        row = next(
            item for item in document["runs"] if item["name"] == entry["run_name"]
        )
        return dict(row["score_vector"])
    if source.kind == "luna":
        document = _read_json(source.evidence_path)
        uat_id = str(document["uat_id"])
        vector = _final_vectors()[f"{uat_id}/{entry['run_name']}"]
        return {
            key: value
            for key, value in vector.items()
            if key
            in {
                "atoms",
                "observed_weight",
                "reached",
                "score",
                "weight_sum",
                "weighted_state_sum_twice",
            }
        }
    return {
        "mapping": "verdict_mapping",
        "reached": False,
        "score": None,
        "atoms": [],
    }


def _usage_and_cost(events: list[dict[str, Any]]) -> tuple[dict[str, int], float]:
    input_tokens = 0
    cached_tokens = 0
    output_tokens = 0
    for event in events:
        if event.get("event") != "provider_turn_duration":
            continue
        if event.get("provider") != "openai":
            continue
        prompt = event.get("prompt_eval_count")
        cached = event.get("provider_cached_input_tokens")
        output = event.get("eval_count")
        assert isinstance(prompt, int) and isinstance(cached, int)
        assert isinstance(output, int)
        input_tokens += prompt
        cached_tokens += cached
        output_tokens += output
    assert cached_tokens <= input_tokens
    cost = (
        (input_tokens - cached_tokens) * 1.0 + cached_tokens * 0.1 + output_tokens * 6.0
    ) / 1_000_000
    return {
        "input_tokens": input_tokens,
        "cached_input_tokens": cached_tokens,
        "output_tokens": output_tokens,
    }, cost


def _verify_execution_preflight(declaration: dict[str, Any]) -> None:
    assert declaration.get("schema_version") == SCHEMA_VERSION
    assert declaration.get("measurement_started") is False
    identity = declaration["identity"]
    assert _git("cat-file", "-t", str(identity["execution_revision"])) == "commit"
    assert _sha256(EXECUTION_BINARY) == identity["binary_sha256"]
    assert _production_tree_pin() == identity["production_path_tree"]
    assert _band_pins() == identity["band_byte_pins"]
    scope = declaration["scope"]
    assert Path(str(scope["execution_root"])) == EXECUTION_ROOT
    assert not EXECUTION_ROOT.exists(), (
        f"execution destination exists: {EXECUTION_ROOT}"
    )
    for entry in declaration["sampling"]["selected_entries"]:
        workspace = Path(str(entry["workspace"]))
        recovery = Path(str(entry["recovery_plan"]))
        assert workspace.is_dir(), workspace
        assert recovery.is_file(), recovery
        assert _sha256(recovery) == entry["recovery_plan_sha256"]


def _iso_now() -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%S%z")


def _run_one(
    ordinal: int,
    entry: dict[str, Any],
    environment: dict[str, str],
) -> dict[str, Any]:
    safe_id = str(entry["census_id"]).replace("/", "__")
    destination = EXECUTION_ROOT / "workspaces" / f"{ordinal:02d}__{safe_id}"
    source_workspace = Path(str(entry["workspace"]))
    source_tree_before = _tree_sha256(source_workspace)
    source_product_before = _tree_sha256(source_workspace, exclude_anvil=True)
    shutil.copytree(source_workspace, destination, symlinks=True)
    copy_product_before = _tree_sha256(destination, exclude_anvil=True)
    assert copy_product_before == source_product_before
    prior_runs = {
        item.name for item in (destination / ".anvil/runs").iterdir() if item.is_dir()
    }
    argv = continuation_argv(entry, destination)
    log_dir = EXECUTION_ROOT / "logs"
    log_dir.mkdir(parents=True, exist_ok=True)
    stdout_path = log_dir / f"{ordinal:02d}__{safe_id}.stdout.log"
    stderr_path = log_dir / f"{ordinal:02d}__{safe_id}.stderr.log"
    started_at = _iso_now()
    started_monotonic = time.monotonic()
    print(f"P2F start {ordinal:02d}/10 {entry['census_id']}", flush=True)
    with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
        process = subprocess.Popen(
            argv,
            cwd=destination,
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=stdout,
            stderr=stderr,
        )
        while process.poll() is None:
            time.sleep(30)
            elapsed = int(time.monotonic() - started_monotonic)
            print(
                f"P2F heartbeat {ordinal:02d}/10 elapsed={elapsed}s",
                flush=True,
            )
        exit_code = process.returncode
    duration_seconds = round(time.monotonic() - started_monotonic, 3)
    finished_at = _iso_now()
    source_tree_after = _tree_sha256(source_workspace)
    assert source_tree_after == source_tree_before, (
        f"source workspace mutated: {entry['census_id']}"
    )
    current_runs = {
        item.name for item in (destination / ".anvil/runs").iterdir() if item.is_dir()
    }
    new_runs = sorted(current_runs - prior_runs)
    assert len(new_runs) == 1, (
        f"expected one new run for {entry['census_id']}: {new_runs}"
    )
    events_path = destination / ".anvil/runs" / new_runs[0] / "events.jsonl"
    events = _read_events(events_path)
    stops = [event for event in events if event.get("event") == "run_stop"]
    assert len(stops) == 1, f"run_stop count mismatch: {events_path}"
    stop = stops[0]
    full = (
        stop.get("ok") is True
        and stop.get("final_acceptance_status") == "full_success"
        and stop.get("assurance_level") == "full"
    )
    before_vector = _before_score_vector(entry)
    after_vector = _after_score_vector(str(entry["profile"]), destination, events, full)
    before_score = before_vector.get("score")
    after_score = after_vector.get("score")
    usage, cost = _usage_and_cost(events)
    product_after = _tree_sha256(destination, exclude_anvil=True)
    result = {
        "ordinal": ordinal,
        "census_id": entry["census_id"],
        "profile": entry["profile"],
        "family": entry["family"],
        "model": entry["model"],
        "failure_class": entry["failure_class"],
        "failure_stratum": entry["failure_stratum"],
        "starting_score_band": entry["score_band"],
        "repair_route": "fix_continuation",
        "repair_cycles": 1,
        "directive": None,
        "source_workspace": str(source_workspace),
        "execution_workspace": str(destination),
        "source_workspace_tree_sha256_before": source_tree_before,
        "source_workspace_tree_sha256_after": source_tree_after,
        "product_tree_sha256_before": copy_product_before,
        "product_tree_sha256_after": product_after,
        "product_changed": copy_product_before != product_after,
        "recovery_plan_sha256": entry["recovery_plan_sha256"],
        "command_argv": argv,
        "started_at": started_at,
        "finished_at": finished_at,
        "duration_seconds": duration_seconds,
        "exit_code": exit_code,
        "run_id": new_runs[0],
        "events_sha256": _sha256(events_path),
        "stdout_sha256": _sha256(stdout_path),
        "stderr_sha256": _sha256(stderr_path),
        "verdict": "full" if full else "failed",
        "full": full,
        "run_stop": {
            "ok": stop.get("ok"),
            "status": stop.get("status"),
            "final_acceptance_status": stop.get("final_acceptance_status"),
            "assurance_level": stop.get("assurance_level"),
            "failure_kind": stop.get("failure_kind"),
            "stop_reason": stop.get("stop_reason"),
        },
        "score_vector_before": before_vector,
        "score_vector_after": after_vector,
        "score_change": {
            "before": before_score,
            "after": after_score,
            "delta": (
                float(after_score) - float(before_score)
                if isinstance(before_score, int | float)
                and isinstance(after_score, int | float)
                else None
            ),
        },
        "usage": usage,
        "cost_usd": cost,
        "original_run_duration_seconds": entry["original_duration_seconds"],
        "original_run_cost_usd": entry["original_cost_usd"],
    }
    print(
        f"P2F finish {ordinal:02d}/10 verdict={result['verdict']} "
        f"score={before_score}->{after_score} seconds={duration_seconds:.1f} "
        f"cost=${cost:.6f}",
        flush=True,
    )
    return result


def run_measurement() -> dict[str, Any]:
    """Execute the immutable one-continuation sample and write sanitized accounting."""
    declaration = _read_json(DECLARATION_PATH)
    _verify_execution_preflight(declaration)
    environment = dict(os.environ)
    _load_dotenv(environment)
    assert environment.get("OPENAI_API_KEY"), "OPENAI_API_KEY is not configured"
    EXECUTION_ROOT.mkdir(parents=True)
    declaration_sha = _sha256(DECLARATION_PATH)
    results: list[dict[str, Any]] = []
    external_manifest = EXECUTION_ROOT / "measurement-manifest.json"
    for ordinal, entry in enumerate(
        declaration["sampling"]["selected_entries"], start=1
    ):
        results.append(_run_one(ordinal, entry, environment))
        external_manifest.write_text(
            json.dumps(
                {
                    "schema_version": "commandagent.p2f-execution-manifest/v0",
                    "campaign_id": "p2f-0",
                    "declaration_sha256": declaration_sha,
                    "completed": len(results),
                    "runs": results,
                },
                ensure_ascii=False,
                indent=2,
            )
            + "\n",
            encoding="utf-8",
        )
    result = {
        "schema_version": "commandagent.p2f-measurement/v0",
        "campaign_id": "p2f-0",
        "status": "complete",
        "declaration_path": str(DECLARATION_PATH.relative_to(ROOT)),
        "declaration_sha256": declaration_sha,
        "declaration_recorded_before_measurement": True,
        "execution_root": str(EXECUTION_ROOT),
        "binary_sha256": EXECUTION_BINARY_SHA256,
        "sample_size": len(results),
        "full_count": sum(bool(item["full"]) for item in results),
        "duration_seconds_total": sum(
            float(item["duration_seconds"]) for item in results
        ),
        "cost_usd_total": sum(float(item["cost_usd"]) for item in results),
        "runs": results,
    }
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    RESULT_PATH.write_text(
        json.dumps(result, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    return result


def _group_observations(rows: list[dict[str, Any]], field: str) -> list[dict[str, Any]]:
    groups: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in rows:
        groups[str(row[field])].append(row)
    summaries: list[dict[str, Any]] = []
    for label in sorted(groups):
        items = groups[label]
        after_reached = sum(
            isinstance(item["score_change"]["after"], int | float) for item in items
        )
        summaries.append(
            {
                "label": label,
                "n": len(items),
                "full": sum(bool(item["full"]) for item in items),
                "rate": sum(bool(item["full"]) for item in items) / len(items),
                "after_score_reached": after_reached,
                "duration_seconds_total": sum(
                    float(item["duration_seconds"]) for item in items
                ),
                "cost_usd_total": sum(float(item["cost_usd"]) for item in items),
            }
        )
    return summaries


def _configuration_accounting(
    observations: list[Any],
    *,
    configuration: str,
    families: set[str],
) -> dict[str, Any]:
    matched = [
        item
        for item in observations
        if item.profile == "cli"
        and item.model == "gpt-5.6-luna"
        and item.configuration == configuration
        and item.family in families
    ]
    instances: dict[str, tuple[float, float, bool]] = {}
    for item in matched:
        assert item.instance_seconds is not None
        assert item.instance_cost_usd is not None
        value = (
            float(item.instance_seconds),
            float(item.instance_cost_usd),
            bool(item.instance_success),
        )
        previous = instances.setdefault(item.instance_id, value)
        assert previous == value
    full = sum(bool(item.full) for item in matched)
    total_seconds = sum(value[0] for value in instances.values())
    total_cost = sum(value[1] for value in instances.values())
    return {
        "configuration": configuration,
        "trials": len(matched),
        "instances": len(instances),
        "successful_instances": sum(value[2] for value in instances.values()),
        "full": full,
        "time_seconds_total": total_seconds,
        "cost_usd_total": total_cost,
        "time_seconds_per_full": total_seconds / full if full else math.inf,
        "cost_usd_per_full": total_cost / full if full else math.inf,
    }


def _exchange_accounting(rows: list[dict[str, Any]]) -> dict[str, Any]:
    import band_aggregate as band
    import score_time_map as score_map

    observations, _sources_used = score_map.collect_observations(band)
    bon = _configuration_accounting(
        observations, configuration="bon:6", families={"filter"}
    )
    single = _configuration_accounting(
        observations, configuration="single", families={"filter", "stats"}
    )
    original_seconds = sum(float(row["original_run_duration_seconds"]) for row in rows)
    original_cost = sum(float(row["original_run_cost_usd"] or 0.0) for row in rows)
    repair_seconds = sum(float(row["duration_seconds"]) for row in rows)
    repair_cost = sum(float(row["cost_usd"]) for row in rows)
    full = sum(bool(row["full"]) for row in rows)
    fix = {
        "configuration": "single+fix",
        "trials": len(rows),
        "instances": len(rows),
        "full": full,
        "original_time_seconds_total": original_seconds,
        "repair_time_seconds_total": repair_seconds,
        "time_seconds_total": original_seconds + repair_seconds,
        "original_cost_usd_total": original_cost,
        "repair_cost_usd_total": repair_cost,
        "cost_usd_total": original_cost + repair_cost,
        "time_seconds_per_full": (
            (original_seconds + repair_seconds) / full if full else math.inf
        ),
        "cost_usd_per_full": (
            (original_cost + repair_cost) / full if full else math.inf
        ),
        "local_source_cost_policy": (
            "the one local source and repair have USD API cost 0; energy is unmeasured"
        ),
    }
    cli_rows = [row for row in rows if row["profile"] == "cli"]
    cli_full = sum(bool(row["full"]) for row in cli_rows)
    cli_seconds = sum(
        float(row["original_run_duration_seconds"]) + float(row["duration_seconds"])
        for row in cli_rows
    )
    cli_cost = sum(
        float(row["original_run_cost_usd"] or 0.0) + float(row["cost_usd"])
        for row in cli_rows
    )
    return {
        "basis": (
            "descriptive observed procurement accounting; no randomized "
            "contemporaneous equivalence claim"
        ),
        "bon_new_trials": bon,
        "fix_failed_plus_one_continuation": fix,
        "fix_cli_luna_slice": {
            "trials": len(cli_rows),
            "full": cli_full,
            "time_seconds_total": cli_seconds,
            "cost_usd_total": cli_cost,
            "time_seconds_per_full": cli_seconds / cli_full if cli_full else math.inf,
            "cost_usd_per_full": cli_cost / cli_full if cli_full else math.inf,
        },
        "single_reference": single,
    }


def build_settlement(recorded_at: str) -> dict[str, Any]:
    """Recompute P2F@1, strata, exchange accounting, and integrity checks."""
    declaration = _read_json(DECLARATION_PATH)
    result = _read_json(RESULT_PATH)
    rows = result.get("runs")
    assert isinstance(rows, list) and len(rows) == 10
    assert [row["census_id"] for row in rows] == declaration["sampling"]["selected_ids"]
    full = sum(bool(row["full"]) for row in rows)
    assert full == int(result["full_count"])
    observed_wilson = wilson_interval(full, len(rows))
    predictive_band = declaration["prediction"]["predictive_full_count_band_95"]
    assert isinstance(predictive_band, list) and len(predictive_band) == 2
    comparable = [
        row for row in rows if isinstance(row["score_change"]["delta"], int | float)
    ]
    integrity = {
        "sample_order_matches_predeclaration": True,
        "source_workspace_unchanged": sum(
            row["source_workspace_tree_sha256_before"]
            == row["source_workspace_tree_sha256_after"]
            for row in rows
        ),
        "product_changed": sum(bool(row["product_changed"]) for row in rows),
        "one_cycle": sum(int(row["repair_cycles"]) == 1 for row in rows),
        "directive_none": sum(row["directive"] is None for row in rows),
        "production_path_tree_matches_predeclaration": (
            _production_tree_pin() == declaration["identity"]["production_path_tree"]
        ),
        "band_byte_pins_match_predeclaration": (
            _band_pins() == declaration["identity"]["band_byte_pins"]
        ),
    }
    assert integrity == {
        "sample_order_matches_predeclaration": True,
        "source_workspace_unchanged": 10,
        "product_changed": 10,
        "one_cycle": 10,
        "directive_none": 10,
        "production_path_tree_matches_predeclaration": True,
        "band_byte_pins_match_predeclaration": True,
    }
    return {
        "schema_version": "commandagent.p2f-settlement/v0",
        "campaign_id": "p2f-0",
        "recorded_at": recorded_at,
        "status": "closed",
        "sources": {
            "predeclaration": str(DECLARATION_PATH.relative_to(ROOT)),
            "predeclaration_sha256": _sha256(DECLARATION_PATH),
            "measurement": str(RESULT_PATH.relative_to(ROOT)),
            "measurement_sha256": _sha256(RESULT_PATH),
        },
        "overall": {
            "metric": "P2F@1",
            "full": full,
            "trials": len(rows),
            "estimate": full / len(rows),
            "wilson_95": list(observed_wilson),
            "predeclared_beta_binomial_full_count_band_95": predictive_band,
            "within_predeclared_band": predictive_band[0] <= full <= predictive_band[1],
            "repair_duration_seconds_total": result["duration_seconds_total"],
            "repair_cost_usd_total": result["cost_usd_total"],
        },
        "by_failure_stratum": _group_observations(rows, "failure_stratum"),
        "by_starting_score_band": _group_observations(rows, "starting_score_band"),
        "score_change": {
            "comparable_pairs": len(comparable),
            "improved": sum(row["score_change"]["delta"] > 0 for row in comparable),
            "unchanged": sum(row["score_change"]["delta"] == 0 for row in comparable),
            "worsened": sum(row["score_change"]["delta"] < 0 for row in comparable),
            "noncomparable_nullable_pairs": len(rows) - len(comparable),
        },
        "exchange": _exchange_accounting(rows),
        "integrity": integrity,
        "decision_material": {
            "p2f_mechanism_observed": True,
            "automatic_bon_repair_connection": "NO-GO remains",
            "score_monotonicity_observed": False,
            "bon3_score_gate": "not released",
            "reason": (
                "one full arose from the unreached band; all seven numeric start "
                "bands produced zero full, and n is descriptive only"
            ),
        },
    }


def _format_seconds(value: float) -> str:
    return f"{value:,.3f}"


def _format_exchange_value(value: float) -> str:
    return "∞" if math.isinf(value) else f"{value:,.3f}"


def render_report(settlement: dict[str, Any], result: dict[str, Any]) -> str:
    """Render the generated P2F-0 settlement report."""
    overall = settlement["overall"]
    exchange = settlement["exchange"]
    lines = [
        "# P2F-0 settlement",
        "",
        "> GENERATED FILE: DO NOT EDIT.",
        (
            "> Regenerate: `python3 workspace/management/scripts/p2f_campaign.py "
            f"settle --recorded-at {settlement['recorded_at']}`"
        ),
        "",
        "## 事前宣言 → 実測 → 検算",
        "",
        "### 事前宣言",
        "",
        (
            "failed母集団44本をcensusし、失敗クラス×開始スコア帯の9非空セルへ"
            "固定seed SHA順位で配分した10本を、原workspaceのcopy上で保存済み"
            "recovery UltraPlanへ各1周だけ通す。先行は円環1/3、Wilson 95% CI "
            "6.1–79.2%。Jeffreys更新Beta-binomialによる10本のfull本数95%予測帯は"
            "0..9。層別点予測は置かない。BoN修復接続、自動配線、directive注入は0。"
        ),
        "",
        "### 実測",
        "",
        (
            f"P2F@1は **{overall['full']}/{overall['trials']} = {overall['estimate']:.1%}**。"
            f"Wilson 95% CI [{overall['wilson_95'][0]:.1%}, {overall['wilson_95'][1]:.1%}]。"
            f"fix単独総所要 {_format_seconds(overall['repair_duration_seconds_total'])}秒、"
            f"API費用 ${overall['repair_cost_usd_total']:.7f}。唯一のfullは"
            "`bon0-002r/filter_bon0_001`（未到達→100）。"
        ),
        "",
        "| # | census id | failure stratum | start band | verdict | score before → after | fix sec | fix cost |",
        "|---:|---|---|---|---|---|---:|---:|",
    ]
    for row in result["runs"]:
        before = row["score_change"]["before"]
        after = row["score_change"]["after"]
        before_text = "未到達" if before is None else f"{before:g}"
        after_text = "未到達" if after is None else f"{after:g}"
        lines.append(
            f"| {row['ordinal']} | `{row['census_id']}` | `{row['failure_stratum']}` | "
            f"`{row['starting_score_band']}` | {row['verdict']} | {before_text} → {after_text} | "
            f"{row['duration_seconds']:.3f} | ${row['cost_usd']:.7f} |"
        )
    band = overall["predeclared_beta_binomial_full_count_band_95"]
    lines.extend(
        [
            "",
            "### 検算",
            "",
            (
                f"観測full本数{overall['full']}は事前Beta-binomial 95%帯{band[0]}..{band[1]}の"
                "内側。先行33%点との見かけの差を、n=3先行とn=10標本から有意差や"
                "定常率へ昇格しない。原workspace tree SHAは10/10前後一致、copyのproduct "
                "treeは10/10変化、1周10/10、directiveなし10/10。production/workflow集約SHAと"
                "既存7 band byte SHAも事前pin一致。"
            ),
            "",
            (
                "数値比較可能な6本は改善0、横ばい1、悪化5。nullableな4組は差をゼロへ"
                "潰さない。これはfixが変更を作ったことと、受理へ近づいたことが同義でない"
                "ことを示す。"
            ),
            "",
            "## 失敗クラス別（記述）",
            "",
            "| failure stratum | full/n | after score reached | fix seconds | fix cost |",
            "|---|---:|---:|---:|---:|",
        ]
    )
    for row in settlement["by_failure_stratum"]:
        lines.append(
            f"| `{row['label']}` | {row['full']}/{row['n']} | "
            f"{row['after_score_reached']}/{row['n']} | {row['duration_seconds_total']:.3f} | "
            f"${row['cost_usd_total']:.7f} |"
        )
    lines.extend(
        [
            "",
            "## 開始スコア帯別（記述）",
            "",
            "| starting score band | full/n | after score reached | fix seconds | fix cost |",
            "|---|---:|---:|---:|---:|",
        ]
    )
    for row in settlement["by_starting_score_band"]:
        lines.append(
            f"| `{row['label']}` | {row['full']}/{row['n']} | "
            f"{row['after_score_reached']}/{row['n']} | {row['duration_seconds_total']:.3f} | "
            f"${row['cost_usd_total']:.7f} |"
        )
    lines.extend(
        [
            "",
            (
                "唯一のfullは開始未到達帯1/3。low 0/2、mid 0/4、high 0/1で、"
                "開始スコアが高いほどfix成功しやすいという単調性は観測しなかった。"
                "n小の記述であり、相関不存在の主張はしない。BoN-3のscore gate解除材料には"
                "ならない。"
            ),
            "",
            "## 為替レート：full 1件の観測調達単価",
            "",
            "| route | accounting population | full | observed total sec | observed total cost | sec/full | cost/full |",
            "|---|---|---:|---:|---:|---:|---:|",
        ]
    )
    exchange_rows = [
        ("BoN new N", exchange["bon_new_trials"]),
        ("failed + one fix", exchange["fix_failed_plus_one_continuation"]),
        ("single reference", exchange["single_reference"]),
    ]
    for label, row in exchange_rows:
        lines.append(
            f"| {label} | {row['trials']} trials / {row['instances']} instances | "
            f"{row['full']} | {_format_seconds(row['time_seconds_total'])} | "
            f"${row['cost_usd_total']:.7f} | {_format_exchange_value(row['time_seconds_per_full'])} | "
            f"${row['cost_usd_per_full']:.7f} |"
        )
    fix = exchange["fix_failed_plus_one_continuation"]
    cli_fix = exchange["fix_cli_luna_slice"]
    lines.extend(
        [
            "",
            (
                f"fix行は原failed run {_format_seconds(fix['original_time_seconds_total'])}秒/"
                f"${fix['original_cost_usd_total']:.7f}と、fix 1周 "
                f"{_format_seconds(fix['repair_time_seconds_total'])}秒/"
                f"${fix['repair_cost_usd_total']:.7f}を合算。local 1本のUSD API費用は$0、"
                "電力量は未計測。CLI×Lunaだけのfix sliceは"
                f"{cli_fix['full']}/{cli_fix['trials']}、{cli_fix['time_seconds_total']:.3f}秒/"
                f"${cli_fix['cost_usd_total']:.7f} per full。"
            ),
            "",
            (
                "比較は時期・family・抽出条件を揃えた無作為試験ではなく、現有実測の"
                "記述的為替表である。BoNは5窓30新規run、singleはCLI Lunaの既存48単発run、"
                "fixは層別failed 10本を分母とする。"
            ),
            "",
            "## 裁定材料",
            "",
            (
                "既存fix継続が不合格をfullへ拾う機構は1/10で実測した。一方、自動BoN修復"
                "接続のNO-GOは維持する。高開始スコアの優位、修復によるscore単調改善、"
                "低単価優位はいずれもこの標本では支持されない。P2F-1（人間指示版）と"
                "混ぜず、本campaignはCLOSEする。"
            ),
            "",
        ]
    )
    return "\n".join(lines)


def write_settlement(recorded_at: str) -> dict[str, Any]:
    settlement = build_settlement(recorded_at)
    result = _read_json(RESULT_PATH)
    SETTLEMENT_PATH.write_text(
        json.dumps(settlement, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    REPORT_PATH.write_text(render_report(settlement, result), encoding="utf-8")
    return settlement


def build_declaration(recorded_at: str) -> dict[str, Any]:
    """Build the complete machine-readable declaration before any fix run."""
    census = build_census()
    sample = select_sample(census)
    wilson = wilson_interval(PRIOR_SUCCESS, PRIOR_TRIALS)
    alpha = PRIOR_SUCCESS + JEFFREYS_ALPHA
    beta = PRIOR_TRIALS - PRIOR_SUCCESS + JEFFREYS_BETA
    predictive = beta_binomial_probabilities(len(sample), alpha, beta)
    band = equal_tail_count_band(predictive)
    census_counts = Counter(entry.source_label for entry in census)
    score_counts = Counter(entry.score_band for entry in census)
    stratum_counts = Counter(
        f"{entry.failure_stratum} x {entry.score_band}" for entry in census
    )
    assert _git("cat-file", "-t", EXECUTION_REVISION) == "commit"
    assert EXECUTION_BINARY.is_file(), EXECUTION_BINARY
    assert _sha256(EXECUTION_BINARY) == EXECUTION_BINARY_SHA256
    assert len(census) == 44
    assert len(sample) == TARGET_SAMPLE_SIZE
    assert all(entry.workspace_exists for entry in census)
    assert all(entry.fix_continuation_applicable for entry in census)
    return {
        "schema_version": SCHEMA_VERSION,
        "campaign_id": "p2f-0",
        "recorded_at": recorded_at,
        "measurement_started": False,
        "scope": {
            "purpose": "measure pass-after-fix at one existing repair continuation",
            "automatic_bon_repair_connection": False,
            "new_repair_wiring": False,
            "human_directive_injection": False,
            "repair_cycles_per_sample": 1,
            "source_workspace_mutation": False,
            "execution_copy_policy": "copy each sampled failed workspace once",
            "execution_root": str(EXECUTION_ROOT),
        },
        "identity": {
            "execution_revision": EXECUTION_REVISION,
            "binary_path": str(EXECUTION_BINARY),
            "binary_sha256": EXECUTION_BINARY_SHA256,
            "binary_version": EXECUTION_BINARY_VERSION,
            "production_path_tree": _production_tree_pin(),
            "band_byte_pins": _band_pins(),
        },
        "population": {
            "definition": (
                "all formally failed runs from bon0-001, bon0-002r, "
                "bon0-003r, bon0-004r, bon-local-001, and luna-006..008"
            ),
            "census_size": len(census),
            "excluded_full_runs": 4,
            "workspace_existing": sum(entry.workspace_exists for entry in census),
            "fix_continuation_applicable": sum(
                entry.fix_continuation_applicable for entry in census
            ),
            "recovery_circle_applicable": sum(
                entry.recovery_circle_applicable for entry in census
            ),
            "source_counts": dict(sorted(census_counts.items())),
            "score_band_counts": dict(sorted(score_counts.items())),
            "cross_stratum_counts": dict(sorted(stratum_counts.items())),
            "entries": [asdict(entry) for entry in census],
        },
        "sampling": {
            "target_size": TARGET_SAMPLE_SIZE,
            "allowed_size": [8, 12],
            "actual_size": len(sample),
            "seed": SAMPLE_SEED,
            "rule": (
                "allocate one to every non-empty failure-stratum x score-band "
                "cell; allocate remaining slots one at a time to the cell with "
                "the largest unselected count (lexical cell tie-break); within "
                "a cell rank by SHA-256(seed + NUL + census_id), then census_id"
            ),
            "selected_ids": [entry.census_id for entry in sample],
            "selected_entries": [asdict(entry) for entry in sample],
        },
        "prediction": {
            "metric": "P2F@1 full count over the complete stratified sample",
            "prior_evidence": {
                "route": "recovery_circle",
                "successes": PRIOR_SUCCESS,
                "trials": PRIOR_TRIALS,
                "estimate": PRIOR_SUCCESS / PRIOR_TRIALS,
                "wilson_95": list(wilson),
                "note": "only prior measured repair evidence; denominator is three",
            },
            "model": "beta_binomial_with_jeffreys_prior",
            "posterior_alpha": alpha,
            "posterior_beta": beta,
            "trials": len(sample),
            "predictive_mean_full_count": len(sample) * alpha / (alpha + beta),
            "predictive_full_count_band_95": list(band),
            "predictive_count_probabilities": predictive,
            "stratum_point_predictions": None,
            "stratum_prediction_policy": (
                "prohibited: no invented point estimate for n=2..3 strata"
            ),
        },
        "execution_contract": {
            "route": "fix_continuation",
            "action": "--run-ultra-plan <run-owned recovery YAML>",
            "argv_rule": (
                "retain the source run's non-action configuration flags; remove "
                "--intent and its value, --ultra-plan-run, and the trailing goal; "
                "replace them with --run-ultra-plan and the same recovery YAML "
                "path inside the copied workspace"
            ),
            "one_cycle_only": True,
            "directive": None,
            "preflight_fail_closed": [
                "execution_revision commit object is unavailable",
                "binary SHA-256 differs from binary_sha256",
                "source workspace or recovery plan SHA-256 differs",
                "production path or band byte pin differs",
                "execution destination already exists",
            ],
        },
    }


def _format_score(score: float | None) -> str:
    return "not reached" if score is None else f"{score:g}"


def render_census(declaration: dict[str, Any]) -> str:
    """Render the canonical census and pre-registration report."""
    population = declaration["population"]
    sampling = declaration["sampling"]
    prediction = declaration["prediction"]
    prior = prediction["prior_evidence"]
    selected = set(sampling["selected_ids"])
    lines = [
        "# P2F-0 census and pre-registration",
        "",
        "> Generated evidence. Do not edit by hand.",
        "> No repair measurement had started when this declaration was recorded.",
        (
            "> Regenerate with: `python3 workspace/management/scripts/p2f_campaign.py declare --recorded-at "
            f"{declaration['recorded_at']}`"
        ),
        "",
        "## Scope lock",
        "",
        (
            "P2F-0 respects the F-BoN-V NO-GO: it adds no automatic BoN-to-repair "
            "connection and no repair wiring. Each selected failed workspace is copied, "
            "then its own already-saved recovery UltraPlan is invoked exactly once via "
            "`--run-ultra-plan`; the source workspace is not mutated and no directive is "
            "injected."
        ),
        (
            f"Execution copies are preassigned under `{declaration['scope']['execution_root']}`; "
            "an existing destination fails closed."
        ),
        "",
        (
            "Execution is pinned before spend to revision "
            f"`{declaration['identity']['execution_revision']}` and binary SHA-256 "
            f"`{declaration['identity']['binary_sha256']}`. Production-path and all "
            "seven existing band byte hashes are pinned in `predeclaration.json`."
        ),
        "",
        "## Population census",
        "",
        (
            "The population is the complete formally failed inventory from "
            "bon0-001/002r/003r/004r, bon-local-001, and luna-006/007/008. Full runs "
            f"are outside the failed population (4 excluded); failed census n={population['census_size']}. "
            f"Workspace existence is {population['workspace_existing']}/{population['census_size']}; "
            f"saved fix-continuation eligibility is {population['fix_continuation_applicable']}/{population['census_size']}. "
            "The fixed recovery-circle YAMLs are data-profile workflows, so none are "
            "applicable to this CLI/Next.js census."
        ),
        "",
        "| selected | census id | profile/family | workspace | failure class | failure stratum | score | score band | route |",
        "|---|---|---|---|---|---|---:|---|---|",
    ]
    for raw in population["entries"]:
        mark = "yes" if raw["census_id"] in selected else "no"
        workspace = "exists" if raw["workspace_exists"] else "missing"
        lines.append(
            f"| {mark} | `{raw['census_id']}` | {raw['profile']}/{raw['family']} | "
            f"{workspace} | `{raw['failure_class']}` | `{raw['failure_stratum']}` | "
            f"{_format_score(raw['score'])} | `{raw['score_band']}` | "
            "fix continuation |"
        )
    lines.extend(
        [
            "",
            (
                "Every row's absolute source workspace, recovery-plan path and SHA-256, "
                "source-meta SHA-256, original time/cost, and route reason are retained "
                "in `predeclaration.json`."
            ),
            "",
            "## Stratified sample pre-registration",
            "",
            (
                f"Fixed seed: `{sampling['seed']}`. Rule: {sampling['rule']}. This "
                f"produces n={sampling['actual_size']}, within the declared 10 +/- 2 range. "
                "The rule was committed before any copied workspace or repair run existed."
            ),
            "",
            "| order | census id | failure stratum | starting score band | recovery plan SHA-256 |",
            "|---:|---|---|---|---|",
        ]
    )
    for index, raw in enumerate(sampling["selected_entries"], start=1):
        lines.append(
            f"| {index} | `{raw['census_id']}` | `{raw['failure_stratum']}` | "
            f"`{raw['score_band']}` | `{raw['recovery_plan_sha256']}` |"
        )
    wilson = prior["wilson_95"]
    band = prediction["predictive_full_count_band_95"]
    lines.extend(
        [
            "",
            "## P2F@1 pre-declaration",
            "",
            (
                f"The only prior measured repair rate is recovery-circle {prior['successes']}/{prior['trials']} "
                f"= {prior['estimate']:.1%}; Wilson 95% CI [{wilson[0]:.1%}, {wilson[1]:.1%}]. "
                "Its denominator is three, so the uncertainty must remain broad."
            ),
            "",
            (
                "Using Jeffreys Beta(0.5, 0.5) updated by 1/3 gives posterior "
                f"Beta({prediction['posterior_alpha']:g}, {prediction['posterior_beta']:g}). "
                f"For the pre-registered n={prediction['trials']} sample, the exact "
                f"Beta-binomial equal-tail 95% predictive full-count band is {band[0]}..{band[1]} "
                f"(predictive mean {prediction['predictive_mean_full_count']:.2f}, an accounting "
                "reference rather than a stratum forecast). No failure-class or score-band "
                "point prediction is declared; those strata have no adequate denominator."
            ),
            "",
            "## Measurement gate",
            "",
            (
                "Before spend, the runner must fail closed on revision/binary, source "
                "workspace, recovery-plan SHA-256, production-path bytes, band bytes, or "
                "destination freshness. Observation and band verification will be written "
                "only after this declaration commit."
            ),
            "",
        ]
    )
    return "\n".join(lines)


def write_declaration(recorded_at: str) -> dict[str, Any]:
    """Write the declaration JSON and generated census Markdown."""
    declaration = build_declaration(recorded_at)
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    DECLARATION_PATH.write_text(
        json.dumps(declaration, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    CENSUS_PATH.write_text(render_census(declaration), encoding="utf-8")
    return declaration


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    declare = subparsers.add_parser("declare")
    declare.add_argument("--recorded-at", required=True)
    subparsers.add_parser("run")
    settle = subparsers.add_parser("settle")
    settle.add_argument("--recorded-at", required=True)
    args = parser.parse_args()
    if args.command == "declare":
        declaration = write_declaration(args.recorded_at)
        print(
            f"declared {declaration['population']['census_size']} failed runs; "
            f"selected {declaration['sampling']['actual_size']}; measurement_started=false"
        )
        return 0
    if args.command == "run":
        result = run_measurement()
        print(
            f"measured {result['sample_size']} repair continuations; "
            f"full={result['full_count']}; cost=${result['cost_usd_total']:.6f}"
        )
        return 0
    if args.command == "settle":
        settlement = write_settlement(args.recorded_at)
        overall = settlement["overall"]
        print(
            f"settled P2F@1={overall['full']}/{overall['trials']}; "
            f"within_band={overall['within_predeclared_band']}"
        )
        return 0
    raise AssertionError(args.command)


if __name__ == "__main__":
    raise SystemExit(main())
