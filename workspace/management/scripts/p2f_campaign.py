#!/usr/bin/env python3
"""Declare and account for the P2F-0 pass-after-fix campaign."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import subprocess
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
    args = parser.parse_args()
    if args.command == "declare":
        declaration = write_declaration(args.recorded_at)
        print(
            f"declared {declaration['population']['census_size']} failed runs; "
            f"selected {declaration['sampling']['actual_size']}; measurement_started=false"
        )
        return 0
    raise AssertionError(args.command)


if __name__ == "__main__":
    raise SystemExit(main())
