#!/usr/bin/env python3
"""Aggregate local-tier capability bands across profile and intent axes.

Next.js retains its original aggregate.json/report input path and output bytes.
Data uses repository-managed uat-meta.json files plus a frozen index for the
pre-uat-meta campaigns. Fix/nextjs uses the four fixed D-1 measurement sets,
their event streams, and their F1-F3 evidence. Investigation/data uses the two
fixed D-3b measurement sets and validates their I1/I2 evidence against
adjudication events. Workflow circle uses the three fixed D-3a local-arm
campaigns and admits only the post-propagation, post-execution-mode campaign
to its formal denominator. CLI and ingest keep defect-era calibration
campaigns visible while restricting their formal elevated denominators to the
declared Window B. Generated summaries are written below
workspace/management/runs/ and printed to stdout.
"""

from __future__ import annotations

import argparse
import json
import re
import statistics
import sys
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from id_vocabulary import INTERRUPTED_ENVIRONMENT
from task_family_vocabulary import (
    AGGREGATION,
    BREAKOUT,
    COMPILE_ERROR_FIX,
    CONTRACT_HOOK_FIX,
    DATA_CREATE_FAMILIES,
    DATA_INVESTIGATE_FAMILIES,
    NEXTJS_FIX_FAMILIES,
    PIPE,
    QUIZ,
    SCHEMA,
    SPACE,
    TIMESERIES,
    UNKNOWN,
)

ROOT = Path(__file__).resolve().parents[3]
RUNS_DIR = ROOT / "workspace" / "management" / "runs"
OUTPUT = RUNS_DIR / "band_summary.md"
DATA_OUTPUT = RUNS_DIR / "band_summary_data.md"
FIX_OUTPUT = RUNS_DIR / "band_summary_fix.md"
INVESTIGATION_OUTPUT = RUNS_DIR / "band_summary_investigation.md"
CIRCLE_OUTPUT = RUNS_DIR / "band_summary_circle.md"
CLI_OUTPUT = RUNS_DIR / "band_summary_cli.md"
INGEST_OUTPUT = RUNS_DIR / "band_summary_ingest.md"
FULL_MEANING_LABELS = {
    "nextjs": (
        "build + real-browser route, interaction, and state-change evidence; "
        "T1 testimony binding is active, with violations failing and "
        "claims_absent/unrecognized prose recorded without promotion."
    ),
    "data": (
        "pipeline execution plus E1 inspection, E2 claim binding, E3 rerun "
        "consistency, and E4 schema conformance; testimony binding is active as E2."
    ),
    "fix": (
        "the before-state reproduces, the repair makes the check pass, and no "
        "regression remains under F1-F3; no separate testimony check is active."
    ),
    "investigation": (
        "I1 executes a failing reproducer and I2 binds report claims to observed "
        "evidence; testimony binding is active as I2."
    ),
    "cli": (
        "C1-C4 pass, including README output claims bound to live CLI output by "
        "C3; testimony binding is active as C3."
    ),
    "ingest": (
        "N1-N5 pass, including source-bound record values and complete candidate "
        "accounting; testimony/source binding is active as N2."
    ),
}
WINDOW_START = "uat-test0711-bs-003"
NEXTJS_ARCHIVED_INPUT_SET_COUNT = 12
NEXTJS_PROVENANCE_ANALYSIS = "band-f821-diff/analysis.md"
DATA_PLANNER = "qwen3.6:27b-coding-nvfp4"
DATA_FIXTURE_SHA256 = "2f6c04e42b0ebdff85a7eb6b52a342610155be6796bd89e5729075d87c78d873"
# The frozen pre-uat-meta rows all use this goal, quoted in their immutable UAT
# and investigation records; family classification still consumes goal text.
DATA_AGGREGATION_GOAL = (
    "data/sales.csv を読み込み、月次×地域の売上集計と全体合計を計算し、"
    "無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。"
)
DATA_FAMILIES = DATA_CREATE_FAMILIES
DATA_FAMILY_STABLE_WINDOWS = {
    "aggregation": ("uat-test0715-data-007", "7b177fe"),
    "timeseries": ("uat-test0716-data-009", "2028eb4"),
}
FIX_WINDOW_SETS = tuple(f"uat-test0717-fix-{index:03d}" for index in range(1, 5))
FIX_BENCH_SET = "uat-test0719-dfix-006"
FIX_EXPECTED_RUNS = 30
FIX_WINDOW_B_BASELINE_HEAD = "6decdce"
FIX_FAMILIES = NEXTJS_FIX_FAMILIES
FIX_ENVIRONMENT_HOLDS = {
    (
        "uat-test0717-fix-001",
        "fix2_hook_qwen35_001",
    ): "host NODE_ENV=production skipped devDependencies before FIX-1",
    (
        "uat-test0717-fix-001",
        "fix2_hook_qwen35_002",
    ): "host NODE_ENV=production skipped devDependencies before FIX-1",
}
INVESTIGATION_WINDOW_SETS = (
    "uat-test0718-inv-001",
    "uat-test0718-inv-002",
)
INVESTIGATION_WINDOW_B_SET = "uat-test0718-inv-002"
INVESTIGATION_WINDOW_B_BASELINE_HEAD = "3302dd9"
INVESTIGATION_EXPECTED_RUNS = 12
INVESTIGATION_FAMILIES = DATA_INVESTIGATE_FAMILIES
CIRCLE_WINDOW_SETS = tuple(
    [f"uat-test0722-circle-{index:03d}" for index in range(1, 4)]
    + [f"uat-test0722-circle-elev-{index:03d}" for index in range(1, 9)]
)
CIRCLE_OFFICIAL_SET = "uat-test0722-circle-elev-008"
CIRCLE_EXPECTED_RUNS = 33
CLI_LOCAL_SET = "uat-test0724-cli-001-v3"
CLI_LOCAL_SUMMARY = RUNS_DIR / CLI_LOCAL_SET / "evidence" / "campaign-summary.json"
CLI_ELEVATED_SETS = tuple(f"uat-test0725-cli-elev-{index:03d}" for index in range(1, 5))
CLI_WINDOW_B_SET = "uat-test0725-cli-elev-004"
CLI_CALIBRATION_SET = "uat-test0725-cli-elev-003"
CLI_PACK_SETS = (
    "uat-test0730-cli-pack-001",
    "uat-test0730-cli-pack-002",
    "uat-test0730-cli-pack-003",
)
CLI_PACK_SUMMARIES = {
    set_id: RUNS_DIR / set_id / "evidence" / "campaign-summary.json"
    for set_id in CLI_PACK_SETS
}
CLI_PACK_ID = "cli-assist@1.0.0"
CLI_PACK_HASH = (
    "sha256:b1dcee70c1a0536954c25639e2d67508d8029328e414aaff030368e7fac844fd"
)
CLI_PACK_V1_1_ID = "cli-assist@1.1.0"
CLI_PACK_V1_1_HASH = (
    "sha256:3d11e126d3afbcd8a53e23367d53859924c700aeaf5345fa366060d66c917c82"
)
CLI_PACK_PINS = {
    CLI_PACK_SETS[0]: (CLI_PACK_ID, CLI_PACK_HASH),
    CLI_PACK_SETS[1]: (CLI_PACK_ID, CLI_PACK_HASH),
    CLI_PACK_SETS[2]: (CLI_PACK_V1_1_ID, CLI_PACK_V1_1_HASH),
}
CLI_DIRECTIVE_SET = "d3c-shakedown-002"
CLI_DIRECTIVE_MEASUREMENTS = (
    RUNS_DIR / CLI_DIRECTIVE_SET / "measurement.json",
    RUNS_DIR / CLI_DIRECTIVE_SET / "measurement-round-2.json",
)
CLI_LUNA_SET = "uat-test0801-cli-luna-001"
CLI_LUNA_SUMMARY = RUNS_DIR / CLI_LUNA_SET / "evidence" / "campaign-summary.json"
CLI_EXPECTED_RUNS_PER_SET = 6
CLI_BASE_EXPECTED_RUNS = CLI_EXPECTED_RUNS_PER_SET * (
    2 + len(CLI_ELEVATED_SETS) + len(CLI_PACK_SETS)
)
CLI_EXPECTED_MATRIX = {
    ("filter", "gemma4:31b"): 1,
    ("filter", "qwen3.6:35b-a3b-coding-nvfp4"): 2,
    ("stats", "gemma4:31b"): 1,
    ("stats", "qwen3.6:35b-a3b-coding-nvfp4"): 2,
}
CLI_EVIDENCE_FILES = (
    "cli-case-binding.json",
    "cli-probe.json",
    "help-binding.json",
    "cli-assurance.json",
)
CLI_EXCLUSION_REASONS = {
    "uat-test0725-cli-elev-001": (
        "機械欠陥期: C1未実行をpartialへ投影したcompletion写像欠落"
    ),
    "uat-test0725-cli-elev-002": (
        "機械欠陥期: final acceptance到達後もC1〜C4 runtimeが未配線"
    ),
}
INGEST_LOCAL_ALIAS = "uat-test0726-ingest-001-v3"
INGEST_LOCAL_SOURCE_SET = "uat-test0726-ingest-001"
INGEST_ELEVATED_SETS = tuple(
    f"uat-test0726-ingest-elev-{index:03d}" for index in range(1, 9)
)
INGEST_WINDOW_B_SET = "uat-test0726-ingest-elev-008"
INGEST_EXPECTED_RUNS_PER_SET = 6
INGEST_EXPECTED_RUNS = INGEST_EXPECTED_RUNS_PER_SET * (
    1 + len(INGEST_ELEVATED_SETS)
)
INGEST_LOCAL_MATRIX = {
    ("list", "gemma4:31b"): 1,
    ("list", "qwen3.6:35b-a3b-coding-nvfp4"): 2,
    ("table", "gemma4:31b"): 1,
    ("table", "qwen3.6:35b-a3b-coding-nvfp4"): 2,
}
INGEST_WINDOW_B_MATRIX = {
    ("list", "gemma4:31b-cloud"): 3,
    ("table", "gemma4:31b-cloud"): 3,
}
INGEST_EXCLUSION_REASONS = {
    "uat-test0726-ingest-elev-001": (
        "較正弧除外: 相異なる成功commandをno-diff停滞とした進捗意味論gap"
    ),
    "uat-test0726-ingest-elev-002": (
        "較正弧除外: candidate_selectorのkind/value正準形を字義配布しないknowledge gap"
    ),
    "uat-test0726-ingest-elev-003": (
        "較正弧除外: planner自由作文とverify path残存を許した計画源gap"
    ),
    "uat-test0726-ingest-elev-004": (
        "較正弧除外: run生成物をimplement期待へ置いた段×生成主体の分解gap"
    ),
    "uat-test0726-ingest-elev-005": (
        "較正弧除外: 計画源と同じ伝達床でsnapshot実構造をimplementへ渡さない材料gap"
    ),
    "uat-test0726-ingest-elev-006": (
        "較正弧除外: 正当な複合CSSのengine被覆gap（同時に契約v0.1較正を採取）"
    ),
    "uat-test0726-ingest-elev-007": (
        "較正弧除外: freeze済み正準candidate IDをimplementへ返さない語彙配布gap"
    ),
}
CIRCLE_EXCLUSION_REASONS = {
    "uat-test0722-circle-001": "profile不伝播により無効（P1-a FAIL）",
    "uat-test0722-circle-002": "実行モード欠落により無効",
    "uat-test0722-circle-003": "localアーム（正式値札ではない）",
    "uat-test0722-circle-elev-001": "R提示不発",
    "uat-test0722-circle-elev-002": "E-A過投影",
    "uat-test0722-circle-elev-003": "I2認識錨",
    "uat-test0722-circle-elev-004": "照準被覆",
    "uat-test0722-circle-elev-005": "存在前提（第1周）",
    "uat-test0722-circle-elev-006": "存在前提（第2周）／binary乖離・HTTP500",
    "uat-test0722-circle-elev-007": "存在前提（第3周）",
}
KNOWN_INTENTS = {"create", "fix", "investigate"}

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
    intent: str
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
    goal: str
    family: str
    final_acceptance: str
    assurance: str
    failure_class: str
    duration_seconds: int | None
    source: str
    intent: str
    excluded_reason: str = ""
    evidence_dir: Path | None = None

    @property
    def is_full(self) -> bool:
        return self.final_acceptance == "full_success" and self.assurance == "full"


@dataclass(frozen=True)
class IntentResolution:
    value: str
    source: str


@dataclass(frozen=True)
class SetFilterDiagnostic:
    set_id: str
    accepted_rows: int
    reason: str


class EmptyAggregationError(RuntimeError):
    """Raised when a selected band has no honest input denominator."""


@dataclass(frozen=True)
class FixRunRecord:
    set_id: str
    run_name: str
    event_run_id: str
    fix_run_id: str
    intent: str
    intent_source: str
    goal: str
    family: str
    executor: str
    final_acceptance: str
    verdict: str
    assurance: str
    failure_class: str
    duration_seconds: int | None
    source: str
    evidence_dir: Path
    excluded_reason: str = ""

    @property
    def is_full(self) -> bool:
        return (
            self.final_acceptance == "full_success"
            and self.verdict == "full"
            and self.assurance == "full"
        )

    @property
    def claims_full(self) -> bool:
        return (
            self.final_acceptance == "full_success"
            or self.verdict == "full"
            or self.assurance == "full"
        )


@dataclass(frozen=True)
class InvestigationRunRecord:
    set_id: str
    run_name: str
    event_run_id: str
    intent: str
    family: str
    executor: str
    build_commit: str
    assurance: str
    assurance_reason: str
    failure_class: str
    duration_seconds: int | None
    evidence_dir: Path
    events_path: Path
    i1_passed: bool
    i2_executed: bool
    claim_count: int
    matched_claim_count: int
    violation_count: int
    claim_kind_counts: Counter[str]

    @property
    def is_full(self) -> bool:
        return self.assurance == "full"


@dataclass(frozen=True)
class CircleRunRecord:
    set_id: str
    run_name: str
    arm: str
    verdict: str
    reason: str
    circle_path: Path
    events_path: Path
    excluded_reason: str = ""

    @property
    def is_full(self) -> bool:
        return self.verdict == "circle_full"


@dataclass(frozen=True)
class CliRunRecord:
    set_id: str
    run_name: str
    family: str
    executor: str
    harness_status: str
    product_exit: int
    verdict: str
    assurance: str
    c1: str
    c2: str
    c3: str
    c4: str
    failure_class: str
    attribution: str
    duration_seconds: int
    evidence_dir: Path
    evidence_report: Path | None = None
    pack_id: str = "none"
    pack_hash: str = ""
    pack_exposed: bool = False
    directive_round: int = 0
    directive_hash: str = ""
    api_input_tokens: int | None = None
    api_output_tokens: int | None = None
    cost_usd: float | None = None

    @property
    def is_full(self) -> bool:
        return self.verdict in {"full", "full_success"} and self.assurance == "full"

    @property
    def reached_checks(self) -> bool:
        return any(
            status not in {"not_executed", "not_reached"}
            for status in (self.c1, self.c2, self.c3, self.c4)
        )

    @property
    def pack_label(self) -> str:
        if self.pack_id == "none":
            return "none"
        return f"{self.pack_id} / {self.pack_hash}"

    @property
    def directive_label(self) -> str:
        if self.directive_round == 0:
            return "round 0 / none"
        return f"round {self.directive_round} / {self.directive_hash}"

    @property
    def cost_label(self) -> str:
        if self.cost_usd is None:
            return "not recorded"
        return f"${self.cost_usd:.6f}"


@dataclass(frozen=True)
class IngestRunRecord:
    set_id: str
    source_set_id: str
    run_name: str
    family: str
    executor: str
    harness_status: str
    product_exit: int
    verdict: str
    earned_assurance: str
    display_assurance: str
    n1: str
    n2: str
    n3: str
    n4: str
    n5: str
    failure_class: str
    attribution: str
    duration_seconds: int
    evidence_summary: Path
    evidence_report: Path

    @property
    def is_full(self) -> bool:
        return self.earned_assurance == "full" and self.verdict in {
            "complete",
            "full",
            "full_success",
        }

    @property
    def reached_checks(self) -> bool:
        return any(
            status not in {"not_executed", "not_reached"}
            for status in (self.n1, self.n2, self.n3, self.n4, self.n5)
        )


def classify_data_family(goal: str) -> str:
    """Classify a data scenario only from its recorded goal text."""
    normalized = re.sub(r"\s+", "", goal).lower()
    if "前月比" in normalized or "移動平均" in normalized:
        return TIMESERIES
    if (
        "月次×地域" in normalized
        or "月次x地域" in normalized
        or (
            "月次" in normalized
            and "地域" in normalized
            and ("集計" in normalized or "合計" in normalized)
        )
    ):
        return AGGREGATION
    return UNKNOWN


def data_record_in_stable_window(record: DataRunRecord) -> bool:
    window = DATA_FAMILY_STABLE_WINDOWS.get(record.family)
    return window is not None and record.set_id >= window[0]


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
        return SPACE
    if "breakout" in text or "ブロック" in text:
        return BREAKOUT
    if "quiz" in text or "クイズ" in text:
        return QUIZ
    return UNKNOWN


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


def explicit_intent(*mappings: dict[str, Any] | None) -> tuple[bool, str]:
    """Return whether metadata declares an intent and its normalized value."""
    values: list[str] = []
    declared = False
    for mapping in mappings:
        if not isinstance(mapping, dict):
            continue
        for key in ("intent_resolved", "intent"):
            if key not in mapping:
                continue
            declared = True
            raw = mapping[key]
            if isinstance(raw, dict):
                raw = raw.get("value")
            value = str(raw or "").strip().lower()
            if value:
                values.append(value)
    if not declared:
        return False, ""
    unique = set(values)
    if len(unique) == 1 and next(iter(unique)) in KNOWN_INTENTS:
        return True, next(iter(unique))
    return True, "unknown"


def event_intent(path: Path | None) -> tuple[bool, str]:
    """Read the single resolved intent from an event stream."""
    if path is None or not path.exists():
        return False, ""
    values: list[str] = []
    declared = False
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        if not isinstance(event, dict) or event.get("event") != "intent_resolved":
            continue
        declared = True
        values.append(str(event.get("value") or "").strip().lower())
    unique = set(values)
    if declared and len(values) == 1 and unique <= KNOWN_INTENTS:
        return True, values[0]
    if declared:
        return True, "unknown"
    return False, ""


def resolve_intent(
    *,
    metadata: dict[str, Any] | None,
    row: dict[str, Any] | None,
    events_path: Path | None,
    legacy_create: bool,
) -> IntentResolution:
    """Resolve metadata first, events second, and only known history as create."""
    measurement = metadata.get("measurement") if isinstance(metadata, dict) else None
    declared, value = explicit_intent(
        row,
        measurement if isinstance(measurement, dict) else None,
        metadata,
    )
    if declared:
        return IntentResolution(value, "uat-meta")
    declared, value = event_intent(events_path)
    if declared:
        return IntentResolution(value, "events")
    if legacy_create:
        return IntentResolution("create", "legacy-default")
    return IntentResolution("unknown", "unresolved")


def events_path_for_run(
    set_dir: Path,
    run_name: str,
    event_run_id: str = "",
) -> Path | None:
    run_root = set_dir / "artifacts" / run_name / ".anvil" / "runs"
    if event_run_id:
        direct = run_root / event_run_id / "events.jsonl"
        if direct.exists():
            return direct
    matches = sorted(run_root.glob("*/events.jsonl"))
    return matches[0] if len(matches) == 1 else None


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
    except (OSError, ValueError, json.JSONDecodeError):
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
        str(
            row.get("plan_preset")
            or row.get("plan_preset_arg")
            or row.get("preset")
            or ""
        )
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
    intent = resolve_intent(
        metadata=None,
        row=row,
        events_path=events_path_for_run(
            set_dir,
            run_id,
            str(row.get("event_run_id") or row.get("run_id") or ""),
        ),
        legacy_create=True,
    )
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
        intent=intent.value,
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
    elapsed = elapsed_from_summary(
        set_dir / "artifacts" / "attempt-2-pass" / "summary.md"
    )
    full_has_pass = "Interaction evidence: passed" in text
    false_full = ""
    if final_state == "full_success" and not full_has_pass:
        summary = set_dir / "artifacts" / "attempt-2-pass" / "summary.md"
        if summary.exists() and "Interaction evidence: passed" in summary.read_text(
            errors="ignore"
        ):
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
            intent="create",
            false_full_reason=false_full,
        )
    ]


def find_markdown_value(text: str, label: str) -> str:
    match = re.search(rf"- {re.escape(label)}:\s*`?([^`\n]+)`?", text)
    return match.group(1).strip() if match else ""


def discover_records() -> tuple[
    list[RunRecord],
    int,
    int,
    list[str],
    list[SetFilterDiagnostic],
]:
    records: list[RunRecord] = []
    aggregate_row_total = 0
    aggregate_record_total = 0
    scanned_sets: list[str] = []
    diagnostics: list[SetFilterDiagnostic] = []
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
            diagnostics.append(
                SetFilterDiagnostic(
                    set_id=set_dir.name,
                    accepted_rows=len(rows),
                    reason=(
                        f"aggregate.json: adopted {len(rows)} row(s)"
                        if rows
                        else "aggregate.json: no usable result rows"
                    ),
                )
            )
        else:
            report_records = parse_report_only(set_dir)
            records.extend(report_records)
            report = set_dir / "uat-report.md"
            if report_records:
                reason = f"uat-report.md smoke fallback: adopted {len(report_records)} row(s)"
            elif not report.exists():
                reason = "aggregate.json missing; uat-report.md missing"
            else:
                reason = (
                    "aggregate.json missing; uat-report.md lacks required "
                    "'## Smoke Result' heading"
                )
            diagnostics.append(
                SetFilterDiagnostic(
                    set_id=set_dir.name,
                    accepted_rows=len(report_records),
                    reason=reason,
                )
            )
    assert aggregate_record_total == aggregate_row_total, (
        f"aggregate-derived record count {aggregate_record_total} != "
        f"aggregate.json row count {aggregate_row_total}"
    )
    return (
        records,
        aggregate_row_total,
        aggregate_record_total,
        scanned_sets,
        diagnostics,
    )


def profile_set_diagnostics(
    records: list[Any], scanned_sets: list[str]
) -> list[SetFilterDiagnostic]:
    row_counts = Counter(record.set_id for record in records)
    return [
        SetFilterDiagnostic(
            set_id=set_id,
            accepted_rows=row_counts[set_id],
            reason=(
                f"selected profile: adopted {row_counts[set_id]} row(s)"
                if row_counts[set_id]
                else "selected profile: no rows adopted"
            ),
        )
        for set_id in scanned_sets
    ]


def require_nonempty_aggregation(
    profile: str,
    records: list[Any],
    diagnostics: list[SetFilterDiagnostic],
) -> None:
    adopted_sets = sum(diagnostic.accepted_rows > 0 for diagnostic in diagnostics)
    failures: list[str] = []
    if not records:
        failures.append("aggregation result has 0 rows")
    if adopted_sets == 0:
        failures.append(f"profile {profile!r} adopted 0 sets")
    if not failures:
        return

    lines = [
        f"band generation aborted for profile {profile!r}: {'; '.join(failures)}",
        "Detected sets and filter reasons:",
    ]
    if diagnostics:
        lines.extend(
            f"- {diagnostic.set_id}: {diagnostic.reason}" for diagnostic in diagnostics
        )
    else:
        lines.append("- <none>: no candidate sets were detected")
    raise EmptyAggregationError("\n".join(lines))


def pct(num: int, den: int) -> str:
    if den == 0:
        return "0%"
    return f"{round(num * 100 / den)}%"


def ingest_pct(num: int, den: int) -> str:
    if den == 0:
        return "0%"
    value = f"{num * 100 / den:.1f}".rstrip("0").rstrip(".")
    return f"{value}%"


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
        counts[(rec.scenario, rec.executor or "unknown")][rec.final_state] += 1
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


def full_meaning_label(profile: str) -> str:
    return f"- Full meaning label: {FULL_MEANING_LABELS[profile]}"


def build_summary(
    records: list[RunRecord], aggregate_row_total: int, scanned_sets: list[str]
) -> str:
    included = [rec for rec in records if not rec.excluded_reason]
    excluded = [rec for rec in records if rec.excluded_reason]
    unknowns = [rec for rec in included if rec.scenario == "unknown"]
    false_full = [rec for rec in included if rec.false_full_reason]
    source_counts = Counter(rec.source for rec in records)
    planner_counts = Counter(rec.planner or "unknown" for rec in included)
    lines: list[str] = []
    lines.append("# Next.js Create Capability Band Summary")
    lines.append("")
    lines.append(
        f"> 出自注記: 本バンドの入力{NEXTJS_ARCHIVED_INPUT_SET_COUNT}セットは"
        "移行前計測に由来し、現リポジトリからの再生成は現在未対応"
        f"（[analysis.md]({NEXTJS_PROVENANCE_ANALYSIS})参照）。"
    )
    lines.append("")
    lines.append(full_meaning_label("nextjs"))
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
        rows.append(
            [
                scenario,
                str(counter["full_success"]),
                str(counter["partial"]),
                str(counter["incomplete"]),
                str(counter["failed"]),
                str(den),
                f"{pct(full, den)}{note}",
            ]
        )
    lines.extend(
        table(
            ["Scenario", "full", "partial", "incomplete", "failed", "n", "full rate"],
            rows,
        )
    )
    lines.append("")
    lines.append("## Scenario x Executor")
    rows = []
    for (scenario, executor), counter in sorted(executor_counts(included).items()):
        den = sum(counter.values())
        full = counter["full_success"]
        note = " n<10" if den < 10 else ""
        rows.append(
            [scenario, executor, str(full), str(den), f"{pct(full, den)}{note}"]
        )
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
        rows.append(
            [
                "all",
                str(len(all_full)),
                time_label(min(all_full)),
                time_label(median(all_full)),
                time_label(max(all_full)),
            ]
        )
    for scenario in sorted(full_by_scenario):
        values = full_by_scenario[scenario]
        rows.append(
            [
                scenario,
                str(len(values)),
                time_label(min(values)),
                time_label(median(values)),
                time_label(max(values)),
            ]
        )
    lines.extend(table(["Scope", "full runs", "min", "median", "max"], rows))
    lines.append("")
    lines.append("## Excluded and Unknown Runs")
    if excluded:
        rows = [
            [rec.set_id, rec.run_id, rec.scenario, rec.excluded_reason]
            for rec in excluded
        ]
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
        rows = [
            [rec.set_id, rec.run_id, rec.scenario, rec.false_full_reason]
            for rec in false_full
        ]
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
                goal=DATA_AGGREGATION_GOAL,
                family=classify_data_family(DATA_AGGREGATION_GOAL),
                final_acceptance=final_acceptance,
                assurance=assurance,
                failure_class=failure_class,
                duration_seconds=duration_seconds,
                source=source,
                intent="create",
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
    goal: str,
    metadata: dict[str, Any],
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
    intent = resolve_intent(
        metadata=metadata,
        row=row,
        events_path=events_path_for_run(
            set_dir,
            run_name,
            str(row.get("run_id") or row.get("event_run_id") or ""),
        ),
        legacy_create=True,
    )
    return DataRunRecord(
        set_id=set_id,
        record_dir=set_dir.name,
        run_name=run_name,
        planner=planner,
        executor=str(row.get("executor") or row.get("model") or "unknown"),
        preset=str(row.get("preset") or row.get("plan_preset") or "unknown"),
        goal=goal,
        family=classify_data_family(goal),
        final_acceptance=final_acceptance,
        assurance=assurance,
        failure_class=failure_class or final_acceptance,
        duration_seconds=duration_seconds,
        source=f"{set_dir.name}/uat-meta.json",
        intent=intent.value,
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
        goal = str(measurement.get("goal") or "")
        scanned_sets.add(set_id)
        for row in runs:
            assert isinstance(row, dict), f"non-object run in {meta_path}"
            meta_records.append(
                data_record_from_meta(set_dir, set_id, planner, goal, data, row)
            )
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


def data_band_state(record: DataRunRecord) -> str:
    if record.is_full:
        return "full"
    if record.assurance in {"partial", "static"}:
        return "partial_static"
    return "failed"


def data_family_rows(records: list[DataRunRecord]) -> list[list[str]]:
    counts: dict[str, Counter[str]] = defaultdict(Counter)
    for record in records:
        counts[record.family][data_band_state(record)] += 1
    rows: list[list[str]] = []
    for family in DATA_FAMILIES:
        counter = counts[family]
        denominator = sum(counter.values())
        rows.append(
            [
                family,
                str(counter["full"]),
                str(counter["partial_static"]),
                str(counter["failed"]),
                str(denominator),
                pct(counter["full"], denominator),
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
            [
                "Family",
                "full",
                "partial+static",
                "failed",
                "denominator",
                "full rate",
            ],
            data_family_rows(records),
        )
    )
    lines.append("")
    lines.append("### Executor × preset full rates")
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
    stable = [record for record in included if data_record_in_stable_window(record)]
    unknown_family = [record for record in included if record.family == "unknown"]
    assert len(records) == scanned_run_count
    assert len(included) + len(excluded) == scanned_run_count
    assert stable, "mechanism-stable data window is empty"
    assert all(record.family in DATA_FAMILIES for record in records)

    lines: list[str] = [
        "# Data × Create Capability Band Summary",
        "",
        "<!-- Generated by band_aggregate.py --profile data. Do not edit by hand. -->",
        "",
        full_meaning_label("data"),
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
        (
            "Assurance truth follows B-2j: final acceptance and "
            "`evidence/data-assurance.json` are authoritative for full; historical "
            "terminal projection fields are not read. Non-full levels come from the "
            "campaign's evidence-audited `uat-meta.json` or frozen pre-uat-meta audit row."
        ),
        "",
        (
            "Family is classified only from the recorded goal: `前月比` or `移動平均` "
            "selects `timeseries`; a monthly-by-region aggregation goal selects "
            "`aggregation`; anything else remains included in Window A as `unknown`."
        ),
        "",
        (
            "The frozen pre-uat-meta index is code-owned input for UAT #1 and M-4; "
            "it preserves rows whose original aggregate files predate repository-managed "
            "`uat-meta.json` and carries the aggregation goal recorded by those campaigns. "
            "New and mixed campaigns are discovered from every "
            "`workspace/management/runs/uat-*/uat-meta.json` whose measurement profile is data."
        ),
        "",
    ]
    append_data_window(
        lines,
        "Window A — all history",
        "UAT #1 through #9, including the machine-defect era. Invalid or discarded "
        "measurements remain visible below but are outside this denominator.",
        included,
    )
    append_data_window(
        lines,
        "Window B — family-specific mechanism-stable",
        "Family-specific fixed-code baselines: aggregation starts at "
        f"`{DATA_FAMILY_STABLE_WINDOWS['aggregation'][0]}` (B-2i code HEAD "
        f"`{DATA_FAMILY_STABLE_WINDOWS['aggregation'][1]}`); timeseries starts at "
        f"`{DATA_FAMILY_STABLE_WINDOWS['timeseries'][0]}` (B-2k code HEAD "
        f"`{DATA_FAMILY_STABLE_WINDOWS['timeseries'][1]}`). `unknown` has no "
        "mechanism-stable threshold and therefore remains an explicit zero row here "
        "while any such records stay visible in Window A and the ledger.",
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
            record.family,
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
            ["Family", "Set", "Run", "Executor", "Preset", "Duration"],
            duration_rows,
        )
    )
    if duration_values:
        lines.extend(
            [
                "",
                (
                    f"- n=`{len(duration_values)}`; min=`{min(duration_values)}s`; "
                    f"median=`{median(duration_values)}s`; max=`{max(duration_values)}s`."
                ),
            ]
        )
    family_duration_rows: list[list[str]] = []
    for family in DATA_FAMILIES:
        values = [
            record.duration_seconds
            for record in full_records
            if record.family == family and record.duration_seconds is not None
        ]
        family_duration_rows.append(
            [
                family,
                str(len(values)),
                f"{min(values)}s" if values else "N/A",
                f"{median(values)}s" if values else "N/A",
                f"{max(values)}s" if values else "N/A",
            ]
        )
    lines.extend(["", "### Full-duration summary by family", ""])
    lines.extend(
        table(
            ["Family", "full runs", "min", "median", "max"],
            family_duration_rows,
        )
    )

    lines.extend(["", "## Excluded rows", ""])
    lines.extend(
        table(
            ["Family", "Set", "Run", "Final acceptance", "Failure class", "Reason"],
            [
                [
                    record.family,
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
            (
                "`uat-test0714-m4-002` is discarded for operator model-ID substitution. "
                "`uat-test0714-m4-004` is outside the denominator because cargo-test "
                "preflight was not green and the campaign was interrupted before four of "
                "its five data rows completed; no interrupted result is inferred."
            ),
            "",
            "## Unknown family records",
            "",
        ]
    )
    if unknown_family:
        lines.extend(
            table(
                ["Set", "Run", "Goal", "Source"],
                [
                    [record.set_id, record.run_name, record.goal, record.source]
                    for record in unknown_family
                ],
            )
        )
        lines.extend(
            [
                "",
                (
                    "These records remain in the Window A denominator and per-run ledger; "
                    "they are not assigned to a Window B baseline."
                ),
            ]
        )
    else:
        lines.append("- Unknown family records: `0`")
    lines.extend(
        [
            "",
            "## Per-run ledger",
            "",
        ]
    )
    ledger_rows: list[list[str]] = []
    for record in sorted(records, key=lambda item: (item.set_id, item.run_name)):
        if record.excluded_reason:
            window = "excluded"
        elif data_record_in_stable_window(record):
            window = "A+B"
        else:
            window = "A"
        ledger_rows.append(
            [
                record.set_id,
                record.record_dir,
                record.run_name,
                record.family,
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
                "Family",
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


def classify_fix_family(run_name: str, goal: str) -> str:
    """Classify fix families from the recorded run name, then its goal."""
    run_text = re.sub(r"\s+", "", run_name).lower()
    goal_text = re.sub(r"\s+", "", goal).lower()
    compile_tokens = ("compile", "build", "コンパイル", "ビルド")
    hook_tokens = (
        "hook",
        "data-anvil-action",
        "data-anvil-state",
        "restart",
        "リスタート",
        "契約フック",
    )
    if any(token in run_text for token in compile_tokens):
        return COMPILE_ERROR_FIX
    if any(token in run_text for token in hook_tokens):
        return CONTRACT_HOOK_FIX
    # Named contract attributes are more specific than a goal's incidental
    # request to run a build as regression coverage.
    if any(token in goal_text for token in hook_tokens):
        return CONTRACT_HOOK_FIX
    if any(token in goal_text for token in compile_tokens):
        return COMPILE_ERROR_FIX
    return UNKNOWN


def discover_fix_records() -> tuple[list[FixRunRecord], list[str]]:
    records: list[FixRunRecord] = []
    scanned_sets: list[str] = []
    for set_id in FIX_WINDOW_SETS:
        set_dir = RUNS_DIR / set_id
        matrix_path = set_dir / "artifacts" / "analysis" / "run-matrix.json"
        assert matrix_path.exists(), f"missing fix run matrix: {matrix_path}"
        matrix = json.loads(matrix_path.read_text(encoding="utf-8"))
        assert isinstance(matrix, list), f"fix run matrix is not a list: {matrix_path}"
        metadata = read_json_dict(set_dir / "uat-meta.json")
        measurement = metadata.get("measurement") if metadata is not None else None
        campaign_goal = (
            str(measurement.get("goal") or "") if isinstance(measurement, dict) else ""
        )
        scanned_sets.append(set_id)
        for row in matrix:
            assert isinstance(row, dict), f"non-object fix row in {matrix_path}"
            run_name = str(row.get("run") or row.get("name") or "unknown")
            event_run_id = str(row.get("event_run_id") or "")
            fix_run_id = str(row.get("fix_run_id") or "")
            assert event_run_id, f"missing event run id: {set_id}/{run_name}"
            assert fix_run_id, f"missing fix run id: {set_id}/{run_name}"
            events_path = events_path_for_run(set_dir, run_name, event_run_id)
            intent = resolve_intent(
                metadata=metadata,
                row=row,
                events_path=events_path,
                legacy_create=False,
            )
            goal = str(row.get("goal") or campaign_goal)
            family = classify_fix_family(run_name, goal)
            assert family in FIX_FAMILIES
            reported_family = str(row.get("family") or "")
            expected_reported_family = {
                "compile_error_fix": "compile",
                "contract_hook_fix": "hook",
            }.get(family)
            if expected_reported_family is not None and reported_family:
                assert reported_family == expected_reported_family, (
                    f"fix family mismatch for {set_id}/{run_name}: "
                    f"classified={family}, reported={reported_family}"
                )
            duration = row.get("time_profile_total_ms")
            duration_seconds = (
                round(float(duration) / 1000)
                if isinstance(duration, (int, float))
                else None
            )
            records.append(
                FixRunRecord(
                    set_id=set_id,
                    run_name=run_name,
                    event_run_id=event_run_id,
                    fix_run_id=fix_run_id,
                    intent=intent.value,
                    intent_source=intent.source,
                    goal=goal,
                    family=family,
                    executor=str(row.get("model") or row.get("executor") or "unknown"),
                    final_acceptance=str(
                        row.get("final_acceptance_status")
                        or row.get("final_acceptance")
                        or "failed"
                    ).lower(),
                    verdict=str(row.get("verdict") or "failed").lower(),
                    assurance=str(row.get("assurance") or "failed").lower(),
                    failure_class=str(row.get("failure_class") or ""),
                    duration_seconds=duration_seconds,
                    source=f"{set_id}/artifacts/analysis/run-matrix.json",
                    evidence_dir=set_dir / "artifacts" / run_name / "evidence",
                    excluded_reason=FIX_ENVIRONMENT_HOLDS.get((set_id, run_name), ""),
                )
            )
    bench_meta = read_json_dict(RUNS_DIR / FIX_BENCH_SET / "uat-meta.json")
    assert bench_meta is not None, f"missing bench metadata: {FIX_BENCH_SET}"
    bench_runs = bench_meta.get("runs")
    assert isinstance(bench_runs, list), f"bench runs is not a list: {FIX_BENCH_SET}"
    for row in bench_runs:
        assert isinstance(row, dict)
        run_name = str(row.get("name") or "")
        interrupted = str(row.get("status") or "") == INTERRUPTED_ENVIRONMENT
        artifact = RUNS_DIR / FIX_BENCH_SET / "artifacts" / run_name
        records.append(
            FixRunRecord(
                set_id=FIX_BENCH_SET,
                run_name=run_name,
                event_run_id=run_name,
                fix_run_id=run_name,
                intent="fix",
                intent_source="uat-meta",
                goal=str(row.get("goal") or ""),
                family="compile_error_fix"
                if row.get("goal") == "pipe"
                else "contract_hook_fix",
                executor=str(row.get("executor") or "unknown"),
                final_acceptance="failed",
                verdict="interrupted" if interrupted else "failed",
                assurance="static" if interrupted else "failed",
                failure_class="environment_interrupted"
                if interrupted
                else "bench_product_exit",
                duration_seconds=row.get("duration_seconds"),
                source=f"{FIX_BENCH_SET}/uat-meta.json",
                evidence_dir=artifact / ".anvil" / "evidence",
                excluded_reason=(
                    f"{INTERRUPTED_ENVIRONMENT}: run non-consuming; "
                    "two attempts were environment-interrupted"
                )
                if interrupted
                else "",
            )
        )
    assert len(records) == FIX_EXPECTED_RUNS, (
        f"fix output rows {len(records)} != expected {FIX_EXPECTED_RUNS}"
    )
    keys = [(record.set_id, record.run_name) for record in records]
    assert len(keys) == len(set(keys)), "duplicate fix set/run rows discovered"
    return records, scanned_sets


def fix_evidence_path(record: FixRunRecord, suffix: str) -> Path:
    return record.evidence_dir / f"fix-{record.fix_run_id}-{suffix}.json"


def require_fix_probe(
    record: FixRunRecord,
    probe: dict[str, Any] | None,
    *,
    requirement_id: str,
    stage: str,
    expected: str,
    outcome: str,
) -> tuple[str, int]:
    label = f"{record.set_id}/{record.run_name}/{requirement_id}"
    assert probe is not None, f"missing fix evidence object: {label}"
    assert probe.get("intent") == "fix", f"wrong evidence intent: {label}"
    assert probe.get("run_id") == record.fix_run_id, f"wrong evidence run id: {label}"
    assert probe.get("requirement_id") == requirement_id, (
        f"wrong requirement id: {label}"
    )
    assert probe.get("stage") == stage, f"wrong evidence stage: {label}"
    assert probe.get("expected") == expected, f"wrong expected polarity: {label}"
    assert probe.get("executed") is True, f"unexecuted fix evidence: {label}"
    assert probe.get("outcome") == outcome, f"wrong fix evidence outcome: {label}"
    lineage = str(probe.get("lineage") or "")
    epoch = probe.get("epoch")
    assert lineage, f"missing fix evidence lineage: {label}"
    assert isinstance(epoch, int), f"missing fix evidence epoch: {label}"
    return lineage, epoch


def assert_full_fix_record_evidence(record: FixRunRecord) -> None:
    label = f"{record.set_id}/{record.run_name}"
    assert record.intent == "fix", f"full fix row has wrong intent: {label}"
    adjudication_path = fix_evidence_path(record, "adjudication")
    before_path = fix_evidence_path(record, "before")
    after_path = fix_evidence_path(record, "after")
    adjudication = read_json_dict(adjudication_path)
    before = read_json_dict(before_path)
    after = read_json_dict(after_path)
    assert adjudication is not None, f"missing fix adjudication evidence: {label}"
    assert before is not None, f"missing F1 before evidence: {label}"
    assert after is not None, f"missing F2 after evidence: {label}"
    assert adjudication.get("intent") == "fix", f"wrong adjudication intent: {label}"
    assert adjudication.get("run_id") == record.fix_run_id, (
        f"wrong adjudication run id: {label}"
    )
    result = adjudication.get("adjudication")
    assert isinstance(result, dict), f"missing adjudication result: {label}"
    assert result.get("assurance") == "full", f"non-full adjudication: {label}"
    statuses = result.get("requirement_statuses")
    assert statuses == {
        "after_passes": "passed",
        "before_fails": "passed",
        "no_regression": "passed",
    }, f"fix requirement status mismatch: {label}"
    evidence = adjudication.get("evidence")
    assert isinstance(evidence, dict), f"missing adjudicated evidence: {label}"
    assert evidence.get("before") == before, f"F1 evidence projection mismatch: {label}"
    assert evidence.get("after") == after, f"F2 evidence projection mismatch: {label}"
    before_lineage, before_epoch = require_fix_probe(
        record,
        before,
        requirement_id="before_fails",
        stage="before",
        expected="failure",
        outcome="failure",
    )
    after_lineage, after_epoch = require_fix_probe(
        record,
        after,
        requirement_id="after_passes",
        stage="after",
        expected="success",
        outcome="success",
    )
    assert before_lineage == after_lineage, f"F1/F2 lineage mismatch: {label}"
    assert after_epoch > before_epoch, f"F2 epoch is not newer than F1: {label}"

    bound_ids = evidence.get("bound_regression_ids")
    bound_lineages = evidence.get("bound_regression_lineages")
    regressions = evidence.get("regressions")
    assert isinstance(bound_ids, list) and bound_ids, (
        f"missing bound regression set: {label}"
    )
    assert isinstance(bound_lineages, dict), f"missing regression lineages: {label}"
    assert isinstance(regressions, list), f"missing F3 regression evidence: {label}"
    assert len(regressions) == len(bound_ids), f"shrunk regression evidence: {label}"
    assert {str(item.get("binding_id") or "") for item in regressions} == set(
        bound_ids
    ), f"regression binding set mismatch: {label}"
    regression_epochs: list[int] = []
    for regression in regressions:
        assert isinstance(regression, dict), f"invalid regression evidence: {label}"
        binding_id = str(regression.get("binding_id") or "")
        standalone = read_json_dict(
            fix_evidence_path(record, f"regression-{binding_id}")
        )
        assert standalone is not None, (
            f"missing F3 regression evidence: {label}/{binding_id}"
        )
        assert standalone == regression, (
            f"F3 evidence projection mismatch: {label}/{binding_id}"
        )
        lineage, epoch = require_fix_probe(
            record,
            regression,
            requirement_id="no_regression",
            stage="after",
            expected="success",
            outcome="success",
        )
        assert bound_lineages.get(binding_id) == lineage, (
            f"regression lineage mismatch: {label}/{binding_id}"
        )
        assert epoch > after_epoch, f"F3 epoch is not newer than F2: {label}"
        regression_epochs.append(epoch)
    assert len(regression_epochs) == len(set(regression_epochs)), (
        f"duplicate F3 epochs: {label}"
    )


def assert_full_fix_evidence(records: list[FixRunRecord]) -> int:
    verified = 0
    for record in records:
        if not record.claims_full:
            continue
        assert record.is_full, (
            f"inconsistent full projection for {record.set_id}/{record.run_name}: "
            f"final={record.final_acceptance}, verdict={record.verdict}, "
            f"assurance={record.assurance}"
        )
        assert_full_fix_record_evidence(record)
        verified += 1
    return verified


def fix_rate_rows(records: list[FixRunRecord]) -> list[list[str]]:
    counts: dict[tuple[str, str, str], Counter[str]] = defaultdict(Counter)
    for record in records:
        if record.is_full:
            state = "full"
        else:
            assert record.verdict == "failed" and record.assurance == "failed", (
                f"unsupported fix band state for {record.set_id}/{record.run_name}: "
                f"{record.verdict}/{record.assurance}"
            )
            state = "failed"
        counts[(record.intent, record.family, record.executor)][state] += 1
    rows: list[list[str]] = []
    for (intent, family, executor), counter in sorted(counts.items()):
        full = counter["full"]
        failed = counter["failed"]
        denominator = full + failed
        rows.append(
            [
                intent,
                family,
                executor,
                str(full),
                str(failed),
                str(denominator),
                pct(full, denominator),
            ]
        )
    return rows


def append_fix_window(
    lines: list[str],
    title: str,
    definition: str,
    records: list[FixRunRecord],
) -> None:
    lines.extend(
        [
            f"## {title}",
            "",
            definition,
            "",
            f"- Denominator: `{len(records)}`",
            f"- Full: `{sum(record.is_full for record in records)}`",
            f"- Failed: `{sum(not record.is_full for record in records)}`",
            "",
        ]
    )
    lines.extend(
        table(
            [
                "Intent",
                "Family",
                "Executor",
                "full",
                "failed",
                "denominator",
                "full rate",
            ],
            fix_rate_rows(records),
        )
    )
    lines.append("")


def fix_full_chain(record: FixRunRecord) -> str:
    adjudication = read_json_dict(fix_evidence_path(record, "adjudication"))
    assert adjudication is not None
    evidence = adjudication["evidence"]
    before = evidence["before"]
    after = evidence["after"]
    regression_epochs = ", ".join(
        f"{item['binding_id']}@{item['epoch']}" for item in evidence["regressions"]
    )
    return (
        f"F1 `{before['binding_id']}` failure@{before['epoch']} -> same-lineage "
        f"F2 success@{after['epoch']} -> F3 `{regression_epochs}` success"
    )


def build_fix_summary(
    records: list[FixRunRecord],
    scanned_sets: list[str],
    full_evidence_verified: int,
) -> str:
    window_a_records = [
        record for record in records if record.set_id in FIX_WINDOW_SETS
    ]
    window_b_records = [record for record in records if record.set_id == FIX_BENCH_SET]
    official = [record for record in window_a_records if not record.excluded_reason]
    excluded = [record for record in records if record.excluded_reason]
    historical_excluded = [
        record for record in window_a_records if record.excluded_reason
    ]
    unknown_intent = [record for record in records if record.intent == "unknown"]
    unknown_family = [record for record in records if record.family == "unknown"]
    intent_sources = Counter(record.intent_source for record in records)
    assert len(records) == FIX_EXPECTED_RUNS
    assert len(window_a_records) == 24
    assert len(official) == 22, (
        f"fix official denominator is {len(official)}, expected 22"
    )
    assert len(excluded) == 3, f"fix exclusions are {len(excluded)}, expected 3"
    assert full_evidence_verified == sum(record.claims_full for record in records)

    lines: list[str] = [
        "# Fix × Next.js Capability Band Summary",
        "",
        "<!-- Generated by band_aggregate.py --profile fix. Do not edit by hand. -->",
        "",
        full_meaning_label("fix"),
        f"- Scanned D-1 measurement sets: `{len(scanned_sets)}`",
        f"- Scanned fix run rows: `{len(records)}`",
        f"- Intent resolution sources: `{dict(sorted(intent_sources.items()))}`",
        f"- Window A raw denominator: `{len(window_a_records)}`",
        f"- Window A official denominator: `{len(official)}`",
        f"- Environment-held exclusions: `{len(excluded)}`",
        f"- Full rows with F1-F3 evidence verified: `{full_evidence_verified}`",
        "- False-full evidence gaps: `0` (generation aborts on any gap)",
        "",
        (
            "The intent column resolves repository-managed `uat-meta.json` first and "
            "the run's single `intent_resolved` event second. Only known historical "
            "create campaigns may use the `create` legacy default; an unresolved or "
            "conflicting modern record remains `unknown` and is never folded into "
            "create or fix."
        ),
        "",
        (
            "Fix family is classified from the recorded run name and goal. Build or "
            "compile language maps to `compile_error_fix`; contract attribute, restart, "
            "or hook language maps to `contract_hook_fix`; unmatched records remain "
            "`unknown`."
        ),
        "",
    ]
    append_fix_window(
        lines,
        "Window A — all D-1 history (raw)",
        "`uat-test0717-fix-001` through `uat-test0717-fix-004`; all 24 observed "
        "runs are retained, including the two pre-FIX-1 environment-held rows.",
        window_a_records,
    )
    append_fix_window(
        lines,
        "Window A — official denominator",
        "The same four campaigns after excluding exactly two #1 rows whose inherited "
        "`NODE_ENV=production` skipped devDependencies. FIX-1 removed that host "
        "contamination mechanism before #2; no model outcome is inferred for the "
        "excluded rows.",
        official,
    )
    lines.extend(
        ["## Environment-held rows excluded from the official denominator", ""]
    )
    lines.extend(
        table(
            ["Set", "Run", "Intent", "Family", "Executor", "Reason"],
            [
                [
                    record.set_id,
                    record.run_name,
                    record.intent,
                    record.family,
                    record.executor,
                    record.excluded_reason,
                ]
                for record in historical_excluded
            ],
        )
    )
    lines.extend(
        [
            "",
            "## Window B — post-FIX-5",
            "",
            f"- Baseline HEAD: `{FIX_WINDOW_B_BASELINE_HEAD}` (FIX-5)",
            "- Definition: measurements beginning with the first campaign after FIX-5.",
            f"- Denominator: `{len([r for r in window_b_records if not r.excluded_reason])}`",
            f"- Full: `{sum(r.is_full for r in window_b_records if not r.excluded_reason)}`",
            f"- Failed: `{sum(not r.is_full for r in window_b_records if not r.excluded_reason)}`",
            "",
        ]
    )
    lines.extend(
        table(
            [
                "Intent",
                "Family",
                "Executor",
                "full",
                "failed",
                "denominator",
                "full rate",
            ],
            fix_rate_rows([r for r in window_b_records if not r.excluded_reason]),
        )
    )
    lines.extend(["", "## Window B rows excluded from consumption", ""])
    lines.extend(
        table(
            ["Set", "Run", "Reason"],
            [
                [r.set_id, r.run_name, r.excluded_reason]
                for r in window_b_records
                if r.excluded_reason
            ],
        )
    )
    lines.extend(["", "## First full and F-evidence chain", ""])
    full_records = [record for record in records if record.is_full]
    for record in full_records:
        lines.append(
            f"- `{record.set_id}` / `{record.run_name}`: {fix_full_chain(record)}."
        )
    lines.extend(
        [
            "",
            "## Unknown classifications",
            "",
            f"- Unknown intent records: `{len(unknown_intent)}`",
            f"- Unknown family records: `{len(unknown_family)}`",
            "",
        ]
    )
    if unknown_intent or unknown_family:
        lines.extend(
            table(
                ["Set", "Run", "Intent", "Family", "Source"],
                [
                    [
                        record.set_id,
                        record.run_name,
                        record.intent,
                        record.family,
                        record.source,
                    ]
                    for record in records
                    if record.intent == "unknown" or record.family == "unknown"
                ],
            )
        )
        lines.append("")
    lines.extend(["## Per-run ledger", ""])
    lines.extend(
        table(
            [
                "Set",
                "Run",
                "Intent",
                "Intent source",
                "Family",
                "Executor",
                "Final acceptance",
                "Verdict",
                "Assurance",
                "Failure class",
                "Duration",
                "Window",
            ],
            [
                [
                    record.set_id,
                    record.run_name,
                    record.intent,
                    record.intent_source,
                    record.family,
                    record.executor,
                    record.final_acceptance,
                    record.verdict,
                    record.assurance,
                    record.failure_class or "completed",
                    f"{record.duration_seconds}s"
                    if record.duration_seconds is not None
                    else "unknown",
                    "A raw only" if record.excluded_reason else "A raw+official",
                ]
                for record in sorted(
                    records, key=lambda item: (item.set_id, item.run_name)
                )
            ],
        )
    )
    lines.extend(["", "## Source sets", ""])
    lines.extend(f"- `{set_id}`" for set_id in scanned_sets)
    lines.append("")
    return "\n".join(lines)


def read_json_events(path: Path) -> list[dict[str, Any]]:
    """Read a complete event stream; band inputs may not silently lose rows."""
    events: list[dict[str, Any]] = []
    for line_number, line in enumerate(
        path.read_text(encoding="utf-8").splitlines(), start=1
    ):
        try:
            event = json.loads(line)
        except json.JSONDecodeError as error:
            raise AssertionError(
                f"invalid JSON event at {path}:{line_number}: {error}"
            ) from error
        assert isinstance(event, dict), f"non-object event at {path}:{line_number}"
        events.append(event)
    return events


def only_event(
    events: list[dict[str, Any]], event_name: str, label: str
) -> dict[str, Any]:
    matches = [event for event in events if event.get("event") == event_name]
    assert len(matches) == 1, (
        f"expected one {event_name} event for {label}, found {len(matches)}"
    )
    return matches[0]


def classify_investigation_family(run_name: str) -> str:
    """Classify the two fixed D-3b UAT families from immutable run names."""
    if "_pipe_" in run_name:
        return PIPE
    if "_schema_" in run_name:
        return SCHEMA
    return UNKNOWN


def investigation_failure_class(stop_reason: str) -> str:
    lowered = stop_reason.lower()
    if "diagnosis_unbound" in lowered:
        return "diagnosis_unbound"
    if "model_stagnation:read_only_loop" in lowered:
        return "model_stagnation:read_only_loop"
    if "artifact recovery exhausted" in lowered:
        return "artifact_recovery_exhausted"
    if "path does not exist" in lowered:
        return "missing_artifact_reference"
    return "investigation_incomplete"


def validate_investigation_evidence(
    evidence_dir: Path,
    events: list[dict[str, Any]],
    label: str,
) -> tuple[str, str, bool, int, int, int, Counter[str]]:
    """Validate I1/I2 evidence and return contract-derived assurance fields."""
    intent = only_event(events, "intent_resolved", label)
    assert intent.get("value") == "investigate", f"wrong intent event for {label}"
    synthesized = only_event(events, "investigation_plan_synthesized", label)
    assert synthesized.get("profile") == "data", (
        f"wrong investigation synthesis profile for {label}"
    )
    assert synthesized.get("phase_count") == 3, (
        f"wrong investigation synthesis phase count for {label}"
    )

    i1_path = evidence_dir / "investigation-run.json"
    i1 = read_json_dict(i1_path)
    assert i1 is not None, f"missing I1 evidence: {label}"
    assert i1.get("intent") == "investigate", f"wrong I1 intent: {label}"
    assert i1.get("requirement_id") == "reproducer_fails", (
        f"wrong I1 requirement: {label}"
    )
    assert i1.get("stage") == "diagnosis", f"wrong I1 stage: {label}"
    assert i1.get("expected") == "failure", f"wrong I1 polarity: {label}"
    assert i1.get("executed") is True, f"I1 was not executed: {label}"
    assert i1.get("outcome") == "failure", f"I1 did not fail: {label}"
    assert isinstance(i1.get("epoch"), int), f"missing I1 epoch: {label}"

    binding_path = evidence_dir / "investigation-binding.json"
    binding = read_json_dict(binding_path)
    adjudications = [
        event for event in events if event.get("event") == "investigation_adjudicated"
    ]
    if binding is None:
        assert not adjudications, f"adjudication exists without I2 evidence: {label}"
        return "failed", "investigation_incomplete", False, 0, 0, 0, Counter()

    assert binding.get("intent") == "investigate", f"wrong I2 intent: {label}"
    assert binding.get("requirement_id") == "diagnosis_bound", (
        f"wrong I2 requirement: {label}"
    )
    claims = binding.get("claims")
    assert isinstance(claims, list), f"I2 claims are not a list: {label}"
    matched = 0
    kinds: Counter[str] = Counter()
    for index, claim in enumerate(claims):
        assert isinstance(claim, dict), f"invalid I2 claim {index}: {label}"
        kind = str(claim.get("kind") or "")
        assert kind in {"error_quote", "file_line", "code_snippet"}, (
            f"unknown I2 claim kind {kind!r}: {label}"
        )
        kinds[kind] += 1
        claim_matched = claim.get("matched")
        assert isinstance(claim_matched, bool), (
            f"I2 claim lacks match result at {index}: {label}"
        )
        matched += int(claim_matched)

    violations = len(claims) - matched
    if not claims:
        expected_level = "partial"
        expected_reason = "diagnosis_claims_absent"
    elif violations:
        expected_level = "failed"
        expected_reason = "diagnosis_unbound"
    else:
        expected_level = "full"
        expected_reason = ""
    adjudication = only_event(events, "investigation_adjudicated", label)
    assert adjudication.get("assurance_level") == expected_level, (
        f"I2 assurance mismatch for {label}"
    )
    assert str(adjudication.get("assurance_reason") or "") == expected_reason, (
        f"I2 assurance reason mismatch for {label}"
    )
    paths = adjudication.get("evidence_paths")
    assert isinstance(paths, list), f"missing adjudicated evidence paths: {label}"
    assert {
        "evidence/investigation-run.json",
        "evidence/investigation-binding.json",
    } <= set(paths), f"incomplete adjudicated evidence paths: {label}"
    return (
        expected_level,
        expected_reason,
        True,
        len(claims),
        matched,
        violations,
        kinds,
    )


def discover_investigation_records() -> tuple[list[InvestigationRunRecord], list[str]]:
    records: list[InvestigationRunRecord] = []
    for set_id in INVESTIGATION_WINDOW_SETS:
        artifacts_dir = RUNS_DIR / set_id / "artifacts"
        assert artifacts_dir.is_dir(), (
            f"missing investigation artifacts: {artifacts_dir}"
        )
        run_dirs = sorted(path for path in artifacts_dir.glob("inv*") if path.is_dir())
        assert len(run_dirs) == 6, (
            f"investigation set {set_id} has {len(run_dirs)} runs, expected 6"
        )
        for run_dir in run_dirs:
            run_name = run_dir.name
            label = f"{set_id}/{run_name}"
            event_paths = sorted(run_dir.glob(".anvil/runs/*/events.jsonl"))
            assert len(event_paths) == 1, (
                f"expected one investigation event stream for {label}, "
                f"found {len(event_paths)}"
            )
            events_path = event_paths[0]
            events = read_json_events(events_path)
            run_start = only_event(events, "run_start", label)
            run_stop = only_event(events, "run_stop", label)
            time_profile = only_event(events, "time_profile", label)
            preset = only_event(events, "plan_preset_resolved", label)
            assert run_start.get("build_dirty") is False, f"dirty build for {label}"
            assert run_start.get("profile") == "data", f"wrong profile for {label}"
            assert run_start.get("plan_preset") == "profile", (
                f"wrong plan preset for {label}"
            )
            assert preset.get("plan_preset") == "profile", (
                f"wrong resolved preset for {label}"
            )
            assert preset.get("origin") == "default_investigate_data", (
                f"wrong investigation preset origin for {label}"
            )
            assert run_stop.get("status") == "failed", (
                f"unsupported investigation terminal state for {label}"
            )

            build_commit = str(run_start.get("build_commit") or "")
            assert build_commit, f"missing build commit for {label}"
            if set_id == INVESTIGATION_WINDOW_B_SET:
                assert build_commit == INVESTIGATION_WINDOW_B_BASELINE_HEAD, (
                    f"Window B build mismatch for {label}: {build_commit}"
                )
            evidence_dir = run_dir / "evidence"
            (
                assurance,
                assurance_reason,
                i2_executed,
                claim_count,
                matched_claim_count,
                violation_count,
                claim_kind_counts,
            ) = validate_investigation_evidence(evidence_dir, events, label)

            if set_id == INVESTIGATION_WINDOW_B_SET:
                assert run_stop.get("assurance_level") == assurance, (
                    f"post-INV-1 assurance projection mismatch for {label}"
                )
                assert (
                    str(run_stop.get("assurance_reason") or "") == assurance_reason
                ), f"post-INV-1 assurance reason mismatch for {label}"
            profile = time_profile.get("profile")
            assert isinstance(profile, dict), f"missing time profile for {label}"
            total_ms = profile.get("total_ms")
            assert isinstance(total_ms, (int, float)), f"missing duration for {label}"
            family = classify_investigation_family(run_name)
            assert family in INVESTIGATION_FAMILIES, (
                f"unknown investigation family for {label}"
            )
            records.append(
                InvestigationRunRecord(
                    set_id=set_id,
                    run_name=run_name,
                    event_run_id=events_path.parent.name,
                    intent="investigate",
                    family=family,
                    executor=str(run_start.get("model") or "unknown"),
                    build_commit=build_commit,
                    assurance=assurance,
                    assurance_reason=assurance_reason,
                    failure_class=investigation_failure_class(
                        str(run_stop.get("stop_reason") or "")
                    ),
                    duration_seconds=round(float(total_ms) / 1000),
                    evidence_dir=evidence_dir,
                    events_path=events_path,
                    i1_passed=True,
                    i2_executed=i2_executed,
                    claim_count=claim_count,
                    matched_claim_count=matched_claim_count,
                    violation_count=violation_count,
                    claim_kind_counts=claim_kind_counts,
                )
            )
    assert len(records) == INVESTIGATION_EXPECTED_RUNS, (
        f"investigation denominator is {len(records)}, "
        f"expected {INVESTIGATION_EXPECTED_RUNS}"
    )
    assert len({(record.set_id, record.run_name) for record in records}) == len(records)
    return records, list(INVESTIGATION_WINDOW_SETS)


def investigation_rate_rows(records: list[InvestigationRunRecord]) -> list[list[str]]:
    counts: dict[tuple[str, str], Counter[str]] = defaultdict(Counter)
    for record in records:
        state = "full" if record.is_full else "failed"
        counts[(record.family, record.executor)][state] += 1
    rows: list[list[str]] = []
    for (family, executor), counter in sorted(counts.items()):
        full = counter["full"]
        failed = counter["failed"]
        denominator = full + failed
        rows.append(
            [
                family,
                executor,
                str(full),
                str(failed),
                str(denominator),
                pct(full, denominator),
            ]
        )
    return rows


def append_investigation_window(
    lines: list[str], title: str, definition: str, records: list[InvestigationRunRecord]
) -> None:
    lines.extend(
        [
            f"## {title}",
            "",
            definition,
            "",
            f"- Denominator: `{len(records)}`",
            f"- Full: `{sum(record.is_full for record in records)}`",
            f"- Failed: `{sum(not record.is_full for record in records)}`",
            "",
        ]
    )
    lines.extend(
        table(
            ["Family", "Executor", "full", "failed", "denominator", "full rate"],
            investigation_rate_rows(records),
        )
    )
    lines.append("")


def build_investigation_summary(
    records: list[InvestigationRunRecord], scanned_sets: list[str]
) -> str:
    assert len(records) == INVESTIGATION_EXPECTED_RUNS
    window_b = [
        record for record in records if record.set_id == INVESTIGATION_WINDOW_B_SET
    ]
    window_a_records = records
    assert len(window_b) == 6, f"Window B denominator is {len(window_b)}, expected 6"
    kind_counts: Counter[str] = Counter()
    for record in records:
        kind_counts.update(record.claim_kind_counts)
    lines = [
        "# Investigation × Data Capability Band Summary",
        "",
        "<!-- Generated by band_aggregate.py --profile investigation. Do not edit by hand. -->",
        "",
        full_meaning_label("investigation"),
        f"- Scanned investigation sets: `{len(scanned_sets)}`",
        f"- Formal consumed runs: `{len(records)}`",
        "- Exclusions: `0`",
        "- Arm: `profile_synthesis` (default for investigate × data)",
        f"- I1 reproducer evidence passed: `{sum(record.i1_passed for record in records)}/{len(records)}`",
        f"- I2 binding executed: `{sum(record.i2_executed for record in records)}/{len(records)}`",
        f"- I2 claims: `{sum(record.claim_count for record in records)}`",
        f"- I2 matched claims: `{sum(record.matched_claim_count for record in records)}`",
        (
            f"- I2 rejected violations: `{sum(record.violation_count for record in records)}` "
            f"(`code_snippet={kind_counts['code_snippet']}`, "
            f"`error_quote={kind_counts['error_quote']}`, `file_line={kind_counts['file_line']}`)"
        ),
        "- False-full evidence gaps: `0` (generation aborts on any I1/I2/adjudication mismatch)",
        "",
        (
            "Assurance is recomputed from the fixed investigation contract: I1 must be "
            "an executed failing diagnosis-stage reproducer; I2 evidence, when present, "
            "must agree with the single `investigation_adjudicated` event. Pre-INV-1 "
            "summary projection errors are not used as contract assurance."
        ),
        "",
    ]
    append_investigation_window(
        lines,
        "Window A — all investigation history",
        "`uat-test0718-inv-001` plus `uat-test0718-inv-002`; all 12 runs were "
        "formally consumed and none is excluded.",
        window_a_records,
    )
    append_investigation_window(
        lines,
        "Window B — post-INV-1",
        f"Baseline HEAD `{INVESTIGATION_WINDOW_B_BASELINE_HEAD}`; the six "
        "`uat-test0718-inv-002` runs measure projection dispatch and diagnosis "
        "guidance after INV-1.",
        window_b,
    )
    lines.extend(["## Per-run evidence ledger", ""])
    lines.extend(
        table(
            [
                "Set",
                "Run",
                "Family",
                "Executor",
                "Assurance",
                "Reason",
                "I1",
                "I2 claims/matched/violations",
                "Failure class",
                "Duration",
                "Window",
            ],
            [
                [
                    record.set_id,
                    record.run_name,
                    record.family,
                    record.executor,
                    record.assurance,
                    record.assurance_reason,
                    "passed" if record.i1_passed else "failed",
                    (
                        f"{record.claim_count}/{record.matched_claim_count}/"
                        f"{record.violation_count}"
                        if record.i2_executed
                        else "not reached"
                    ),
                    record.failure_class,
                    f"{record.duration_seconds}s",
                    "A+B" if record.set_id == INVESTIGATION_WINDOW_B_SET else "A",
                ]
                for record in sorted(
                    records, key=lambda item: (item.set_id, item.run_name)
                )
            ],
        )
    )
    lines.extend(["", "## Source sets", ""])
    lines.extend(f"- `{set_id}`" for set_id in scanned_sets)
    lines.append("")
    return "\n".join(lines)


def discover_circle_records() -> tuple[list[CircleRunRecord], list[str]]:
    """Load the fixed local circle campaigns without inferring missing verdicts."""
    records: list[CircleRunRecord] = []
    for set_id in CIRCLE_WINDOW_SETS:
        set_dir = RUNS_DIR / set_id
        assert set_dir.is_dir(), f"missing circle set: {set_dir}"
        run_dirs = sorted(path for path in set_dir.glob("run[1-3]") if path.is_dir())
        assert len(run_dirs) == 3, (
            f"circle set {set_id} has {len(run_dirs)} run directories, expected 3"
        )
        for run_dir in run_dirs:
            label = f"{set_id}/{run_dir.name}"
            circle_path = run_dir / "workflow-circle.json"
            events_path = run_dir / "workflow-events.jsonl"
            assert circle_path.is_file(), f"missing workflow-circle.json for {label}"
            assert events_path.is_file(), f"missing workflow events for {label}"
            circle = json.loads(circle_path.read_text(encoding="utf-8"))
            assert isinstance(circle, dict), f"non-object workflow circle for {label}"
            adjudication = only_event(
                read_json_events(events_path), "workflow_adjudicated", label
            )
            verdict = str(circle.get("verdict") or "")
            reason = str(circle.get("reason") or "")
            assert verdict in {"circle_full", "circle_failed"}, (
                f"unsupported workflow verdict {verdict!r} for {label}"
            )
            assert adjudication.get("verdict") == verdict, (
                f"workflow verdict mismatch for {label}"
            )
            if verdict != "circle_full":
                assert str(adjudication.get("reason") or "") == reason, (
                    f"workflow reason mismatch for {label}"
                )
            records.append(
                CircleRunRecord(
                    set_id=set_id,
                    run_name=run_dir.name,
                    arm="elevated" if "circle-elev-" in set_id else "local",
                    verdict=verdict,
                    reason=reason,
                    circle_path=circle_path,
                    events_path=events_path,
                    excluded_reason=CIRCLE_EXCLUSION_REASONS.get(set_id, ""),
                )
            )
    assert len(records) == CIRCLE_EXPECTED_RUNS, (
        f"circle observed run count is {len(records)}, expected {CIRCLE_EXPECTED_RUNS}"
    )
    assert len({(record.set_id, record.run_name) for record in records}) == len(records)
    return records, list(CIRCLE_WINDOW_SETS)


def circle_rate_rows(records: list[CircleRunRecord]) -> list[list[str]]:
    counts: dict[str, Counter[str]] = defaultdict(Counter)
    for record in records:
        if record.excluded_reason:
            continue
        counts[record.arm][record.verdict] += 1
    rows: list[list[str]] = []
    for arm, counter in sorted(counts.items()):
        full = counter["circle_full"]
        failed = counter["circle_failed"]
        denominator = full + failed
        rows.append(
            [arm, str(full), str(failed), str(denominator), pct(full, denominator)]
        )
    return rows


def build_circle_summary(
    records: list[CircleRunRecord], scanned_sets: list[str]
) -> str:
    assert len(records) == CIRCLE_EXPECTED_RUNS
    included = [record for record in records if not record.excluded_reason]
    excluded = [record for record in records if record.excluded_reason]
    assert {record.set_id for record in included} == {CIRCLE_OFFICIAL_SET}, (
        "circle formal denominator contains a non-official set"
    )
    assert len(included) == 3, (
        f"circle formal denominator is {len(included)}, expected 3"
    )
    assert len(excluded) == 30, (
        f"circle exclusion count is {len(excluded)}, expected 30"
    )
    official_run = next(record for record in included if record.run_name == "run1")
    for required in ("investigation-binding.json", "workflow-circle.json"):
        assert official_run.circle_path.parent.joinpath(required).is_file(), (
            f"missing formal evidence {required}"
        )
    assert official_run.circle_path.parent.joinpath("fix-events.jsonl").is_file()
    lines = [
        "# Workflow Circle Capability Band Summary",
        "",
        "<!-- Generated by band_aggregate.py --profile circle. Do not edit by hand. -->",
        "",
        f"- Scanned circle sets: `{len(scanned_sets)}`",
        f"- Observed runs: `{len(records)}`",
        f"- Formal denominator: `{len(included)}` (`{CIRCLE_OFFICIAL_SET}` only)",
        f"- Exclusions: `{len(excluded)}`",
        f"- workflow-circle.json verified: `{len(records)}/{len(records)}`",
        f"- workflow_adjudicated verified and verdict-aligned: `{len(records)}/{len(records)}`",
        "- Zero-row policy: generation aborts before replacing the tracked output.",
        "",
        "## Formal elevated arm",
        "",
    ]
    lines.extend(
        table(
            ["Arm", "circle_full", "circle_failed", "denominator", "full rate"],
            circle_rate_rows(records),
        )
    )
    lines.extend(["", "## Excluded runs", ""])
    lines.extend(
        table(
            ["Set", "Run", "Recorded verdict", "Reason"],
            [
                [record.set_id, record.run_name, record.verdict, record.excluded_reason]
                for record in excluded
            ],
        )
    )
    lines.extend(["", "## Per-run evidence ledger", ""])
    lines.extend(
        table(
            [
                "Set",
                "Run",
                "Arm",
                "Verdict",
                "Terminal reason",
                "Band status",
                "workflow-circle.json",
                "workflow_adjudicated stream",
            ],
            [
                [
                    record.set_id,
                    record.run_name,
                    record.arm,
                    record.verdict,
                    record.reason,
                    record.excluded_reason or "formal denominator",
                    record.circle_path.relative_to(RUNS_DIR).as_posix(),
                    record.events_path.relative_to(RUNS_DIR).as_posix(),
                ]
                for record in records
            ],
        )
    )
    lines.extend(["", "## Source sets", ""])
    lines.extend(f"- `{set_id}`" for set_id in scanned_sets)
    lines.append("")
    return "\n".join(lines)


def cli_record_from_summary(
    set_id: str,
    summary: Path,
    row: dict[str, Any],
) -> CliRunRecord:
    run_name = str(row.get("name") or "")
    return CliRunRecord(
        set_id=set_id,
        run_name=run_name,
        family=str(row.get("family") or "unknown"),
        executor=str(row.get("executor") or "unknown"),
        harness_status=str(row.get("harness_status") or ""),
        product_exit=int(row.get("product_exit")),
        verdict=str(row.get("verdict") or ""),
        assurance=str(row.get("assurance") or ""),
        c1=str(row.get("c1") or "not_executed"),
        c2=str(row.get("c2") or "not_executed"),
        c3=str(row.get("c3") or "not_executed"),
        c4=str(row.get("c4") or "not_executed"),
        failure_class=str(row.get("class_id") or ""),
        attribution=str(row.get("attribution") or ""),
        duration_seconds=int(row.get("duration_seconds")),
        evidence_dir=summary.parent.parent / "artifacts" / run_name / "evidence",
        evidence_report=summary.parent.parent / "uat-report.md",
        pack_id=str(row.get("pack_id") or "none"),
        pack_hash=str(row.get("pack_hash") or ""),
        pack_exposed=bool(row.get("pack_exposed", False)),
        directive_round=int(row.get("directive_round", 0)),
        directive_hash=str(row.get("directive_hash") or ""),
        api_input_tokens=(
            int(row["api_input_tokens"])
            if row.get("api_input_tokens") is not None
            else None
        ),
        api_output_tokens=(
            int(row["api_output_tokens"])
            if row.get("api_output_tokens") is not None
            else None
        ),
        cost_usd=(
            float(row["cost_usd"]) if row.get("cost_usd") is not None else None
        ),
    )


def discover_cli_directive_records() -> list[CliRunRecord]:
    records = []
    for expected_round, measurement in enumerate(CLI_DIRECTIVE_MEASUREMENTS, 1):
        data = read_json_dict(measurement)
        if data is None:
            continue
        assert data.get("schema_version") == "1"
        assert data.get("set_id") == CLI_DIRECTIVE_SET
        assert data.get("directive_round") == expected_round
        directive_hash = str(data.get("directive_hash") or "")
        assert re.fullmatch(r"sha256:[0-9a-f]{64}", directive_hash), (
            "CLI directive measurement must pin an exact directive hash"
        )
        evidence_dir = RUNS_DIR / CLI_DIRECTIVE_SET / str(data["evidence_dir"])
        evidence_report = RUNS_DIR / CLI_DIRECTIVE_SET / str(data["evidence_report"])
        records.append(
            CliRunRecord(
                set_id=CLI_DIRECTIVE_SET,
                run_name=str(data["run_name"]),
                family=str(data["family"]),
                executor=str(data["executor"]),
                harness_status=str(data["harness_status"]),
                product_exit=int(data["product_exit"]),
                verdict=str(data["verdict"]),
                assurance=str(data["assurance"]),
                c1=str(data["c1"]),
                c2=str(data["c2"]),
                c3=str(data["c3"]),
                c4=str(data["c4"]),
                failure_class=str(data["failure_class"]),
                attribution=str(data["attribution"]),
                duration_seconds=int(data["duration_seconds"]),
                evidence_dir=evidence_dir,
                evidence_report=evidence_report,
                directive_round=expected_round,
                directive_hash=directive_hash,
            )
        )
    assert [record.directive_round for record in records] == list(
        range(1, len(records) + 1)
    ), "CLI directive rounds must be contiguous"
    return records


def normalize_cli_markdown_cell(value: str) -> str:
    return value.strip().replace("`", "").replace("**", "")


def normalize_cli_check(value: str) -> str:
    normalized = normalize_cli_markdown_cell(value).lower()
    if normalized == "—":
        return "not_reached"
    if normalized.startswith("pass"):
        return "pass"
    if normalized.startswith("fail"):
        return "fail"
    return normalized.replace(" ", "_")


def discover_cli_window_b_records() -> list[CliRunRecord]:
    report = RUNS_DIR / CLI_WINDOW_B_SET / "uat-report.md"
    text = report.read_text(encoding="utf-8")
    assert "product exit 1を6件保持" in text
    section = text.split("## 4. Run行列", 1)[1].split("\n## ", 1)[0]
    executor_match = re.search(r"^- executor: `([^`]+)`", text, re.MULTILINE)
    assert executor_match is not None, f"missing executor in {report}"
    executor = executor_match.group(1)
    records: list[CliRunRecord] = []
    for line in section.splitlines():
        if not line.startswith("| `"):
            continue
        cells = [
            normalize_cli_markdown_cell(cell) for cell in line.strip("|").split("|")
        ]
        assert len(cells) == 10, f"unexpected CLI Window B row: {line}"
        failure_class, attribution = cells[8].rsplit(" / ", 1)
        records.append(
            CliRunRecord(
                set_id=CLI_WINDOW_B_SET,
                run_name=cells[0],
                family=cells[1],
                executor=executor,
                harness_status="completed",
                product_exit=1,
                verdict=cells[2],
                assurance=cells[3],
                c1=normalize_cli_check(cells[4]),
                c2=normalize_cli_check(cells[5]),
                c3=normalize_cli_check(cells[6]),
                c4=normalize_cli_check(cells[7]),
                failure_class=failure_class,
                attribution=attribution,
                duration_seconds=int(cells[9]),
                evidence_dir=report.parent / "artifacts" / cells[0] / "evidence",
                evidence_report=report,
            )
        )
    assert len(records) == CLI_EXPECTED_RUNS_PER_SET, (
        f"CLI Window B has {len(records)} runs, expected {CLI_EXPECTED_RUNS_PER_SET}"
    )
    return records


def discover_cli_records() -> tuple[list[CliRunRecord], list[str]]:
    data = read_json_dict(CLI_LOCAL_SUMMARY)
    assert data is not None, f"missing CLI local-arm summary: {CLI_LOCAL_SUMMARY}"
    assert data.get("uat_id") == CLI_LOCAL_SET
    suite = data.get("suite")
    assert isinstance(suite, dict)
    assert suite.get("profile") == "cli"
    assert suite.get("intent") == "create"
    rows = data.get("runs")
    assert isinstance(rows, list)
    records: list[CliRunRecord] = []
    for row in rows:
        assert isinstance(row, dict)
        records.append(cli_record_from_summary(CLI_LOCAL_SET, CLI_LOCAL_SUMMARY, row))
    for set_id in CLI_ELEVATED_SETS[:-1]:
        summary = RUNS_DIR / set_id / "evidence" / "campaign-summary.json"
        elevated = read_json_dict(summary)
        assert elevated is not None, f"missing CLI elevated summary: {summary}"
        assert elevated.get("uat_id") == set_id
        suite = elevated.get("suite")
        assert isinstance(suite, dict)
        assert suite.get("profile") == "cli"
        assert suite.get("intent") == "create"
        elevated_rows = elevated.get("runs")
        assert isinstance(elevated_rows, list)
        for row in elevated_rows:
            assert isinstance(row, dict)
            records.append(cli_record_from_summary(set_id, summary, row))
    records.extend(discover_cli_window_b_records())
    for set_id in CLI_PACK_SETS:
        summary = CLI_PACK_SUMMARIES[set_id]
        pack_data = read_json_dict(summary)
        assert pack_data is not None, f"missing CLI pack-arm summary: {summary}"
        assert pack_data.get("uat_id") == set_id
        pack_suite = pack_data.get("suite")
        assert isinstance(pack_suite, dict)
        assert pack_suite.get("profile") == "cli"
        assert pack_suite.get("intent") == "create"
        pack = pack_suite.get("pack")
        assert isinstance(pack, dict)
        expected_id, expected_hash = CLI_PACK_PINS[set_id]
        assert f"{pack.get('id')}@{pack.get('version')}" == expected_id
        assert pack.get("hash") == expected_hash
        pack_rows = pack_data.get("runs")
        assert isinstance(pack_rows, list)
        for row in pack_rows:
            assert isinstance(row, dict)
            record = cli_record_from_summary(set_id, summary, row)
            assert record.pack_id == expected_id
            assert record.pack_hash == expected_hash
            records.append(record)
    luna_data = read_json_dict(CLI_LUNA_SUMMARY)
    assert luna_data is not None, f"missing CLI Luna-arm summary: {CLI_LUNA_SUMMARY}"
    assert luna_data.get("uat_id") == CLI_LUNA_SET
    luna_suite = luna_data.get("suite")
    assert isinstance(luna_suite, dict)
    assert luna_suite.get("profile") == "cli"
    assert luna_suite.get("intent") == "create"
    assert luna_suite.get("provider") == "openai"
    assert luna_suite.get("executor") == "gpt-5.6-luna"
    luna_rows = luna_data.get("runs")
    assert isinstance(luna_rows, list)
    for row in luna_rows:
        assert isinstance(row, dict)
        records.append(cli_record_from_summary(CLI_LUNA_SET, CLI_LUNA_SUMMARY, row))
    scanned_sets = [
        CLI_LOCAL_SET,
        *CLI_ELEVATED_SETS,
        *CLI_PACK_SETS,
        CLI_LUNA_SET,
    ]
    directive_records = discover_cli_directive_records()
    if directive_records:
        records.extend(directive_records)
        scanned_sets.append(CLI_DIRECTIVE_SET)
    return records, scanned_sets


def cli_evidence_attested(record: CliRunRecord, reached_in_set: int) -> bool:
    if all(record.evidence_dir.joinpath(name).is_file() for name in CLI_EVIDENCE_FILES):
        return True
    if record.evidence_report is None:
        return False
    text = record.evidence_report.read_text(encoding="utf-8")
    sha_section = text.rsplit("一次資料SHA-256", 1)[-1]
    for name in CLI_EVIDENCE_FILES:
        labels = [f"{record.run_name}/{name}"]
        if reached_in_set == 1:
            labels.append(name)
        if not any(
            re.search(
                rf"- `{re.escape(label)}`:\s*\n\s+`[0-9a-f]{{64}}`",
                sha_section,
            )
            for label in labels
        ):
            return False
    return True


def verify_cli_reached_evidence(records: list[CliRunRecord]) -> int:
    reached_counts = Counter(
        record.set_id for record in records if record.reached_checks
    )
    verified = 0
    for record in records:
        if not record.reached_checks:
            continue
        assert cli_evidence_attested(record, reached_counts[record.set_id]), (
            f"{record.run_name}: reached C checks without all four evidence attestations"
        )
        verified += 1
    return verified


def assert_cli_invariants(records: list[CliRunRecord]) -> int:
    directive_records = [
        record for record in records if record.set_id == CLI_DIRECTIVE_SET
    ]
    expected_runs = CLI_BASE_EXPECTED_RUNS + len(directive_records)
    assert len(records) == expected_runs, (
        f"CLI settlement has {len(records)} runs, expected {expected_runs}"
    )
    expected_sets = {
        set_id: CLI_EXPECTED_RUNS_PER_SET
        for set_id in [
            CLI_LOCAL_SET,
            *CLI_ELEVATED_SETS,
            *CLI_PACK_SETS,
            CLI_LUNA_SET,
        ]
    }
    if directive_records:
        expected_sets[CLI_DIRECTIVE_SET] = len(directive_records)
    assert Counter(record.set_id for record in records) == Counter(expected_sets), (
        "CLI source-set denominators drifted"
    )
    assert all(
        (record.directive_round == 0 and not record.directive_hash)
        or (
            record.set_id == CLI_DIRECTIVE_SET
            and record.directive_round > 0
            and re.fullmatch(r"sha256:[0-9a-f]{64}", record.directive_hash)
        )
        for record in records
    ), "CLI directive configuration was collapsed or incompletely pinned"
    local = [record for record in records if record.set_id == CLI_LOCAL_SET]
    assert Counter((record.family, record.executor) for record in local) == Counter(
        CLI_EXPECTED_MATRIX
    ), "CLI family/executor matrix drifted"
    pack_arm = [record for record in records if record.set_id in CLI_PACK_SETS]
    for set_id in CLI_PACK_SETS:
        pack_set = [record for record in pack_arm if record.set_id == set_id]
        assert Counter(
            (record.family, record.executor) for record in pack_set
        ) == Counter(
            {
                ("filter", "gemma4:31b-cloud"): 3,
                ("stats", "gemma4:31b-cloud"): 3,
            }
        ), f"CLI pack family/executor matrix drifted for {set_id}"
    assert all(
        (record.pack_id, record.pack_hash) == CLI_PACK_PINS[record.set_id]
        for record in pack_arm
    ), "CLI pack pin drifted"
    luna_arm = [record for record in records if record.set_id == CLI_LUNA_SET]
    assert Counter((record.family, record.executor) for record in luna_arm) == Counter(
        {
            ("filter", "gpt-5.6-luna"): 3,
            ("stats", "gpt-5.6-luna"): 3,
        }
    ), "CLI Luna family/executor matrix drifted"
    assert all(
        record.api_input_tokens == 0
        and record.api_output_tokens == 0
        and record.cost_usd == 0.0
        for record in luna_arm
    ), "CLI Luna cost observation is incomplete"
    for record in records:
        assert record.harness_status == "completed", (
            f"{record.run_name}: dishonest harness terminal"
        )
        if record.reached_checks:
            if record.is_full:
                assert {record.c1, record.c2, record.c3, record.c4} == {"pass"}, (
                    f"{record.run_name}: false full without C1-C4 pass"
                )
        elif record.set_id not in CLI_EXCLUSION_REASONS:
            assert record.verdict == "failed", (
                f"{record.run_name}: no C checks but verdict={record.verdict}"
            )
            expected_assurance = (
                "static (profile_not_admitted)"
                if record.set_id == CLI_LOCAL_SET
                else "static (cli_probe_not_run)"
            )
            assert record.assurance == expected_assurance, (
                f"{record.run_name}: no C checks but assurance={record.assurance}"
            )
    window_b = [record for record in records if record.set_id == CLI_WINDOW_B_SET]
    assert len(window_b) == CLI_EXPECTED_RUNS_PER_SET
    assert all(record.verdict == "failed" for record in window_b)
    return verify_cli_reached_evidence(records)


def cli_rate_rows(records: list[CliRunRecord]) -> list[list[str]]:
    counts: dict[tuple[str, str, int], Counter[str]] = defaultdict(Counter)
    for record in records:
        counts[(record.family, record.executor, record.directive_round)][
            "full" if record.is_full else "non_full"
        ] += 1
    rows = []
    for family, executor, directive_round in sorted(counts):
        full = counts[(family, executor, directive_round)]["full"]
        non_full = counts[(family, executor, directive_round)]["non_full"]
        denominator = full + non_full
        rows.append(
            [
                family,
                executor,
                str(directive_round),
                str(full),
                str(non_full),
                str(denominator),
                pct(full, denominator),
            ]
        )
    return rows


def cli_cost_rows(records: list[CliRunRecord]) -> list[list[str]]:
    groups: dict[tuple[str, str], list[CliRunRecord]] = defaultdict(list)
    for record in records:
        groups[(record.family, record.executor)].append(record)
    rows = []
    for (family, executor), grouped in sorted(groups.items()):
        full = sum(record.is_full for record in grouped)
        rows.append(
            [
                family,
                executor,
                str(full),
                str(len(grouped) - full),
                str(len(grouped)),
                str(sum(record.reached_checks for record in grouped)),
                str(sum(record.api_input_tokens or 0 for record in grouped)),
                str(sum(record.api_output_tokens or 0 for record in grouped)),
                f"${sum(record.cost_usd or 0.0 for record in grouped):.6f}",
            ]
        )
    return rows


def cli_band_status(set_id: str) -> str:
    if set_id in CLI_EXCLUSION_REASONS:
        return f"excluded — {CLI_EXCLUSION_REASONS[set_id]}"
    if set_id == CLI_CALIBRATION_SET:
        return "calibration predecessor — 配線後・較正前"
    if set_id == CLI_WINDOW_B_SET:
        return "formal Window B"
    if set_id == CLI_PACK_SETS[0]:
        return "A/B pack arm segment 1 — renderer exposure 0/6"
    if set_id == CLI_PACK_SETS[1]:
        return "A/B pack arm segment 2 — live renderer exposure 2/6"
    if set_id == CLI_PACK_SETS[2]:
        return (
            "assist ceiling measured — 援助飽和・効果なし "
            "(live 3/3同一署名); v1.1 testimony target 1/1 reached; "
            "C3 material 1/1; README write 0/1"
        )
    if set_id == CLI_DIRECTIVE_SET:
        return "interactive directive arm — human Gate 4 continuation"
    if set_id == CLI_LUNA_SET:
        return (
            "OpenAI Luna arm — machine-confounded before C checks: "
            "Chat Completions rejected function tools with reasoning_effort"
        )
    return "local reference arm"


def build_cli_summary(
    records: list[CliRunRecord],
    scanned_sets: list[str],
    evidence_verified: int,
) -> str:
    local = [record for record in records if record.set_id == CLI_LOCAL_SET]
    window_b = [record for record in records if record.set_id == CLI_WINDOW_B_SET]
    pack_arm = [record for record in records if record.set_id in CLI_PACK_SETS]
    directive_arm = [record for record in records if record.set_id == CLI_DIRECTIVE_SET]
    luna_arm = [record for record in records if record.set_id == CLI_LUNA_SET]
    reached = [record for record in records if record.reached_checks]
    window_b_reached = [record for record in window_b if record.reached_checks]
    dispositions = []
    for set_id in scanned_sets:
        set_records = [record for record in records if record.set_id == set_id]
        disposition_groups = (
            [[record] for record in set_records]
            if set_id == CLI_DIRECTIVE_SET
            else [set_records]
        )
        for disposition_records in disposition_groups:
            dispositions.append(
                [
                    set_id,
                    str(len(disposition_records)),
                    str(sum(record.reached_checks for record in disposition_records)),
                    str(sum(record.is_full for record in disposition_records)),
                    disposition_records[0].pack_label,
                    disposition_records[0].directive_label,
                    cli_band_status(set_id),
                ]
            )
    lines = [
        "# CLI Create Capability Band Summary",
        "",
        "<!-- Generated by band_aggregate.py --profile cli. Do not edit by hand. -->",
        "",
        full_meaning_label("cli"),
        f"- Source sets: `{len(scanned_sets)}` (local 1 + elevated 4 + pack arms 3 + Luna arm 1 + directive source 1)",
        f"- Observed runs: `{len(records)}`",
        f"- Honest terminals: `{sum(record.harness_status == 'completed' for record in records)}/{len(records)}`",
        f"- Formal Window B: `{CLI_WINDOW_B_SET}` only",
        f"- Window B full: `{sum(record.is_full for record in window_b)}/{len(window_b)}` ({pct(sum(record.is_full for record in window_b), len(window_b))})",
        f"- Window B runs reaching C checks: `{len(window_b_reached)}/{len(window_b)}`",
        f"- All-history runs reaching C checks: `{len(reached)}/{len(records)}`",
        f"- Reached-run C evidence sets verified: `{evidence_verified}/{len(reached)}`",
        f"- Pack v1.0.0 pin: `{CLI_PACK_ID}` / `{CLI_PACK_HASH}`",
        f"- Pack v1.1.0 pin: `{CLI_PACK_V1_1_ID}` / `{CLI_PACK_V1_1_HASH}`",
        f"- Pack arm full: `{sum(record.is_full for record in pack_arm)}/{len(pack_arm)}` ({pct(sum(record.is_full for record in pack_arm), len(pack_arm))}); Window Bとの差 `0 percentage points`",
        f"- Pack runs reaching C checks: `{sum(record.reached_checks for record in pack_arm)}/{len(pack_arm)}`",
        f"- Pack renderer exposure: `{sum(record.pack_exposed for record in pack_arm)}/{len(pack_arm)}`",
        "- Pack arm note: assist ceiling measured — 援助飽和・効果なし (live 3/3同一署名).",
        "- Pack interpretation: v1.0.0はlive 2件でC1材料のみ、v1.1.0はlive 1件でREADME照準+C3全3対を直接露出した。いずれもモデルはREADMEを更新せず、pack armのC3は9/9 violationのままだった。",
        f"- Directive arm: `{sum(record.is_full for record in directive_arm)}/{len(directive_arm)}` full; each round is a distinct configuration and retains its directive hash.",
        f"- Luna arm: `{sum(record.is_full for record in luna_arm)}/{len(luna_arm)}` full; C checks reached `{sum(record.reached_checks for record in luna_arm)}/{len(luna_arm)}`; observed API usage `{sum(record.api_input_tokens or 0 for record in luna_arm)}` input / `{sum(record.api_output_tokens or 0 for record in luna_arm)}` output tokens; calculated cost `${sum(record.cost_usd or 0.0 for record in luna_arm):.6f}`.",
        "- Luna interpretation: 6/6 requests were rejected before generation because Chat Completions received function tools with reasoning_effort; C3 testimony behavior and response fingerprint were not observed.",
        "- Reach-rate comparison: Window B 2/6 (33.3%) vs v1.1.0 arm 1/6 (16.7%), -16.6 percentage points; combined pack arms 3/18 (16.7%) (descriptive only).",
        "- Invariant: C evidence files are mandatory only after a run reaches the CLI checks.",
        "- Invariant: every reached run must attest cli-case-binding.json, cli-probe.json, help-binding.json, and cli-assurance.json by file or recorded SHA-256.",
        "- Invariant: non-reaching admitted runs outside defect-era exclusions remain failed with static (cli_probe_not_run) assurance.",
        "- Zero-row policy: generation aborts before replacing the tracked output.",
        "",
        "## Formal Window B by family and executor",
        "",
    ]
    lines.extend(
        table(
            ["Family", "Executor", "Directive round", "full", "non-full", "denominator", "full rate"],
            cli_rate_rows(window_b),
        )
    )
    lines.extend(["", "## Local reference arm by family and executor", ""])
    lines.extend(
        table(
            ["Family", "Executor", "Directive round", "full", "non-full", "denominator", "full rate"],
            cli_rate_rows(local),
        )
    )
    lines.extend(["", "## Pack A/B arm by family and executor", ""])
    lines.extend(
        table(
            ["Family", "Executor", "Directive round", "full", "non-full", "denominator", "full rate"],
            cli_rate_rows(pack_arm),
        )
    )
    lines.extend(["", "## Interactive directive arm", ""])
    lines.extend(
        table(
            ["Family", "Executor", "Directive round", "full", "non-full", "denominator", "full rate"],
            cli_rate_rows(directive_arm),
        )
        if directive_arm
        else ["No live directive measurement has been admitted yet."]
    )
    lines.extend(["", "## OpenAI Luna arm with observed cost", ""])
    lines.extend(
        table(
            [
                "Family",
                "Executor",
                "full",
                "non-full",
                "denominator",
                "C reached",
                "API input tokens",
                "API output tokens",
                "Cost USD",
            ],
            cli_cost_rows(luna_arm),
        )
    )
    lines.extend(["", "## Campaign disposition", ""])
    lines.extend(
        table(
            ["Set", "Runs", "C reached", "Full", "Pack ID / hash", "Directive round / hash", "Band status"],
            dispositions,
        )
    )
    lines.extend(["", "## Per-run ledger", ""])
    lines.extend(
        table(
            [
                "Set",
                "Run",
                "Family",
                "Executor",
                "Pack ID / hash",
                "Directive round / hash",
                "Verdict",
                "Assurance",
                "C1",
                "C2",
                "C3",
                "C4",
                "Failure class",
                "Attribution",
                "Band status",
                "Seconds",
                "Cost USD",
            ],
            [
                [
                    record.set_id,
                    record.run_name,
                    record.family,
                    record.executor,
                    record.pack_label,
                    record.directive_label,
                    record.verdict,
                    record.assurance,
                    record.c1,
                    record.c2,
                    record.c3,
                    record.c4,
                    record.failure_class,
                    record.attribution,
                    cli_band_status(record.set_id),
                    str(record.duration_seconds),
                    record.cost_label,
                ]
                for record in records
            ],
        )
    )
    lines.extend(["", "## Source sets", ""])
    lines.extend(f"- `{set_id}`" for set_id in scanned_sets)
    lines.append("")
    return "\n".join(lines)


def ingest_record_from_summary(
    set_id: str,
    source_set_id: str,
    summary: Path,
    suite: dict[str, Any],
    row: dict[str, Any],
) -> IngestRunRecord:
    earned = str(
        row.get("earned_assurance")
        or row.get("assurance")
        or ("failed" if row.get("verdict") == "failed" else "")
    )
    display = str(row.get("display_assurance") or row.get("assurance") or earned)
    return IngestRunRecord(
        set_id=set_id,
        source_set_id=source_set_id,
        run_name=str(row.get("name") or ""),
        family=str(row.get("family") or "unknown"),
        executor=str(row.get("executor") or suite.get("executor") or "unknown"),
        harness_status=str(row.get("harness_status") or ""),
        product_exit=int(row.get("product_exit")),
        verdict=str(row.get("verdict") or ""),
        earned_assurance=earned,
        display_assurance=display,
        n1=str(row.get("n1") or "not_reached"),
        n2=str(row.get("n2") or "not_reached"),
        n3=str(row.get("n3") or "not_reached"),
        n4=str(row.get("n4") or "not_reached"),
        n5=str(row.get("n5") or "not_reached"),
        failure_class=str(row.get("automatic_class") or ""),
        attribution=str(row.get("audited_attribution") or ""),
        duration_seconds=int(row.get("duration_seconds")),
        evidence_summary=summary,
        evidence_report=summary.parent.parent / "uat-report.md",
    )


def discover_ingest_records() -> tuple[list[IngestRunRecord], list[str]]:
    sources = [
        (INGEST_LOCAL_ALIAS, INGEST_LOCAL_SOURCE_SET),
        *((set_id, set_id) for set_id in INGEST_ELEVATED_SETS),
    ]
    records: list[IngestRunRecord] = []
    for set_id, source_set_id in sources:
        summary = (
            RUNS_DIR / source_set_id / "evidence" / "campaign-summary.json"
        )
        data = read_json_dict(summary)
        assert data is not None, f"missing ingest campaign summary: {summary}"
        assert data.get("uat_id") == source_set_id
        suite = data.get("suite")
        assert isinstance(suite, dict)
        assert suite.get("profile") == "ingest"
        assert suite.get("intent") == "create"
        rows = data.get("runs")
        assert isinstance(rows, list)
        for row in rows:
            assert isinstance(row, dict)
            records.append(
                ingest_record_from_summary(
                    set_id,
                    source_set_id,
                    summary,
                    suite,
                    row,
                )
            )
    return records, [INGEST_LOCAL_ALIAS, *INGEST_ELEVATED_SETS]


def verify_ingest_reached_evidence(records: list[IngestRunRecord]) -> int:
    verified = 0
    for record in records:
        if not record.reached_checks:
            continue
        statuses = (record.n1, record.n2, record.n3, record.n4, record.n5)
        assert all(
            status not in {"not_executed", "not_reached"} for status in statuses
        ), f"{record.set_id}/{record.run_name}: partial N1-N5 evidence set"
        assert record.evidence_summary.is_file(), (
            f"{record.set_id}/{record.run_name}: missing campaign evidence summary"
        )
        assert record.evidence_report.is_file(), (
            f"{record.set_id}/{record.run_name}: missing evidence audit report"
        )
        verified += 1
    return verified


def assert_ingest_invariants(records: list[IngestRunRecord]) -> int:
    assert len(records) == INGEST_EXPECTED_RUNS, (
        f"ingest settlement has {len(records)} runs, expected {INGEST_EXPECTED_RUNS}"
    )
    assert Counter(record.set_id for record in records) == Counter(
        {
            set_id: INGEST_EXPECTED_RUNS_PER_SET
            for set_id in [INGEST_LOCAL_ALIAS, *INGEST_ELEVATED_SETS]
        }
    ), "ingest source-set denominators drifted"
    assert all(record.harness_status == "completed" for record in records), (
        "ingest settlement contains a dishonest harness terminal"
    )

    local = [record for record in records if record.set_id == INGEST_LOCAL_ALIAS]
    assert Counter((record.family, record.executor) for record in local) == Counter(
        INGEST_LOCAL_MATRIX
    ), "ingest local family/executor matrix drifted"
    assert not any(record.is_full for record in local)

    window_b = [
        record for record in records if record.set_id == INGEST_WINDOW_B_SET
    ]
    assert Counter(
        (record.family, record.executor) for record in window_b
    ) == Counter(INGEST_WINDOW_B_MATRIX), "ingest Window B matrix drifted"
    assert sum(record.is_full for record in window_b) == 4
    assert sum(record.verdict == "failed" for record in window_b) == 2
    assert not any(record.attribution == "machine" for record in window_b)
    for record in window_b:
        assert record.reached_checks
        if record.is_full:
            assert {record.n1, record.n2, record.n3, record.n4, record.n5} == {
                "pass"
            }, f"{record.run_name}: false ingest full"
            assert record.product_exit == 0
        else:
            assert record.product_exit != 0
            assert record.earned_assurance == "failed"
    return verify_ingest_reached_evidence(records)


def ingest_rate_rows(records: list[IngestRunRecord]) -> list[list[str]]:
    counts: dict[tuple[str, str], Counter[str]] = defaultdict(Counter)
    for record in records:
        counts[(record.family, record.executor)][
            "full" if record.is_full else "non_full"
        ] += 1
    rows = []
    for family, executor in sorted(counts):
        full = counts[(family, executor)]["full"]
        non_full = counts[(family, executor)]["non_full"]
        denominator = full + non_full
        rows.append(
            [
                family,
                executor,
                str(full),
                str(non_full),
                str(denominator),
                ingest_pct(full, denominator),
            ]
        )
    return rows


def ingest_band_status(set_id: str) -> str:
    if set_id in INGEST_EXCLUSION_REASONS:
        return f"excluded — {INGEST_EXCLUSION_REASONS[set_id]}"
    if set_id == INGEST_WINDOW_B_SET:
        return "formal elevated Window B"
    return (
        "formal local reference — repository source "
        f"{INGEST_LOCAL_SOURCE_SET}; requested alias {INGEST_LOCAL_ALIAS}"
    )


def build_ingest_summary(
    records: list[IngestRunRecord],
    scanned_sets: list[str],
    evidence_verified: int,
) -> str:
    local = [record for record in records if record.set_id == INGEST_LOCAL_ALIAS]
    window_b = [
        record for record in records if record.set_id == INGEST_WINDOW_B_SET
    ]
    reached = [record for record in records if record.reached_checks]
    dispositions = []
    for set_id in scanned_sets:
        set_records = [record for record in records if record.set_id == set_id]
        dispositions.append(
            [
                set_id,
                str(len(set_records)),
                str(sum(record.reached_checks for record in set_records)),
                str(sum(record.is_full for record in set_records)),
                ingest_band_status(set_id),
            ]
        )
    window_full = sum(record.is_full for record in window_b)
    lines = [
        "# Ingest Create Capability Band Summary",
        "",
        "<!-- Generated by band_aggregate.py --profile ingest. Do not edit by hand. -->",
        "",
        full_meaning_label("ingest"),
        f"- Source sets: `{len(scanned_sets)}` (local 1 + elevated 8)",
        f"- Observed runs: `{len(records)}`",
        f"- Honest terminals: `{sum(record.harness_status == 'completed' for record in records)}/{len(records)}`",
        f"- Local reference: `{INGEST_LOCAL_ALIAS}` display alias → immutable repository source `{INGEST_LOCAL_SOURCE_SET}`",
        f"- Formal elevated Window B: `{INGEST_WINDOW_B_SET}` only",
        f"- Local full-equivalent: `{sum(record.is_full for record in local)}/{len(local)}` ({ingest_pct(sum(record.is_full for record in local), len(local))})",
        f"- Window B full-equivalent: `{window_full}/{len(window_b)}` ({ingest_pct(window_full, len(window_b))})",
        f"- Window B machine-attributed terminals: `{sum(record.attribution == 'machine' for record in window_b)}/{len(window_b)}`",
        f"- All-history runs reaching N checks: `{len(reached)}/{len(records)}`",
        f"- Reached-run N1-N5 evidence sets verified: `{evidence_verified}/{len(reached)}`",
        "- Invariant: N1-N5 result evidence is mandatory for every run that executes any N check; non-reaching runs may not borrow artifact presence as N evidence.",
        "- Invariant: full-equivalent requires N1-N5 all pass; failed N2/N3/N4/N5 remains failed even under the historical draft cap.",
        "- Calibration campaigns elev-001 through elev-007 stay visible but never enter the local or Window B denominators.",
        "- Zero-row policy: generation aborts before replacing the tracked output.",
        "",
        "## Formal elevated Window B by family and executor",
        "",
    ]
    lines.extend(
        table(
            ["Family", "Executor", "full", "non-full", "denominator", "full rate"],
            ingest_rate_rows(window_b),
        )
    )
    lines.extend(["", "## Local reference arm by family and executor", ""])
    lines.extend(
        table(
            ["Family", "Executor", "full", "non-full", "denominator", "full rate"],
            ingest_rate_rows(local),
        )
    )
    lines.extend(["", "## Campaign disposition", ""])
    lines.extend(
        table(
            ["Set", "Runs", "N reached", "Full-equivalent", "Band status"],
            dispositions,
        )
    )
    lines.extend(["", "## Per-run ledger", ""])
    lines.extend(
        table(
            [
                "Set",
                "Run",
                "Family",
                "Executor",
                "Verdict",
                "Earned",
                "Display at measurement",
                "N1",
                "N2",
                "N3",
                "N4",
                "N5",
                "Failure class",
                "Attribution",
                "Band status",
                "Seconds",
            ],
            [
                [
                    record.set_id,
                    record.run_name,
                    record.family,
                    record.executor,
                    record.verdict,
                    record.earned_assurance,
                    record.display_assurance,
                    record.n1,
                    record.n2,
                    record.n3,
                    record.n4,
                    record.n5,
                    record.failure_class or "—",
                    record.attribution or "—",
                    ingest_band_status(record.set_id),
                    str(record.duration_seconds),
                ]
                for record in records
            ],
        )
    )
    lines.extend(["", "## Source sets", ""])
    lines.append(
        f"- `{INGEST_LOCAL_ALIAS}` → `{INGEST_LOCAL_SOURCE_SET}` "
        "(display alias only; historical evidence is not rewritten)"
    )
    lines.extend(f"- `{set_id}`" for set_id in INGEST_ELEVATED_SETS)
    lines.append("")
    return "\n".join(lines)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--profile",
        choices=(
            "nextjs",
            "data",
            "fix",
            "investigation",
            "circle",
            "cli",
            "ingest",
        ),
        default="nextjs",
        help=(
            "capability band to aggregate (nextjs/data create, nextjs fix, "
            "data investigation, workflow circle, CLI create, or ingest create)"
        ),
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        if args.profile == "ingest":
            ingest_records, scanned_sets = discover_ingest_records()
            require_nonempty_aggregation(
                args.profile,
                ingest_records,
                profile_set_diagnostics(ingest_records, scanned_sets),
            )
            evidence_verified = assert_ingest_invariants(ingest_records)
            summary = build_ingest_summary(
                ingest_records,
                scanned_sets,
                evidence_verified,
            )
            output = INGEST_OUTPUT
        elif args.profile == "cli":
            cli_records, scanned_sets = discover_cli_records()
            require_nonempty_aggregation(
                args.profile,
                cli_records,
                profile_set_diagnostics(cli_records, scanned_sets),
            )
            evidence_verified = assert_cli_invariants(cli_records)
            summary = build_cli_summary(
                cli_records,
                scanned_sets,
                evidence_verified,
            )
            output = CLI_OUTPUT
        elif args.profile == "circle":
            circle_records, scanned_sets = discover_circle_records()
            official_records = [
                record for record in circle_records if not record.excluded_reason
            ]
            require_nonempty_aggregation(
                args.profile,
                official_records,
                profile_set_diagnostics(official_records, [CIRCLE_OFFICIAL_SET]),
            )
            summary = build_circle_summary(circle_records, scanned_sets)
            output = CIRCLE_OUTPUT
        elif args.profile == "investigation":
            investigation_records, scanned_sets = discover_investigation_records()
            require_nonempty_aggregation(
                args.profile,
                investigation_records,
                profile_set_diagnostics(investigation_records, scanned_sets),
            )
            summary = build_investigation_summary(investigation_records, scanned_sets)
            output = INVESTIGATION_OUTPUT
        elif args.profile == "fix":
            fix_records, scanned_sets = discover_fix_records()
            require_nonempty_aggregation(
                args.profile,
                fix_records,
                profile_set_diagnostics(fix_records, scanned_sets),
            )
            full_verified = assert_full_fix_evidence(fix_records)
            summary = build_fix_summary(fix_records, scanned_sets, full_verified)
            output = FIX_OUTPUT
        elif args.profile == "data":
            data_records, scanned_rows, meta_rows, scanned_sets = (
                discover_data_records()
            )
            require_nonempty_aggregation(
                args.profile,
                data_records,
                profile_set_diagnostics(data_records, scanned_sets),
            )
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
            (
                records,
                aggregate_row_total,
                _aggregate_record_total,
                scanned_sets,
                diagnostics,
            ) = discover_records()
            require_nonempty_aggregation(args.profile, records, diagnostics)
            summary = build_summary(records, aggregate_row_total, scanned_sets)
            output = OUTPUT
    except EmptyAggregationError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    output.write_text(summary, encoding="utf-8")
    print(summary)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
