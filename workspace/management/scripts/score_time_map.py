#!/usr/bin/env python3
"""Build the generated score/time projection without rewriting band reports."""

from __future__ import annotations

import hashlib
import html
import json
import math
import statistics
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Any

MAP_SCHEMA = "commandagent.score-time-map/v0"
MAP_COMMAND = (
    "python3 workspace/management/scripts/band_aggregate.py --profile score-time-map"
)
HISTORICAL_VECTOR_COUNT = 287
HISTORICAL_FINAL_ONLY_COUNT = 251
HISTORICAL_CHECKPOINT_COUNT = 36
SUPPLEMENTAL_RUN_COUNT = 48


@dataclass(frozen=True)
class Observation:
    """One formal run and its configuration-instance accounting."""

    profile: str
    model: str
    family: str
    configuration: str
    marker: str
    run_id: str
    score: float | None
    full: bool
    duration_seconds: float | None
    cost_usd: float | None
    instance_id: str
    instance_seconds: float | None
    instance_cost_usd: float | None
    instance_success: bool
    source: str


@dataclass(frozen=True)
class MapCell:
    """A model/use-case/configuration point and its denominator accounting."""

    point_id: str
    profile: str
    model: str
    family: str
    configuration: str
    marker: str
    n: int
    reached: int
    full: int
    instance_count: int
    successful_instances: int
    mean_seconds: float | None
    time_coverage: int
    mean_score: float
    five_number: tuple[float, float, float, float, float]
    mean_cost_usd: float | None
    cost_coverage: int
    expected_seconds_per_success: float | None
    expected_cost_per_success: float | None
    plotted: bool
    plot_reason: str

    @property
    def use_case(self) -> str:
        return f"{self.profile}/{self.family}"


def _read_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    assert isinstance(value, dict), f"JSON root must be an object: {path}"
    return value


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _load_checkpoint_run_ids(path: Path) -> set[str]:
    run_ids: set[str] = set()
    for line_number, line in enumerate(
        path.read_text(encoding="utf-8").splitlines(), start=1
    ):
        value = json.loads(line)
        assert isinstance(value, dict), (
            f"checkpoint is not an object: {path}:{line_number}"
        )
        run_id = value.get("run_id")
        assert isinstance(run_id, str) and run_id, (
            f"checkpoint run_id missing: {path}:{line_number}"
        )
        run_ids.add(run_id)
    return run_ids


def _cli_configuration(record: Any) -> str:
    if record.directive_round:
        suffix = f"#{record.directive_hash}" if record.directive_hash else ""
        return f"directive:round{record.directive_round}{suffix}"
    if record.pack_id != "none":
        suffix = f"#{record.pack_hash}" if record.pack_hash else ""
        return f"pack:{record.pack_id}{suffix}"
    return "single"


def _historical_records(band: Any) -> dict[str, list[Any]]:
    return {
        "data": band.discover_data_records()[0],
        "fix": band.discover_fix_records()[0],
        "investigation": band.discover_investigation_records()[0],
        "circle": band.discover_circle_records()[0],
        "cli": band.discover_cli_records()[0],
        "ingest": band.discover_ingest_records()[0],
    }


def _circle_durations(band: Any, records: list[Any]) -> dict[str, int]:
    """Recover the immutable workflow wall-time tables without adding a parser dep."""
    by_set: dict[str, dict[int, int]] = {}
    for set_id in sorted({record.set_id for record in records}):
        report = band.RUNS_DIR / set_id / "uat-report.md"
        lines = report.read_text(encoding="utf-8").splitlines()
        table: dict[int, int] | None = None
        for index, line in enumerate(lines):
            if not line.startswith("|"):
                continue
            headers = [cell.strip().lower() for cell in line.strip("|").split("|")]
            duration_indexes = [
                position
                for position, header in enumerate(headers)
                if header in {"wall seconds", "elapsed"}
            ]
            if not duration_indexes or headers[0] != "run":
                continue
            duration_index = duration_indexes[0]
            candidate: dict[int, int] = {}
            for row in lines[index + 2 :]:
                if not row.startswith("|"):
                    break
                cells = [cell.strip() for cell in row.strip("|").split("|")]
                if len(cells) <= duration_index:
                    break
                run_text = cells[0].strip("`")
                duration_text = cells[duration_index].replace(",", "")
                duration_text = duration_text.removesuffix(" s").strip("`")
                if not run_text.isdigit() or not duration_text.isdigit():
                    break
                candidate[int(run_text)] = int(duration_text)
            if candidate:
                table = candidate
                break
        assert table is not None, f"workflow duration table missing: {report}"
        by_set[set_id] = table

    durations: dict[str, int] = {}
    for record in records:
        run_number_text = record.run_name.removeprefix("run")
        assert run_number_text.isdigit(), (
            f"unexpected circle run name: {record.run_name}"
        )
        run_number = int(run_number_text)
        assert run_number in by_set[record.set_id], (
            f"workflow duration missing: {record.set_id}/{record.run_name}"
        )
        durations[band.band_record_id(record)] = by_set[record.set_id][run_number]
    assert len(durations) == 33, f"expected 33 workflow durations, got {len(durations)}"
    return durations


def _historical_observations(band: Any) -> tuple[list[Observation], list[Any]]:
    vectors = band.load_final_score_vectors()
    assert len(vectors) == HISTORICAL_VECTOR_COUNT, (
        f"expected frozen {HISTORICAL_VECTOR_COUNT}-run retrospective, got {len(vectors)}"
    )
    checkpoint_ids = _load_checkpoint_run_ids(
        band.RUNS_DIR / "f1-retrospective-001" / "checkpoint-vectors.jsonl"
    )
    assert len(checkpoint_ids) == HISTORICAL_CHECKPOINT_COUNT, (
        f"expected {HISTORICAL_CHECKPOINT_COUNT} checkpoint-capable runs, "
        f"got {len(checkpoint_ids)}"
    )
    records = _historical_records(band)
    circle_durations = _circle_durations(band, records["circle"])
    observations: list[Observation] = []
    unmatched_ingest: list[Any] = []
    for profile, profile_records in records.items():
        for record in profile_records:
            run_id = band.band_record_id(record)
            vector = vectors.get((profile, run_id))
            if vector is None:
                assert profile == "ingest", (
                    f"historical vector missing: {profile}/{run_id}"
                )
                unmatched_ingest.append(record)
                continue
            configuration = _cli_configuration(record) if profile == "cli" else "single"
            duration = (
                circle_durations[run_id]
                if profile == "circle"
                else getattr(record, "duration_seconds", None)
            )
            cost = getattr(record, "cost_usd", None)
            marker = "checkpoint" if run_id in checkpoint_ids else "verdict_mapping"
            score = vector["score"]
            observations.append(
                Observation(
                    profile=profile,
                    model=str(vector["model"]),
                    family=str(vector["family"]),
                    configuration=configuration,
                    marker=marker,
                    run_id=run_id,
                    score=float(score) if score is not None else None,
                    full=bool(record.is_full),
                    duration_seconds=float(duration) if duration is not None else None,
                    cost_usd=float(cost) if cost is not None else None,
                    instance_id=run_id,
                    instance_seconds=float(duration) if duration is not None else None,
                    instance_cost_usd=float(cost) if cost is not None else None,
                    instance_success=bool(record.is_full),
                    source="f1-retrospective-001/final-vectors.jsonl",
                )
            )
    assert len(observations) == HISTORICAL_VECTOR_COUNT
    final_only = sum(item.marker == "verdict_mapping" for item in observations)
    assert final_only == HISTORICAL_FINAL_ONLY_COUNT, (
        f"expected {HISTORICAL_FINAL_ONLY_COUNT} final-only runs, got {final_only}"
    )
    return observations, unmatched_ingest


def _ingest_luna_observations(
    band: Any, unmatched_records: list[Any]
) -> list[Observation]:
    source = (
        band.RUNS_DIR
        / "uat-test0802-ingest-luna-001"
        / "evidence"
        / "campaign-summary.json"
    )
    document = _read_json(source)
    rows = document.get("runs")
    assert isinstance(rows, list) and len(rows) == 6, (
        f"expected six Luna ingest rows: {source}"
    )
    records = {record.run_name: record for record in unmatched_records}
    observations: list[Observation] = []
    for row in rows:
        assert isinstance(row, dict)
        run_name = str(row["name"])
        record = records.pop(run_name)
        score = row.get("score")
        duration = float(row["duration_seconds"])
        cost = float(row["cost_usd"])
        observations.append(
            Observation(
                profile="ingest",
                model=str(row["executor"]),
                family=str(row["family"]),
                configuration="single",
                marker="verdict_mapping",
                run_id=band.band_record_id(record),
                score=float(score) if score is not None else None,
                full=bool(record.is_full),
                duration_seconds=duration,
                cost_usd=cost,
                instance_id=band.band_record_id(record),
                instance_seconds=duration,
                instance_cost_usd=cost,
                instance_success=bool(record.is_full),
                source=str(source.relative_to(band.ROOT)),
            )
        )
    assert not records, f"unmatched Luna ingest records: {sorted(records)}"
    return observations


def _luna_bon_observations(
    band: Any,
) -> tuple[list[Observation], list[tuple[str, str]]]:
    accounting_path = (
        band.RUNS_DIR / "f-bon-v-001" / "evidence" / "luna-selection-accounting.json"
    )
    accounting = _read_json(accounting_path)
    campaigns = accounting.get("campaigns")
    assert isinstance(campaigns, list) and len(campaigns) == 5, (
        f"expected five admitted Luna campaigns: {accounting_path}"
    )
    observations: list[Observation] = []
    sources: list[tuple[str, str]] = [
        (str(accounting_path.relative_to(band.ROOT)), _sha256(accounting_path))
    ]
    for campaign in campaigns:
        assert isinstance(campaign, dict)
        relative_source = Path(str(campaign["selection_source"]))
        source = band.ROOT / relative_source
        expected_sha = str(campaign["selection_source_sha256"])
        actual_sha = _sha256(source)
        assert actual_sha == expected_sha, (
            f"Luna selection source SHA mismatch: {source}"
        )
        document = _read_json(source)
        rows = document.get("runs")
        summary = document.get("summary")
        assert isinstance(rows, list) and len(rows) == 6
        assert isinstance(summary, dict)
        instance_id = f"luna-bon/{campaign['window']}"
        instance_seconds = float(summary["duration_seconds_total"])
        instance_cost = float(summary["cost_usd_total"])
        instance_success = int(summary.get("full_count", summary["earned_full"])) > 0
        assert sum(bool(row["earned_full"]) for row in rows) == int(
            campaign["full_count"]
        )
        for row in rows:
            assert isinstance(row, dict)
            score_vector = row.get("score_vector")
            assert isinstance(score_vector, dict)
            score = score_vector.get("score")
            observations.append(
                Observation(
                    profile="cli",
                    model="gpt-5.6-luna",
                    family="filter",
                    configuration="bon:6",
                    marker="verdict_mapping",
                    run_id=f"{instance_id}/{row['name']}",
                    score=float(score) if score is not None else None,
                    full=bool(row["earned_full"]),
                    duration_seconds=float(row["duration_seconds"]),
                    cost_usd=float(row["cost_usd"]),
                    instance_id=instance_id,
                    instance_seconds=instance_seconds,
                    instance_cost_usd=instance_cost,
                    instance_success=instance_success,
                    source=str(relative_source),
                )
            )
        sources.append((str(relative_source), actual_sha))
    assert len(observations) == 30
    return observations, sources


def _gemma_negative_observations(
    band: Any,
) -> tuple[list[Observation], tuple[str, str]]:
    source = band.RUNS_DIR / "f-bon-v-001" / "evidence" / "gemma-negative-result.json"
    document = _read_json(source)
    rows = document.get("runs")
    observed = document.get("observed")
    assert isinstance(rows, list) and len(rows) == 6
    assert isinstance(observed, dict)
    assert int(observed["duration_seconds_total"]) == sum(
        int(row["duration_seconds"]) for row in rows
    )
    instance_seconds = float(observed["duration_seconds_total"])
    observations: list[Observation] = []
    for row in rows:
        assert isinstance(row, dict)
        full = (
            row.get("final_acceptance_status") == "full_success"
            and row.get("assurance_level") == "full"
        )
        observations.append(
            Observation(
                profile="cli",
                model="gemma4:31b-cloud",
                family=str(row["goal"]),
                configuration="bon:6",
                marker="verdict_mapping",
                run_id=f"gemma-negative/{row['name']}",
                score=100.0 if full else None,
                full=full,
                duration_seconds=float(row["duration_seconds"]),
                cost_usd=None,
                instance_id="gemma-negative/bon-neg-001",
                instance_seconds=instance_seconds,
                instance_cost_usd=None,
                instance_success=False,
                source=str(source.relative_to(band.ROOT)),
            )
        )
    return observations, (str(source.relative_to(band.ROOT)), _sha256(source))


def _local_breakout_observations(
    band: Any,
) -> tuple[list[Observation], list[tuple[str, str]]]:
    source = band.RUNS_DIR / "f-bon-v-001" / "evidence" / "local-breakout-result.json"
    document = _read_json(source)
    rows = document.get("runs")
    campaign = document.get("campaign")
    observation = document.get("observation")
    assert isinstance(rows, list) and len(rows) == 6
    assert isinstance(campaign, dict) and isinstance(observation, dict)
    instance_seconds = float(campaign["duration_seconds_total"])
    assert int(instance_seconds) == sum(int(row["duration_seconds"]) for row in rows)
    instance_success = bool(observation["at_least_one_full"])
    predeclaration_ref = document.get("predeclaration")
    assert isinstance(predeclaration_ref, dict)
    predeclaration_path = band.ROOT / str(predeclaration_ref["path"])
    assert _sha256(predeclaration_path) == str(predeclaration_ref["sha256"]), (
        f"local predeclaration SHA mismatch: {predeclaration_path}"
    )
    predeclaration = _read_json(predeclaration_path)
    local_measurement = predeclaration.get("local_measurement")
    assert isinstance(local_measurement, dict)
    model = str(local_measurement["executor"])
    observations: list[Observation] = []
    for row in rows:
        assert isinstance(row, dict)
        full = bool(row["full"])
        observations.append(
            Observation(
                profile="nextjs",
                model=model,
                family="Breakout",
                configuration="bon:6",
                marker="verdict_mapping",
                run_id=f"local-breakout/{row['run']}",
                score=100.0 if full else None,
                full=full,
                duration_seconds=float(row["duration_seconds"]),
                cost_usd=None,
                instance_id="local-breakout/bon-local-001",
                instance_seconds=instance_seconds,
                instance_cost_usd=None,
                instance_success=instance_success,
                source=str(source.relative_to(band.ROOT)),
            )
        )
    assert sum(item.full for item in observations) == int(observation["full_count"])
    return observations, [
        (str(source.relative_to(band.ROOT)), _sha256(source)),
        (str(predeclaration_path.relative_to(band.ROOT)), _sha256(predeclaration_path)),
    ]


def collect_observations(band: Any) -> tuple[list[Observation], list[tuple[str, str]]]:
    historical, unmatched_ingest = _historical_observations(band)
    ingest_luna = _ingest_luna_observations(band, unmatched_ingest)
    luna_bon, sources = _luna_bon_observations(band)
    gemma_negative, gemma_source = _gemma_negative_observations(band)
    local_breakout, local_sources = _local_breakout_observations(band)
    supplemental = ingest_luna + luna_bon + gemma_negative + local_breakout
    assert len(supplemental) == SUPPLEMENTAL_RUN_COUNT
    source_rows = [
        (
            "workspace/management/runs/f1-retrospective-001/final-vectors.jsonl",
            _sha256(band.SCORE_VECTOR_PATH),
        ),
        (
            "workspace/management/runs/f1-retrospective-001/checkpoint-vectors.jsonl",
            _sha256(
                band.RUNS_DIR / "f1-retrospective-001" / "checkpoint-vectors.jsonl"
            ),
        ),
        (
            "workspace/management/runs/uat-test0802-ingest-luna-001/evidence/campaign-summary.json",
            _sha256(
                band.RUNS_DIR
                / "uat-test0802-ingest-luna-001"
                / "evidence"
                / "campaign-summary.json"
            ),
        ),
        *sources,
        gemma_source,
        *local_sources,
    ]
    return historical + supplemental, source_rows


def score_contribution(observation: Observation) -> float:
    """Map an unreached formal run to zero without mutating its nullable score."""
    return observation.score if observation.score is not None else 0.0


def _five_number(
    values: list[float], quantile: Any
) -> tuple[float, float, float, float, float]:
    return (
        round(min(values), 1),
        quantile(values, 0.25),
        quantile(values, 0.5),
        quantile(values, 0.75),
        round(max(values), 1),
    )


def aggregate_cells(observations: list[Observation], quantile: Any) -> list[MapCell]:
    groups: dict[tuple[str, str, str, str, str], list[Observation]] = defaultdict(list)
    for observation in observations:
        groups[
            (
                observation.profile,
                observation.model,
                observation.family,
                observation.configuration,
                observation.marker,
            )
        ].append(observation)

    pending: list[MapCell] = []
    for key in sorted(groups):
        profile, model, family, configuration, marker = key
        items = groups[key]
        instances: dict[str, tuple[float | None, float | None, bool]] = {}
        for item in items:
            value = (
                item.instance_seconds,
                item.instance_cost_usd,
                item.instance_success,
            )
            previous = instances.setdefault(item.instance_id, value)
            assert previous == value, f"instance accounting drift: {item.instance_id}"
        instance_values = list(instances.values())
        seconds = [value[0] for value in instance_values if value[0] is not None]
        costs = [value[1] for value in instance_values if value[1] is not None]
        mean_seconds = (
            statistics.fmean(seconds) if len(seconds) == len(instances) else None
        )
        mean_cost = statistics.fmean(costs) if len(costs) == len(instances) else None
        successful_instances = sum(value[2] for value in instance_values)
        expected_seconds = (
            sum(seconds) / successful_instances
            if len(seconds) == len(instances) and successful_instances
            else math.inf
            if len(seconds) == len(instances)
            else None
        )
        expected_cost = (
            sum(costs) / successful_instances
            if len(costs) == len(instances) and successful_instances
            else math.inf
            if len(costs) == len(instances)
            else None
        )
        scores = [score_contribution(item) for item in items]
        if len(items) < 3:
            plotted, reason = False, "n不足"
        elif mean_seconds is None:
            plotted, reason = False, "時間欠落"
        elif mean_seconds <= 0:
            plotted, reason = False, "所要非正"
        else:
            plotted, reason = True, "描画"
        pending.append(
            MapCell(
                point_id="",
                profile=profile,
                model=model,
                family=family,
                configuration=configuration,
                marker=marker,
                n=len(items),
                reached=sum(item.score is not None for item in items),
                full=sum(item.full for item in items),
                instance_count=len(instances),
                successful_instances=successful_instances,
                mean_seconds=mean_seconds,
                time_coverage=len(seconds),
                mean_score=round(statistics.fmean(scores), 2),
                five_number=_five_number(scores, quantile),
                mean_cost_usd=mean_cost,
                cost_coverage=len(costs),
                expected_seconds_per_success=expected_seconds,
                expected_cost_per_success=expected_cost,
                plotted=plotted,
                plot_reason=reason,
            )
        )
    return [
        MapCell(**{**cell.__dict__, "point_id": f"P{index:03d}"})
        for index, cell in enumerate(pending, start=1)
    ]


def _number(value: float | None, digits: int = 2) -> str:
    if value is None:
        return "N/A"
    if math.isinf(value):
        return "∞"
    return f"{value:.{digits}f}"


def _cost(value: float | None) -> str:
    if value is None:
        return "N/A"
    if math.isinf(value):
        return "∞"
    return f"${value:.6f}"


def _markdown_cell(value: str) -> str:
    return value.replace("|", "\\|").replace("\n", " ")


def _reading(cells: list[MapCell]) -> list[str]:
    index = {
        (cell.profile, cell.model, cell.family, cell.configuration): cell
        for cell in cells
    }
    cli_single = index[("cli", "gpt-5.6-luna", "filter", "single")]
    cli_bon = index[("cli", "gpt-5.6-luna", "filter", "bon:6")]
    ingest_list = index[("ingest", "gpt-5.6-luna", "list", "single")]
    ingest_table = index[("ingest", "gpt-5.6-luna", "table", "single")]
    local = next(
        cell
        for cell in cells
        if cell.profile == "nextjs"
        and cell.family == "Breakout"
        and cell.configuration == "bon:6"
    )
    return [
        (
            f"cli×Luna/filter は単発 {cli_single.mean_seconds:.1f}秒・"
            f"{cli_single.mean_score:.2f}点（n={cli_single.n}）に対し、bon:6 は"
            f"{cli_bon.mean_seconds:.1f}秒・{cli_bon.mean_score:.2f}点（n={cli_bon.n}）で、"
            "構成時間と全run平均の位置だけを示す。"
        ),
        (
            f"ingest×Luna単発は list {ingest_list.mean_seconds:.1f}秒・"
            f"{ingest_list.mean_score:.2f}点、table {ingest_table.mean_seconds:.1f}秒・"
            f"{ingest_table.mean_score:.2f}点（各n=3）で右上側に現れる。"
        ),
        (
            f"local Breakout bon:6 は {local.mean_seconds:.1f}秒・"
            f"{local.mean_score:.2f}点（n={local.n}, 構成instance={local.instance_count}）が初出で、"
            "1窓だけのため優劣は読まない。"
        ),
    ]


def build_markdown(
    band: Any, cells: list[MapCell], source_rows: list[tuple[str, str]]
) -> str:
    plotted = sum(cell.plotted for cell in cells)
    lines = [
        "<!-- GENERATED FILE: DO NOT EDIT. -->",
        "# Score/time map",
        "",
        f"- Schema: `{MAP_SCHEMA}`",
        f"- Regenerate: `{MAP_COMMAND}`",
        "- 手編集禁止。入力証跡または投影器を直して再生成する。既存band summaryは読み取り専用で、地図生成時に書き換えない。",
        "",
        "## 定義",
        "",
        "- 1点は `(model, profile/family, configuration)`。`single`、`bon:N`、`directive:roundN#hash`、`pack:id#hash`を別構成として混ぜない。",
        "- 横軸は構成instance総所要の算術平均。単発はrun所要、`bon:N`はN本を時分割実行したcampaign総所要で、SVGは対数軸。縦軸は正式run全数の平均到達寄与。",
        "- 到達済みは保存score、未到達はこの投影だけ0寄与とする。元の遡及vectorの`score=null`は不変で、`reached`分母を併記する。五数要約も同じ全run寄与に対する値。",
        "- `verdict_mapping`（菱形）は歴史final-only 251本とpost-seal最終判定写像、`checkpoint`（丸）はcheckpoint-capable 36本。両者を同一点へ混ぜない。",
        "- n<3は表に残して`n不足`、時間欠落も表に残して非描画。費用は構成instance平均を色とサイズへ写像し、欠測は灰色。",
        f"- 点数: {len(cells)}（描画 {plotted}、非描画 {len(cells) - plotted}）。run分母: {HISTORICAL_VECTOR_COUNT + SUPPLEMENTAL_RUN_COUNT}（遡及 {HISTORICAL_VECTOR_COUNT} + post-seal {SUPPLEMENTAL_RUN_COUNT}）。",
        "- 遡及Next.js 78本はaggregate-onlyでrun-level score/timeを同じ分母規律で復元できず、coverage gapとして非投影（n不足の点へ偽装しない）。",
        "",
        "![Score/time scatter](score_time_map.svg)",
        "",
        "## 読み",
        "",
    ]
    lines.extend(f"- {line}" for line in _reading(cells))
    lines.extend(
        [
            "",
            "## 正準数値表",
            "",
            "| ID | 描画 | model | use case | configuration | marker | n | reached | full | instances | mean config sec | time coverage | mean score | min | Q1 | median | Q3 | max | mean cost | cost coverage | Full meaning |",
            "|---|---|---|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|",
        ]
    )
    for cell in cells:
        five = [_number(value, 1) for value in cell.five_number]
        values = [
            cell.point_id,
            cell.plot_reason,
            cell.model,
            cell.use_case,
            cell.configuration,
            cell.marker,
            str(cell.n),
            str(cell.reached),
            str(cell.full),
            str(cell.instance_count),
            _number(cell.mean_seconds),
            f"{cell.time_coverage}/{cell.instance_count}",
            _number(cell.mean_score),
            *five,
            _cost(cell.mean_cost_usd),
            f"{cell.cost_coverage}/{cell.instance_count}",
            f"[{cell.profile}](#full-meaning-{cell.profile})",
        ]
        lines.append(
            "| " + " | ".join(_markdown_cell(value) for value in values) + " |"
        )
    lines.extend(
        [
            "",
            "## 成功1件あたり期待時間・費用",
            "",
            "成功は構成instance内にfullが1本以上。単発等は1 run=1 instance、bonは1 campaign=1 instance。観測成功率による記述値で、成功0件は`∞`、費用欠測は`N/A`。発散し得るためSVGには描かない。",
            "",
            "| ID | model | use case | configuration | success/instances | observed rate | expected sec/success | expected cost/success |",
            "|---|---|---|---|---:|---:|---:|---:|",
        ]
    )
    for cell in cells:
        rate = 100.0 * cell.successful_instances / cell.instance_count
        values = [
            cell.point_id,
            cell.model,
            cell.use_case,
            cell.configuration,
            f"{cell.successful_instances}/{cell.instance_count}",
            f"{rate:.2f}%",
            _number(cell.expected_seconds_per_success),
            _cost(cell.expected_cost_per_success),
        ]
        lines.append(
            "| " + " | ".join(_markdown_cell(value) for value in values) + " |"
        )
    lines.extend(["", "## Full meaning labels", ""])
    for profile, meaning in band.FULL_MEANING_LABELS.items():
        lines.extend(
            [
                f'<a id="full-meaning-{profile}"></a>',
                f"- `{profile}`: {meaning}",
            ]
        )
    lines.extend(["", "## 入力証跡", ""])
    for source, digest in sorted(set(source_rows)):
        lines.append(f"- `{source}` — `sha256:{digest}`")
    lines.append("")
    return "\n".join(lines)


def _cost_style(cell: MapCell, maximum_cost: float) -> tuple[str, float]:
    if cell.mean_cost_usd is None:
        return "#7a8494", 6.0
    fraction = min(cell.mean_cost_usd / maximum_cost, 1.0) if maximum_cost else 0.0
    red = round(46 + fraction * 194)
    green = round(160 - fraction * 76)
    blue = round(67 - fraction * 25)
    radius = 5.0 + 7.0 * math.sqrt(fraction)
    return f"#{red:02x}{green:02x}{blue:02x}", radius


def build_svg(cells: list[MapCell]) -> str:
    width, height = 1400, 900
    left, right, top, bottom = 105, 1340, 60, 760
    plotted = [cell for cell in cells if cell.plotted]
    assert plotted, "score/time map has no drawable points"
    times = [cell.mean_seconds for cell in plotted]
    assert all(value is not None and value > 0 for value in times)
    log_min = math.floor(math.log10(min(value for value in times if value is not None)))
    log_max = math.ceil(math.log10(max(value for value in times if value is not None)))
    if log_min == log_max:
        log_max += 1
    maximum_cost = max(
        (cell.mean_cost_usd or 0.0 for cell in plotted),
        default=0.0,
    )

    def x_position(seconds: float) -> float:
        return left + (math.log10(seconds) - log_min) / (log_max - log_min) * (
            right - left
        )

    def y_position(score: float) -> float:
        return bottom - score / 100.0 * (bottom - top)

    lines = [
        '<?xml version="1.0" encoding="UTF-8"?>',
        "<!-- GENERATED FILE: DO NOT EDIT. See the Markdown regeneration command. -->",
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">',
        "<style>text{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;fill:#1d2530}.grid{stroke:#d5dae1;stroke-width:1}.axis{stroke:#303844;stroke-width:2}.label{font-size:12px}.title{font-size:22px;font-weight:700}.note{font-size:13px}</style>",
        '<rect width="100%" height="100%" fill="#ffffff"/>',
        '<text x="105" y="32" class="title">Score/time map — all-formal-run attainment</text>',
    ]
    for score in range(0, 101, 20):
        y = y_position(float(score))
        lines.append(
            f'<line x1="{left}" y1="{y:.1f}" x2="{right}" y2="{y:.1f}" class="grid"/>'
        )
        lines.append(
            f'<text x="{left - 15}" y="{y + 4:.1f}" text-anchor="end" class="label">{score}</text>'
        )
    for exponent in range(log_min, log_max + 1):
        x = x_position(10.0**exponent)
        lines.append(
            f'<line x1="{x:.1f}" y1="{top}" x2="{x:.1f}" y2="{bottom}" class="grid"/>'
        )
        lines.append(
            f'<text x="{x:.1f}" y="{bottom + 23}" text-anchor="middle" class="label">10^{exponent}s</text>'
        )
    lines.extend(
        [
            f'<line x1="{left}" y1="{bottom}" x2="{right}" y2="{bottom}" class="axis"/>',
            f'<line x1="{left}" y1="{top}" x2="{left}" y2="{bottom}" class="axis"/>',
            f'<text x="{(left + right) / 2:.1f}" y="{bottom + 55}" text-anchor="middle" class="note">mean configuration total seconds (log10)</text>',
            f'<text x="25" y="{(top + bottom) / 2:.1f}" transform="rotate(-90 25 {(top + bottom) / 2:.1f})" text-anchor="middle" class="note">mean score contribution across all formal runs</text>',
        ]
    )
    reference_keys = {
        ("cli", "gpt-5.6-luna", "filter", "single"): (10.0, -12.0),
        ("cli", "gpt-5.6-luna", "filter", "bon:6"): (10.0, -8.0),
        ("ingest", "gpt-5.6-luna", "list", "single"): (10.0, 18.0),
        ("ingest", "gpt-5.6-luna", "table", "single"): (10.0, -10.0),
        (
            "nextjs",
            "qwen3.6:35b-a3b-coding-nvfp4",
            "Breakout",
            "bon:6",
        ): (10.0, -8.0),
    }
    for cell in plotted:
        assert cell.mean_seconds is not None
        x = x_position(cell.mean_seconds)
        y = y_position(cell.mean_score)
        fill, radius = _cost_style(cell, maximum_cost)
        tooltip = html.escape(
            f"{cell.point_id} | {cell.model} | {cell.use_case} | {cell.configuration} | "
            f"marker={cell.marker} | n={cell.n} reached={cell.reached} full={cell.full} | "
            f"config_seconds={cell.mean_seconds:.2f} score_mean={cell.mean_score:.2f} | "
            f"five_number={cell.five_number} | mean_cost={_cost(cell.mean_cost_usd)}"
        )
        if cell.marker == "checkpoint":
            shape = (
                f'<circle cx="{x:.1f}" cy="{y:.1f}" r="{radius:.1f}" '
                f'fill="{fill}" fill-opacity="0.78" stroke="#17202b" '
                f'stroke-width="1.5"><title>{tooltip}</title></circle>'
            )
        else:
            points = " ".join(
                [
                    f"{x:.1f},{y - radius:.1f}",
                    f"{x + radius:.1f},{y:.1f}",
                    f"{x:.1f},{y + radius:.1f}",
                    f"{x - radius:.1f},{y:.1f}",
                ]
            )
            shape = (
                f'<polygon points="{points}" fill="{fill}" fill-opacity="0.78" '
                f'stroke="#17202b" stroke-width="1.5"><title>{tooltip}</title></polygon>'
            )
        lines.append(shape)
        label_offset = reference_keys.get(
            (cell.profile, cell.model, cell.family, cell.configuration)
        )
        if label_offset is not None:
            label_x = x + radius + label_offset[0]
            label_y = y + label_offset[1]
            lines.append(
                f'<text x="{label_x:.1f}" y="{label_y:.1f}" '
                f'class="label">{cell.point_id}</text>'
            )
    lines.extend(
        [
            '<circle cx="110" cy="840" r="7" fill="#7a8494" stroke="#17202b"/><text x="125" y="844" class="note">checkpoint</text>',
            '<polygon points="255,833 262,840 255,847 248,840" fill="#7a8494" stroke="#17202b"/><text x="270" y="844" class="note">verdict mapping / final-only</text>',
            '<circle cx="500" cy="840" r="6" fill="#7a8494"/><text x="515" y="844" class="note">cost missing</text>',
            '<circle cx="670" cy="840" r="10" fill="#e0552d"/><text x="687" y="844" class="note">larger/redder = higher observed config cost</text>',
            '<text x="105" y="875" class="note">Labels mark the five reading-reference points; every point tooltip carries its table ID. n&lt;3 and incomplete-time rows are absent.</text>',
            "</svg>",
            "",
        ]
    )
    return "\n".join(lines)


def build_score_time_map(band: Any) -> tuple[str, str, list[MapCell]]:
    observations, source_rows = collect_observations(band)
    cells = aggregate_cells(observations, band.score_quantile)
    return build_markdown(band, cells, source_rows), build_svg(cells), cells


def write_score_time_map(
    band: Any,
    markdown_path: Path | None = None,
    svg_path: Path | None = None,
) -> tuple[str, str, list[MapCell]]:
    markdown, svg, cells = build_score_time_map(band)
    markdown_output = markdown_path or band.RUNS_DIR / "score_time_map.md"
    svg_output = svg_path or band.RUNS_DIR / "score_time_map.svg"
    markdown_output.write_text(markdown, encoding="utf-8")
    svg_output.write_text(svg, encoding="utf-8")
    return markdown, svg, cells
