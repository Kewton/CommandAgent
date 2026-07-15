#!/usr/bin/env python3
"""Aggregate local-tier nextjs/create and data/create UAT capability bands.

Next.js retains its original aggregate.json/report input path and output bytes.
Data uses repository-managed uat-meta.json files plus a frozen index for the
pre-uat-meta campaigns. Generated summaries are written below
workspace/management/runs/ and printed to stdout.
"""

from __future__ import annotations

import argparse
import json
import re
import statistics
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[3]
RUNS_DIR = ROOT / "workspace" / "management" / "runs"
OUTPUT = RUNS_DIR / "band_summary.md"
DATA_OUTPUT = RUNS_DIR / "band_summary_data.md"
WINDOW_START = "uat-test0711-bs-003"
DATA_STABLE_WINDOW_START = "uat-test0715-data-007"
DATA_PLANNER = "qwen3.6:27b-coding-nvfp4"
DATA_FIXTURE_SHA256 = "2f6c04e42b0ebdff85a7eb6b52a342610155be6796bd89e5729075d87c78d873"

FINAL_STATES = ("full_success", "partial", "incomplete", "failed")
PROVISIONAL = {"Quiz": 85, "Breakout": 30, "Space": 7}


@dataclass
class RunRecord:
    set_id: str
    run_id: str
    scenario: str
    planner: str
    executor: str
    plan_preset: str
    final_state: str
    stop_class: str
    elapsed_seconds: int | None
    source: str
    excluded_reason: str = ""
    false_full_reason: str = ""
    prompt: str = ""


@dataclass(frozen=True)
class DataRunRecord:
    set_id: str
    record_dir: str
    run_name: str
    planner: str
    executor: str
    preset: str
    final_acceptance: str
    assurance: str
    failure_class: str
    duration_seconds: int | None
    source: str
    excluded_reason: str = ""
    evidence_dir: Path | None = None

    @property
    def is_full(self) -> bool:
        return self.final_acceptance == "full_success" and self.assurance == "full"


# UAT #1 and the M-4 rounds predate repository-managed campaign-level
# uat-meta.json. This frozen row index preserves their immutable run metadata;
# later campaigns are always discovered from uat-meta.json. The 24 entries are
# cross-referenced by the repository investigations, ledger, and M-4 report.
# They must not be edited to tune a band result.
# Fields: set|run|executor|preset|final|assurance|class|seconds|exclusion|source
DATA_ARCHIVED_RUN_INDEX = """\
uat-test0713-data-001|data_agg_qwen27_plan_qwen35_exec_preset_profile_001|qwen3.6:35b-a3b-coding-nvfp4|profile|not_checked|failed|verify_repair_progress_unchanged|139||uat-test0713-data-001/investigation-01.md
uat-test0713-data-001|data_agg_qwen27_plan_gemma31_exec_preset_profile_001|gemma4:31b-cloud|profile|not_checked|static|verify_repair_progress_unchanged|386||uat-test0713-data-001/investigation-01.md
uat-test0713-data-001|data_agg_qwen27_plan_qwen35_exec_preset_none_001|qwen3.6:35b-a3b-coding-nvfp4|none|not_checked|static|artifact_follow_through_exhausted|556||uat-test0713-data-001/investigation-01.md
uat-test0713-data-001|data_agg_qwen27_plan_gemma31_exec_preset_none_001|gemma4:31b-cloud|none|not_checked|static|dependency_setup_authority_required|473||uat-test0713-data-001/investigation-01.md
uat-test0714-m4-001|data_agg_qwen27_plan_qwen35_exec_preset_profile_001|qwen3.6:35b-a3b-coding-nvfp4|profile|not_checked|failed|pipeline_exit_nonzero_then_model_stagnation_read_only|362||uat-test0714-m4-001/investigation-b2d.md
uat-test0714-m4-001|data_agg_qwen27_plan_gemma31_exec_preset_profile_001|gemma4:31b-cloud|profile|not_checked|failed|model_stagnation_read_only_write_required_inspection|142||uat-test0714-m4-001/investigation-b2d.md
uat-test0714-m4-001|data_agg_qwen27_plan_qwen35_exec_preset_none_001|qwen3.6:35b-a3b-coding-nvfp4|none|not_checked|static|planner_shell_control_syntax_after_corrective_retries|801||uat-test0714-m4-001/investigation-b2d.md
uat-test0714-m4-001|data_agg_qwen27_plan_gemma31_exec_preset_none_001|gemma4:31b-cloud|none|not_checked|failed|planner_shell_control_syntax_after_corrective_retries|556||uat-test0714-m4-001/investigation-b2d.md
uat-test0714-m4-001|data_agg_qwen27_plan_qwen35_exec_preset_profile_002|qwen3.6:35b-a3b-coding-nvfp4|profile|not_checked|failed|workspace_policy_blocked_hidden_anvil_path|161||uat-test0714-m4-001/investigation-b2d.md
uat-test0714-m4-002|data_agg_qwen27_plan_qwen35_exec_preset_profile_001|qwen35|profile|not_checked|failed|executor_model_identifier_not_found_404|141|operator_model_substitution_error|archived:test0714_m4_002/aggregate.json
uat-test0714-m4-002|data_agg_qwen27_plan_gemma31_exec_preset_profile_001|gemma31|profile|not_checked|failed|executor_model_identifier_not_found_404|180|operator_model_substitution_error|archived:test0714_m4_002/aggregate.json
uat-test0714-m4-002|data_agg_qwen27_plan_qwen35_exec_preset_none_001|qwen35|none|not_checked|failed|planner_shell_control_syntax_after_corrective_retries|612|operator_model_substitution_error|archived:test0714_m4_002/aggregate.json
uat-test0714-m4-002|data_agg_qwen27_plan_gemma31_exec_preset_none_001|gemma31|none|not_checked|failed|executor_model_identifier_not_found_404|275|operator_model_substitution_error|archived:test0714_m4_002/aggregate.json
uat-test0714-m4-002|data_agg_qwen27_plan_qwen35_exec_preset_profile_002|qwen35|profile|not_checked|failed|executor_model_identifier_not_found_404|130|operator_model_substitution_error|archived:test0714_m4_002/aggregate.json
uat-test0714-m4-003|data_agg_qwen27_plan_qwen35_exec_preset_profile_001|qwen3.6:35b-a3b-coding-nvfp4|profile|not_checked|failed|claims_binding_failure_then_read_only_stagnation|660||archived:test0714_m4_003/aggregate.json
uat-test0714-m4-003|data_agg_qwen27_plan_gemma31_exec_preset_profile_001|gemma4:31b-cloud|profile|not_checked|failed|script_absent_then_inspection_read_only_stagnation|240||archived:test0714_m4_003/aggregate.json
uat-test0714-m4-003|data_agg_qwen27_plan_qwen35_exec_preset_none_001|qwen3.6:35b-a3b-coding-nvfp4|none|not_checked|static|lint_recovered_then_results_missing_read_only_stagnation|831||archived:test0714_m4_003/aggregate.json
uat-test0714-m4-003|data_agg_qwen27_plan_gemma31_exec_preset_none_001|gemma4:31b-cloud|none|not_checked|static|lint_recovered_then_artifact_follow_through_exhausted|442||archived:test0714_m4_003/aggregate.json
uat-test0714-m4-003|data_agg_qwen27_plan_qwen35_exec_preset_profile_002|qwen3.6:35b-a3b-coding-nvfp4|profile|not_checked|failed|claims_binding_failure_then_inspection_read_only_stagnation|418||archived:test0714_m4_003/aggregate.json
uat-test0714-m4-004|data_agg_qwen27_plan_qwen35_exec_preset_profile_001|qwen3.6:35b-a3b-coding-nvfp4|profile|not_checked|failed|model_stagnation_read_only_write_required_inspection|355|preflight_not_green_and_campaign_incomplete|uat-test0714-m4-004/uat-report.md
uat-test0714-m4-004|data_agg_qwen27_plan_gemma31_exec_preset_profile_001|gemma4:31b|profile|not_checked|failed|campaign_interrupted|166|preflight_not_green_and_campaign_incomplete|uat-test0714-m4-004/uat-report.md
uat-test0714-m4-004|data_agg_qwen27_plan_qwen35_exec_preset_none_001|qwen3.6:35b-a3b-coding-nvfp4|none|not_checked|failed|campaign_interrupted||preflight_not_green_and_campaign_incomplete|uat-test0714-m4-004/uat-report.md
uat-test0714-m4-004|data_agg_qwen27_plan_gemma31_exec_preset_none_001|gemma4:31b|none|not_checked|failed|campaign_interrupted||preflight_not_green_and_campaign_incomplete|uat-test0714-m4-004/uat-report.md
uat-test0714-m4-004|data_agg_qwen27_plan_qwen35_exec_preset_profile_002|qwen3.6:35b-a3b-coding-nvfp4|profile|not_checked|failed|campaign_interrupted||preflight_not_green_and_campaign_incomplete|uat-test0714-m4-004/uat-report.md
"""



def normalize_scenario(*parts: Any) -> str:
    text = " ".join(str(p or "") for p in parts).lower()
    if "space" in text or "invader" in text or "インベーダ" in text:
        return "Space"
    if "breakout" in text or "ブロック" in text:
        return "Breakout"
    if "quiz" in text or "クイズ" in text:
        return "Quiz"
    return "unknown"


def normalize_final(raw: Any, status: Any = "", release_gate: Any = "") -> str:
    text = str(raw or "").strip().lower()
    status_text = str(status or "").strip().lower()
    gate_text = str(release_gate or "").strip().lower()
    if text in {"full_success", "full", "completed"}:
        return "full_success"
    if text == "partial" or status_text == "partial":
        return "partial"
    if text == "incomplete":
        return "incomplete"
    if text in {"failed", "failure"} or status_text == "failed":
        return "failed"
    if text in {"not_checked", "not_applicable", ""}:
        if gate_text == "failed":
            return "incomplete"
        return "failed"
    return "failed"


def classify_stop(row: dict[str, Any], stop_reason: str, final_state: str) -> str:
    explicit = row.get("class") or row.get("failure_class")
    if explicit:
        return str(explicit)
    if final_state == "full_success":
        return "full"
    text = stop_reason.lower()
    if "probe_infrastructure_failed" in text or "probe infrastructure" in text:
        return "probe_infrastructure_failed"
    if "path_confinement" in text:
        return "path_confinement"
    if "no_progress" in text:
        return "no_progress"
    if "read_only_loop" in text or "write_required exhausted" in text:
        return "read_only_loop"
    if "restart_or_recoverable_state" in text or "restart" in text:
        return "restart_evidence"
    if "input_state_change" in text:
        return "input_state_change"
    if "compile" in text or "type error" in text:
        return "compile"
    if "dependency" in text:
        return "dependency"
    if "panic" in text or "char boundary" in text:
        return "panic"
    return final_state


def should_exclude(stop_class: str, stop_reason: str) -> str:
    text = f"{stop_class} {stop_reason}".lower()
    patterns = [
        "probe_infrastructure_failed",
        "probe infrastructure",
        "probe_preflight_failed",
        "playwright unavailable",
        "managed_interaction_probe unavailable",
    ]
    if any(p in text for p in patterns):
        return "probe_infrastructure_failed"
    return ""


def nested_value(row: dict[str, Any], key: str, nested_key: str) -> str:
    value = row.get(key)
    if isinstance(value, dict):
        return str(value.get(nested_key) or "")
    return ""


def artifact_dirs(set_dir: Path, row: dict[str, Any], run_id: str) -> list[Path]:
    candidates: list[Path] = []
    artifacts = row.get("artifacts")
    if isinstance(artifacts, list):
        for item in artifacts:
            path = set_dir / str(item)
            if path.exists():
                candidates.append(path.parent)
    direct = set_dir / "artifacts" / run_id
    if direct.exists():
        candidates.append(direct)
    # Some reports flatten or rename attempts.
    for path in (set_dir / "artifacts").glob(f"*{run_id}*"):
        if path.is_dir():
            candidates.append(path)
    seen: set[Path] = set()
    unique: list[Path] = []
    for path in candidates:
        if path not in seen:
            seen.add(path)
            unique.append(path)
    return unique


def json_file_has_interaction_pass(path: Path) -> bool:
    try:
        data = json.loads(path.read_text())
    except Exception:
        return False
    if not isinstance(data, dict):
        return False
    ok = data.get("ok") is True
    success = data.get("interaction_success") is True
    performed = data.get("interaction_performed") is True
    failure = str(data.get("failure_category") or "")
    return ok and performed and (success or failure == "")


def has_interaction_pass(set_dir: Path, row: dict[str, Any], run_id: str) -> bool:
    if row.get("full_has_interaction_pass") is True:
        return True
    for art_dir in artifact_dirs(set_dir, row, run_id):
        for path in art_dir.glob("*browser-interaction.json"):
            if json_file_has_interaction_pass(path):
                return True
        for path in art_dir.glob("summary.md"):
            text = path.read_text(errors="ignore").lower()
            if "interaction evidence: passed" in text:
                return True
        for path in art_dir.glob("events.jsonl"):
            text = path.read_text(errors="ignore").lower()
            if '"interaction_evidence_status":"passed"' in text:
                return True
    run_dir = row.get("run_dir")
    if run_dir:
        evidence_dir = Path(str(run_dir)).parents[1] / "evidence"
        path = evidence_dir / "browser-interaction.json"
        if path.exists() and json_file_has_interaction_pass(path):
            return True
    return False


def elapsed_from_summary(path: Path) -> int | None:
    if not path.exists():
        return None
    text = path.read_text(errors="ignore")
    match = re.search(r"total ([0-9]+)m([0-9]+)s", text)
    if match:
        return int(match.group(1)) * 60 + int(match.group(2))
    match = re.search(r"total ([0-9]+)s", text)
    if match:
        return int(match.group(1))
    return None


def record_from_row(set_dir: Path, row: dict[str, Any]) -> RunRecord:
    set_id = set_dir.name
    run_id = str(row.get("name") or row.get("run") or row.get("id") or "unknown")
    prompt = str(row.get("prompt") or row.get("goal") or "")
    scenario = normalize_scenario(row.get("scenario"), run_id, prompt)
    planner = str(row.get("planner") or row.get("planner_model") or "")
    executor = str(row.get("executor") or row.get("model") or "")
    plan_preset = (
        str(row.get("plan_preset") or row.get("plan_preset_arg") or row.get("preset") or "")
        or nested_value(row, "plan_preset_resolved", "value")
        or nested_value(row, "preset_resolved", "value")
        or "unknown"
    )
    final_raw = row.get("final_acceptance") or row.get("summary_final_acceptance")
    status = row.get("status") or row.get("summary_status")
    gate = row.get("release_gate") or row.get("summary_release_gate")
    final_state = normalize_final(final_raw, status, gate)
    stop_reason = str(row.get("stop_reason") or row.get("summary_stop_reason") or "")
    stop_class = classify_stop(row, stop_reason, final_state)
    elapsed = row.get("elapsed_seconds")
    elapsed_seconds = int(elapsed) if isinstance(elapsed, int) else None
    if elapsed_seconds is None:
        for art_dir in artifact_dirs(set_dir, row, run_id):
            elapsed_seconds = elapsed_from_summary(art_dir / "summary.md")
            if elapsed_seconds is not None:
                break
    excluded_reason = should_exclude(stop_class, stop_reason)
    false_full_reason = ""
    if final_state == "full_success" and not has_interaction_pass(set_dir, row, run_id):
        false_full_reason = "missing_browser_interaction_pass_evidence"
    return RunRecord(
        set_id=set_id,
        run_id=run_id,
        scenario=scenario,
        planner=planner,
        executor=executor,
        plan_preset=plan_preset,
        final_state=final_state,
        stop_class=stop_class,
        elapsed_seconds=elapsed_seconds,
        source="aggregate",
        excluded_reason=excluded_reason,
        false_full_reason=false_full_reason,
        prompt=prompt,
    )


def aggregate_rows(path: Path) -> list[dict[str, Any]]:
    data = json.loads(path.read_text())
    if isinstance(data, list):
        return [row for row in data if isinstance(row, dict)]
    if isinstance(data, dict):
        rows = data.get("results")
        if isinstance(rows, list):
            return [row for row in rows if isinstance(row, dict)]
    return []


def parse_report_only(set_dir: Path) -> list[RunRecord]:
    report = set_dir / "uat-report.md"
    if not report.exists():
        return []
    text = report.read_text(errors="ignore")
    if "## Smoke Result" not in text:
        return []
    run_id = "smoke_result"
    scenario = normalize_scenario(text)
    planner = find_markdown_value(text, "Planner")
    executor = find_markdown_value(text, "Executor")
    preset_match = re.search(r"Resolved preset:\s*`?([A-Za-z0-9_-]+)`?", text)
    plan_preset = preset_match.group(1) if preset_match else "unknown"
    final = find_markdown_value(text, "Final acceptance")
    release_gate = find_markdown_value(text, "Release gate")
    final_state = normalize_final(final, "", release_gate)
    stop_class = "full" if final_state == "full_success" else final_state
    elapsed = elapsed_from_summary(set_dir / "artifacts" / "attempt-2-pass" / "summary.md")
    full_has_pass = "Interaction evidence: passed" in text
    false_full = ""
    if final_state == "full_success" and not full_has_pass:
        summary = set_dir / "artifacts" / "attempt-2-pass" / "summary.md"
        if summary.exists() and "Interaction evidence: passed" in summary.read_text(errors="ignore"):
            full_has_pass = True
    if final_state == "full_success" and not full_has_pass:
        false_full = "missing_browser_interaction_pass_evidence"
    return [
        RunRecord(
            set_id=set_dir.name,
            run_id=run_id,
            scenario=scenario,
            planner=planner,
            executor=executor,
            plan_preset=plan_preset,
            final_state=final_state,
            stop_class=stop_class,
            elapsed_seconds=elapsed,
            source="report",
            false_full_reason=false_full,
        )
    ]


def find_markdown_value(text: str, label: str) -> str:
    match = re.search(rf"- {re.escape(label)}:\s*`?([^`\n]+)`?", text)
    return match.group(1).strip() if match else ""


def discover_records() -> tuple[list[RunRecord], int, int, list[str]]:
    records: list[RunRecord] = []
    aggregate_row_total = 0
    aggregate_record_total = 0
    scanned_sets: list[str] = []
    for set_dir in sorted(RUNS_DIR.glob("uat-*")):
        if set_dir.name < WINDOW_START:
            continue
        if not set_dir.is_dir():
            continue
        scanned_sets.append(set_dir.name)
        aggregate = set_dir / "aggregate.json"
        if aggregate.exists():
            rows = aggregate_rows(aggregate)
            aggregate_row_total += len(rows)
            for row in rows:
                records.append(record_from_row(set_dir, row))
                aggregate_record_total += 1
        else:
            records.extend(parse_report_only(set_dir))
    assert aggregate_record_total == aggregate_row_total, (
        f"aggregate-derived record count {aggregate_record_total} != "
        f"aggregate.json row count {aggregate_row_total}"
    )
    return records, aggregate_row_total, aggregate_record_total, scanned_sets


def pct(num: int, den: int) -> str:
    if den == 0:
        return "0%"
    return f"{round(num * 100 / den)}%"


def state_counts(records: list[RunRecord]) -> dict[str, Counter[str]]:
    counts: dict[str, Counter[str]] = defaultdict(Counter)
    for rec in records:
        if rec.excluded_reason:
            continue
        counts[rec.scenario][rec.final_state] += 1
    return counts


def executor_counts(records: list[RunRecord]) -> dict[tuple[str, str], Counter[str]]:
    counts: dict[tuple[str, str], Counter[str]] = defaultdict(Counter)
    for rec in records:
        if rec.excluded_reason:
            continue
        counts[(rec.scenario, rec.executor or "unknown")] [rec.final_state] += 1
    return counts


def median(values: list[int]) -> int:
    return int(statistics.median(values))


def time_label(seconds: int | None) -> str:
    if seconds is None:
        return "unknown"
    mins, secs = divmod(seconds, 60)
    return f"{mins}m{secs:02d}s"


def table(headers: list[str], rows: list[list[str]]) -> list[str]:
    lines = ["| " + " | ".join(headers) + " |"]
    lines.append("| " + " | ".join("---" for _ in headers) + " |")
    lines.extend("| " + " | ".join(row) + " |" for row in rows)
    return lines


def build_summary(records: list[RunRecord], aggregate_row_total: int, scanned_sets: list[str]) -> str:
    included = [rec for rec in records if not rec.excluded_reason]
    excluded = [rec for rec in records if rec.excluded_reason]
    unknowns = [rec for rec in included if rec.scenario == "unknown"]
    false_full = [rec for rec in included if rec.false_full_reason]
    source_counts = Counter(rec.source for rec in records)
    planner_counts = Counter(rec.planner or "unknown" for rec in included)
    lines: list[str] = []
    lines.append("# Next.js Create Capability Band Summary")
    lines.append("")
    lines.append(f"- Window start: `{WINDOW_START}`")
    lines.append(f"- Scanned UAT sets: `{len(scanned_sets)}`")
    lines.append(f"- Aggregate.json rows asserted: `{aggregate_row_total}`")
    lines.append(f"- Total run records: `{len(records)}`")
    lines.append(f"- Record sources: `{dict(sorted(source_counts.items()))}`")
    lines.append(f"- Included denominator after exclusions: `{len(included)}`")
    lines.append(f"- Excluded infrastructure records: `{len(excluded)}`")
    lines.append("")
    lines.append("## Planner Coverage")
    rows = [[planner, str(count)] for planner, count in sorted(planner_counts.items())]
    lines.extend(table(["Planner", "included records"], rows))
    lines.append("")
    lines.append("## Scenario x Final State")
    rows: list[list[str]] = []
    counts = state_counts(included)
    for scenario in sorted(counts):
        counter = counts[scenario]
        den = sum(counter.values())
        full = counter["full_success"]
        note = " n<10" if den < 10 else ""
        rows.append([
            scenario,
            str(counter["full_success"]),
            str(counter["partial"]),
            str(counter["incomplete"]),
            str(counter["failed"]),
            str(den),
            f"{pct(full, den)}{note}",
        ])
    lines.extend(table(["Scenario", "full", "partial", "incomplete", "failed", "n", "full rate"], rows))
    lines.append("")
    lines.append("## Scenario x Executor")
    rows = []
    for (scenario, executor), counter in sorted(executor_counts(included).items()):
        den = sum(counter.values())
        full = counter["full_success"]
        note = " n<10" if den < 10 else ""
        rows.append([scenario, executor, str(full), str(den), f"{pct(full, den)}{note}"])
    lines.extend(table(["Scenario", "Executor", "full", "n", "full rate"], rows))
    lines.append("")
    lines.append("## Full Run Durations")
    rows = []
    full_by_scenario: dict[str, list[int]] = defaultdict(list)
    all_full: list[int] = []
    for rec in included:
        if rec.final_state == "full_success" and rec.elapsed_seconds is not None:
            full_by_scenario[rec.scenario].append(rec.elapsed_seconds)
            all_full.append(rec.elapsed_seconds)
    if all_full:
        rows.append([
            "all",
            str(len(all_full)),
            time_label(min(all_full)),
            time_label(median(all_full)),
            time_label(max(all_full)),
        ])
    for scenario in sorted(full_by_scenario):
        values = full_by_scenario[scenario]
        rows.append([
            scenario,
            str(len(values)),
            time_label(min(values)),
            time_label(median(values)),
            time_label(max(values)),
        ])
    lines.extend(table(["Scope", "full runs", "min", "median", "max"], rows))
    lines.append("")
    lines.append("## Excluded and Unknown Runs")
    if excluded:
        rows = [[rec.set_id, rec.run_id, rec.scenario, rec.excluded_reason] for rec in excluded]
        lines.extend(table(["Set", "Run", "Scenario", "Reason"], rows))
    else:
        lines.append("- Excluded infrastructure runs: none")
    if unknowns:
        rows = [[rec.set_id, rec.run_id, rec.stop_class] for rec in unknowns]
        lines.append("")
        lines.append("Unknown scenario records:")
        lines.extend(table(["Set", "Run", "Stop class"], rows))
    else:
        lines.append("- Unknown scenario records: none")
    lines.append("")
    lines.append("## False-Full Check")
    if false_full:
        rows = [[rec.set_id, rec.run_id, rec.scenario, rec.false_full_reason] for rec in false_full]
        lines.extend(table(["Set", "Run", "Scenario", "Reason"], rows))
    else:
        lines.append("- False-full suspects: 0")
    lines.append("")
    lines.append("## Stop-Class Distribution")
    stop_counts: dict[str, Counter[str]] = defaultdict(Counter)
    for rec in included:
        stop_counts[rec.scenario][rec.stop_class] += 1
    rows = []
    for scenario in sorted(stop_counts):
        parts = ", ".join(f"{k}={v}" for k, v in sorted(stop_counts[scenario].items()))
        rows.append([scenario, parts])
    lines.extend(table(["Scenario", "Stop classes"], rows))
    lines.append("")
    lines.append("## Provisional Comparison")
    rows = []
    for scenario, expected in PROVISIONAL.items():
        counter = counts.get(scenario, Counter())
        den = sum(counter.values())
        actual = round(counter["full_success"] * 100 / den) if den else 0
        delta = actual - expected
        note = ""
        if abs(delta) > 15:
            note = "diff >15pp; target window includes post-0711 gate/A-B/task28 sets and counts every rerun in time order"
        rows.append([scenario, f"{expected}%", f"{actual}%", f"{delta:+d}pp", note])
    lines.extend(table(["Scenario", "Provisional", "Measured", "Delta", "Note"], rows))
    lines.append("")
    lines.append("## Source Sets")
    for set_id in scanned_sets:
        lines.append(f"- `{set_id}`")
    lines.append("")
    return "\n".join(lines)


def archived_data_records() -> list[DataRunRecord]:
    records: list[DataRunRecord] = []
    for line in DATA_ARCHIVED_RUN_INDEX.splitlines():
        fields = line.split("|")
        assert len(fields) == 10, f"invalid archived data row: {line}"
        (
            set_id,
            run_name,
            executor,
            preset,
            final_acceptance,
            assurance,
            failure_class,
            duration,
            excluded_reason,
            source,
        ) = fields
        duration_seconds = int(duration) if duration else None
        if not source.startswith("archived:"):
            assert (RUNS_DIR / source).exists(), (
                f"missing archived data source: {source}"
            )
        records.append(
            DataRunRecord(
                set_id=set_id,
                record_dir=set_id,
                run_name=run_name,
                planner=DATA_PLANNER,
                executor=executor,
                preset=preset,
                final_acceptance=final_acceptance,
                assurance=assurance,
                failure_class=failure_class,
                duration_seconds=duration_seconds,
                source=source,
                excluded_reason=excluded_reason,
            )
        )
    return records


def data_meta_is_data(data: dict[str, Any]) -> bool:
    measurement = data.get("measurement")
    if isinstance(measurement, dict) and measurement.get("profile") == "data":
        return True
    uat_id = str(data.get("uat_id") or "")
    runs = data.get("runs")
    return "-data-" in uat_id and isinstance(runs, list)


def read_json_dict(path: Path) -> dict[str, Any] | None:
    if not path.exists():
        return None
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    return value if isinstance(value, dict) else None


def data_assurance_status(evidence_dir: Path) -> str:
    data = read_json_dict(evidence_dir / "data-assurance.json")
    if data is None:
        return ""
    return str(data.get("status") or data.get("assurance_level") or "").lower()


def data_record_from_meta(
    set_dir: Path,
    set_id: str,
    planner: str,
    row: dict[str, Any],
) -> DataRunRecord:
    run_name = str(row.get("name") or row.get("run") or row.get("id") or "unknown")
    final_acceptance = str(
        row.get("final_acceptance_status")
        or row.get("final_acceptance")
        or "not_checked"
    ).lower()
    evidence_dir = set_dir / "artifacts" / run_name / "evidence"
    earned_status = data_assurance_status(evidence_dir)
    if final_acceptance == "full_success":
        assurance = earned_status or "missing"
    else:
        # Campaign uat-meta assurance is the evidence-audited value in these
        # records, not the tui_command_stop/run_stop terminal projection.
        assurance = str(row.get("assurance_level") or earned_status or "failed").lower()
    failure_class = str(row.get("stop_class") or row.get("failure_class") or "")
    if final_acceptance == "full_success" and not failure_class:
        failure_class = "full"
    duration = row.get("duration_seconds")
    duration_seconds = (
        round(float(duration)) if isinstance(duration, (int, float)) else None
    )
    return DataRunRecord(
        set_id=set_id,
        record_dir=set_dir.name,
        run_name=run_name,
        planner=planner,
        executor=str(row.get("executor") or row.get("model") or "unknown"),
        preset=str(row.get("preset") or row.get("plan_preset") or "unknown"),
        final_acceptance=final_acceptance,
        assurance=assurance,
        failure_class=failure_class or final_acceptance,
        duration_seconds=duration_seconds,
        source=f"{set_dir.name}/uat-meta.json",
        evidence_dir=evidence_dir,
    )


def discover_data_records() -> tuple[list[DataRunRecord], int, int, list[str]]:
    meta_records: list[DataRunRecord] = []
    meta_row_count = 0
    scanned_sets: set[str] = set()
    for set_dir in sorted(RUNS_DIR.glob("uat-*")):
        meta_path = set_dir / "uat-meta.json"
        data = read_json_dict(meta_path)
        if data is None or not data_meta_is_data(data):
            continue
        runs = data.get("runs")
        assert isinstance(runs, list), f"data uat-meta runs is not a list: {meta_path}"
        measurement = data.get("measurement")
        assert isinstance(measurement, dict), f"data measurement missing: {meta_path}"
        fixture_sha = str(measurement.get("input_sha256") or "")
        if fixture_sha:
            assert fixture_sha == DATA_FIXTURE_SHA256, (
                f"fixture hash mismatch in {meta_path}: {fixture_sha}"
            )
        set_id = str(data.get("uat_id") or set_dir.name)
        planner = str(measurement.get("planner_model") or DATA_PLANNER)
        scanned_sets.add(set_id)
        for row in runs:
            assert isinstance(row, dict), f"non-object run in {meta_path}"
            meta_records.append(data_record_from_meta(set_dir, set_id, planner, row))
            meta_row_count += 1

    # If a pre-uat-meta campaign later gains managed metadata, prefer that
    # metadata and suppress the matching frozen row rather than double count.
    meta_keys = {(record.set_id, record.run_name) for record in meta_records}
    archived = [
        record
        for record in archived_data_records()
        if (record.set_id, record.run_name) not in meta_keys
    ]
    records = archived + meta_records
    scanned_sets.update(record.set_id for record in archived)
    scanned_run_count = len(archived) + meta_row_count
    assert len(records) == scanned_run_count, (
        f"data output rows {len(records)} != scanned run rows {scanned_run_count}"
    )
    keys = [(record.set_id, record.run_name) for record in records]
    assert len(keys) == len(set(keys)), "duplicate data set/run rows discovered"
    return records, scanned_run_count, meta_row_count, sorted(scanned_sets)


def evidence_passes(path: Path) -> bool:
    data = read_json_dict(path)
    if data is None:
        return False
    status = str(data.get("status") or "").lower()
    return data.get("ok") is True or status in {"pass", "passed", "success", "full"}


def assert_full_data_evidence(records: list[DataRunRecord]) -> int:
    required = (
        "pipeline-run.json",
        "reconciliation.json",
        "claims-binding.json",
        "rerun-consistency.json",
        "results-schema.json",
    )
    verified = 0
    for record in records:
        if record.final_acceptance != "full_success":
            continue
        assert record.evidence_dir is not None, (
            f"full row has no evidence directory: {record.set_id}/{record.run_name}"
        )
        missing_or_failed = [
            name for name in required if not evidence_passes(record.evidence_dir / name)
        ]
        assert not missing_or_failed, (
            f"false-full evidence gap for {record.set_id}/{record.run_name}: "
            f"{', '.join(missing_or_failed)}"
        )
        assurance = read_json_dict(record.evidence_dir / "data-assurance.json")
        assert (
            assurance is not None
            and data_assurance_status(record.evidence_dir) == "full"
        ), f"full row lacks earned data-assurance: {record.set_id}/{record.run_name}"
        checks = assurance.get("checks")
        assert isinstance(checks, dict), (
            f"full data-assurance lacks checks: {record.set_id}/{record.run_name}"
        )
        required_checks = (
            "pipeline_probe",
            "data_reconciliation",
            "data_claims_binding",
            "data_rerun_consistency",
            "data_results_schema",
        )
        assert all(checks.get(check) is True for check in required_checks), (
            f"full data-assurance check mismatch: {record.set_id}/{record.run_name}"
        )
        verified += 1
    return verified


def data_rate_rows(records: list[DataRunRecord]) -> list[list[str]]:
    counts: dict[tuple[str, str], Counter[str]] = defaultdict(Counter)
    for record in records:
        counts[(record.executor, record.preset)][
            "full" if record.is_full else "non_full"
        ] += 1
    rows: list[list[str]] = []
    for (executor, preset), counter in sorted(counts.items()):
        full = counter["full"]
        denominator = sum(counter.values())
        note = " n<10" if denominator < 10 else ""
        rows.append(
            [
                executor,
                preset,
                str(full),
                str(denominator),
                f"{pct(full, denominator)}{note}",
            ]
        )
    return rows


def data_failure_rows(records: list[DataRunRecord]) -> list[list[str]]:
    counts: dict[tuple[str, str], Counter[str]] = defaultdict(Counter)
    for record in records:
        if not record.is_full:
            counts[(record.executor, record.preset)][record.failure_class] += 1
    rows: list[list[str]] = []
    for (executor, preset), counter in sorted(counts.items()):
        distribution = ", ".join(
            f"{failure_class}={count}"
            for failure_class, count in sorted(counter.items())
        )
        rows.append([executor, preset, distribution])
    return rows


def append_data_window(
    lines: list[str],
    title: str,
    definition: str,
    records: list[DataRunRecord],
) -> None:
    lines.append(f"## {title}")
    lines.append("")
    lines.append(definition)
    lines.append("")
    lines.append(f"- Denominator: `{len(records)}`")
    lines.append(f"- Full: `{sum(record.is_full for record in records)}`")
    lines.append("")
    lines.extend(
        table(
            ["Executor", "Preset", "full", "n", "full rate"],
            data_rate_rows(records),
        )
    )
    lines.append("")
    lines.append("### Failure-class distribution")
    lines.append("")
    lines.extend(
        table(
            ["Executor", "Preset", "Failure classes"],
            data_failure_rows(records),
        )
    )
    lines.append("")


def build_data_summary(
    records: list[DataRunRecord],
    scanned_run_count: int,
    meta_row_count: int,
    scanned_sets: list[str],
    full_evidence_verified: int,
) -> str:
    included = [record for record in records if not record.excluded_reason]
    excluded = [record for record in records if record.excluded_reason]
    stable = [
        record for record in included if record.set_id >= DATA_STABLE_WINDOW_START
    ]
    assert len(records) == scanned_run_count
    assert len(included) + len(excluded) == scanned_run_count
    assert stable, "mechanism-stable data window is empty"

    lines: list[str] = [
        "# Data × Create Capability Band Summary",
        "",
        "<!-- Generated by band_aggregate.py --profile data. Do not edit by hand. -->",
        "",
        f"- Planner: `{DATA_PLANNER}`",
        f"- Input fixture SHA-256: `{DATA_FIXTURE_SHA256}`",
        f"- Scanned campaign sets: `{len(scanned_sets)}`",
        f"- Scanned data run rows: `{scanned_run_count}`",
        f"- Repository uat-meta rows: `{meta_row_count}`",
        f"- Frozen pre-uat-meta rows: `{scanned_run_count - meta_row_count}`",
        f"- Window A included denominator: `{len(included)}`",
        f"- Excluded invalid/discarded rows: `{len(excluded)}`",
        f"- Full rows with E1–E4 and data-assurance verified: `{full_evidence_verified}`",
        "- False-full evidence gaps: `0` (generation aborts on any gap)",
        "",
        "Assurance truth follows B-2j: final acceptance and "
        "`evidence/data-assurance.json` are authoritative for full; historical "
        "terminal projection fields are not read. Non-full levels come from the "
        "campaign's evidence-audited `uat-meta.json` or frozen pre-uat-meta audit row.",
        "",
        "The frozen pre-uat-meta index is code-owned input for UAT #1 and M-4; "
        "it preserves rows whose original aggregate files predate repository-managed "
        "`uat-meta.json`. New and mixed campaigns are discovered from every "
        "`workspace/management/runs/uat-*/uat-meta.json` whose measurement profile is data.",
        "",
    ]
    append_data_window(
        lines,
        "Window A — all history",
        "UAT #1 through #7, including the machine-defect era. Invalid or discarded "
        "measurements remain visible below but are outside this denominator.",
        included,
    )
    append_data_window(
        lines,
        "Window B — mechanism-stable",
        f"`{DATA_STABLE_WINDOW_START}` and later: DATA-1–12 are fixed and the "
        "earned-assurance projection contract is in force. This is the update baseline.",
        stable,
    )

    full_records = [record for record in included if record.is_full]
    duration_values = [
        record.duration_seconds
        for record in full_records
        if record.duration_seconds is not None
    ]
    lines.extend(["## Full durations", ""])
    duration_rows = [
        [
            record.set_id,
            record.run_name,
            record.executor,
            record.preset,
            f"{record.duration_seconds}s"
            if record.duration_seconds is not None
            else "unknown",
        ]
        for record in full_records
    ]
    lines.extend(
        table(
            ["Set", "Run", "Executor", "Preset", "Duration"],
            duration_rows,
        )
    )
    if duration_values:
        lines.extend(
            [
                "",
                f"- n=`{len(duration_values)}`; min=`{min(duration_values)}s`; "
                f"median=`{median(duration_values)}s`; max=`{max(duration_values)}s`.",
            ]
        )

    lines.extend(["", "## Excluded rows", ""])
    lines.extend(
        table(
            ["Set", "Run", "Final acceptance", "Failure class", "Reason"],
            [
                [
                    record.set_id,
                    record.run_name,
                    record.final_acceptance,
                    record.failure_class,
                    record.excluded_reason,
                ]
                for record in excluded
            ],
        )
    )
    lines.extend(
        [
            "",
            "`uat-test0714-m4-002` is discarded for operator model-ID substitution. "
            "`uat-test0714-m4-004` is outside the denominator because cargo-test "
            "preflight was not green and the campaign was interrupted before four of "
            "its five data rows completed; no interrupted result is inferred.",
            "",
            "## Per-run ledger",
            "",
        ]
    )
    ledger_rows: list[list[str]] = []
    for record in sorted(records, key=lambda item: (item.set_id, item.run_name)):
        if record.excluded_reason:
            window = "excluded"
        elif record.set_id >= DATA_STABLE_WINDOW_START:
            window = "A+B"
        else:
            window = "A"
        ledger_rows.append(
            [
                record.set_id,
                record.record_dir,
                record.run_name,
                record.executor,
                record.preset,
                record.final_acceptance,
                record.assurance,
                record.failure_class,
                f"{record.duration_seconds}s"
                if record.duration_seconds is not None
                else "unknown",
                window,
            ]
        )
    assert len(ledger_rows) == scanned_run_count, (
        f"rendered ledger rows {len(ledger_rows)} != scanned rows {scanned_run_count}"
    )
    lines.extend(
        table(
            [
                "Set",
                "Record directory",
                "Run",
                "Executor",
                "Preset",
                "Final acceptance",
                "Assurance",
                "Failure class",
                "Duration",
                "Window",
            ],
            ledger_rows,
        )
    )
    lines.extend(["", "## Source sets", ""])
    lines.extend(
        table(
            ["Set ID", "Record directory"],
            [
                [set_id, record_dir]
                for set_id, record_dir in sorted(
                    {(record.set_id, record.record_dir) for record in records}
                )
            ],
        )
    )
    lines.append("")
    return "\n".join(lines)



def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--profile",
        choices=("nextjs", "data"),
        default="nextjs",
        help="capability band to aggregate (default: nextjs)",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.profile == "data":
        data_records, scanned_rows, meta_rows, scanned_sets = discover_data_records()
        full_verified = assert_full_data_evidence(data_records)
        summary = build_data_summary(
            data_records,
            scanned_rows,
            meta_rows,
            scanned_sets,
            full_verified,
        )
        output = DATA_OUTPUT
    else:
        records, aggregate_row_total, _aggregate_record_total, scanned_sets = (
            discover_records()
        )
        summary = build_summary(records, aggregate_row_total, scanned_sets)
        output = OUTPUT
    output.write_text(summary, encoding="utf-8")
    print(summary)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
