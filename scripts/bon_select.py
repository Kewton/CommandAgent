#!/usr/bin/env python3
"""Select one completed BoN-0 run without prediction, pruning, or repair.

The selector consumes only persisted campaign evidence.  It first validates
that the six runs are independent repetitions of one pinned task.  An earned
full is selected ahead of every non-full run; multiple fulls are ordered by
fixed F-1 score, elapsed seconds, calculated cost, then run name.  If no run
earned full, the same deterministic order identifies a most-promising loser
for reporting only.  BoN-0 never dispatches a repair.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import statistics
import sys
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import tomllib
from score_retrospective import AtomObservation, normalize_state, score_atoms

SCHEMA_VERSION = "commandagent.bon-selection/v0"
EXPECTED_SUITE_ID = "cli-filter-bon0"
EXPECTED_N = 6
EXPECTED_GOAL = "filter"
EXPECTED_EXECUTOR = "gpt-5.6-luna"
REGISTERED_ATOMS = (
    "cli_probe",
    "help_binding",
    "cli_output_claims",
    "cli_rerun_consistency",
)
BASELINE_FIELDS = (
    "profile",
    "intent",
    "plan_preset",
    "workspace_mode",
    "context_budget",
    "planner_model",
    "planner_provider",
    "provider",
    "api",
    "tool_protocol",
    "min_head",
)
LUNA_PRICE_PER_MILLION = {
    "uncached_input": 1.0,
    "cached_input": 0.1,
    "output": 6.0,
}


class SelectionError(ValueError):
    """Raised when campaign evidence cannot be interpreted honestly."""


@dataclass(frozen=True)
class RunObservation:
    name: str
    earned_full: bool
    vector: dict[str, Any]
    duration_seconds: int
    cost_usd: float
    input_tokens: int
    cached_input_tokens: int
    output_tokens: int
    reasoning_tokens: int
    provider_turns: int
    native_tool_calls: int
    identity: dict[str, Any]
    evidence: dict[str, Any]


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def canonical_sha256(value: Any) -> str:
    return sha256_bytes(
        json.dumps(
            value, ensure_ascii=False, sort_keys=True, separators=(",", ":")
        ).encode()
    )


def load_json_object(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise SelectionError(f"cannot read JSON object {path}: {error}") from error
    if not isinstance(value, dict):
        raise SelectionError(f"JSON root must be an object: {path}")
    return value


def load_toml(path: Path) -> dict[str, Any]:
    try:
        with path.open("rb") as handle:
            value = tomllib.load(handle)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise SelectionError(f"cannot read TOML {path}: {error}") from error
    if not isinstance(value, dict):
        raise SelectionError(f"TOML root must be a table: {path}")
    return value


def validate_suite(
    suite_path: Path, baseline_path: Path
) -> tuple[dict[str, Any], dict[str, Any], list[str]]:
    document = load_toml(suite_path)
    baseline = load_toml(baseline_path)
    suite = document.get("suite")
    goals = document.get("goals")
    runs = document.get("runs")
    baseline_suite = baseline.get("suite")
    baseline_goals = baseline.get("goals")
    if not isinstance(suite, dict) or not isinstance(goals, dict):
        raise SelectionError("BoN suite requires [suite] and [goals]")
    if not isinstance(runs, list):
        raise SelectionError("BoN suite requires [[runs]]")
    if not isinstance(baseline_suite, dict) or not isinstance(baseline_goals, dict):
        raise SelectionError("baseline suite requires [suite] and [goals]")

    reasons: list[str] = []
    if suite.get("id") != EXPECTED_SUITE_ID:
        reasons.append("suite_id_mismatch")
    if set(goals) != {EXPECTED_GOAL}:
        reasons.append("mixed_or_missing_goal")
    if len(runs) != EXPECTED_N:
        reasons.append("run_count_not_six")
    names: list[str] = []
    for index, run in enumerate(runs):
        if not isinstance(run, dict):
            reasons.append(f"run_{index + 1}_not_table")
            continue
        name = str(run.get("name") or "")
        names.append(name)
        if run.get("goal") != EXPECTED_GOAL:
            reasons.append(f"{name or index + 1}:goal_mismatch")
        if run.get("executor") != EXPECTED_EXECUTOR:
            reasons.append(f"{name or index + 1}:executor_mismatch")
        if set(run) != {"name", "goal", "executor"}:
            reasons.append(f"{name or index + 1}:run_shape_mismatch")
    if len(names) != len(set(names)):
        reasons.append("duplicate_run_name")
    if goals.get(EXPECTED_GOAL) != baseline_goals.get(EXPECTED_GOAL):
        reasons.append("baseline_goal_bytes_mismatch")
    for field in BASELINE_FIELDS:
        if suite.get(field) != baseline_suite.get(field):
            reasons.append(f"baseline_{field}_mismatch")
    return document, baseline, reasons


def read_events(artifact: Path) -> list[dict[str, Any]]:
    paths = sorted(artifact.glob(".anvil/runs/**/events.jsonl"))
    if not paths:
        raise SelectionError(f"events evidence missing: {artifact}")
    events: list[dict[str, Any]] = []
    for path in paths:
        for line_number, line in enumerate(
            path.read_text(encoding="utf-8", errors="replace").splitlines(), start=1
        ):
            try:
                event = json.loads(line)
            except json.JSONDecodeError as error:
                raise SelectionError(
                    f"invalid event JSON: {path}:{line_number}: {error}"
                ) from error
            if isinstance(event, dict):
                events.append(event)
    return events


def run_stop(events: list[dict[str, Any]]) -> dict[str, Any]:
    stops = [event for event in events if event.get("event") == "run_stop"]
    if not stops:
        raise SelectionError("run_stop evidence missing")
    return stops[-1]


def score_vector(artifact: Path) -> dict[str, Any]:
    assurance_path = artifact / "evidence" / "cli-assurance.json"
    assurance = load_json_object(assurance_path) if assurance_path.is_file() else {}
    evidence = assurance.get("evidence")
    checks = evidence.get("checks") if isinstance(evidence, dict) else None
    atoms: dict[str, AtomObservation] = {}
    for atom in REGISTERED_ATOMS:
        raw = checks.get(atom) if isinstance(checks, dict) else None
        atoms[atom] = AtomObservation(normalize_state(raw), str(assurance_path))
    calculated = score_atoms(atoms)
    calculated["atoms"] = [
        {"key": atom, "state": atoms[atom].state} for atom in REGISTERED_ATOMS
    ]
    return calculated


def executor_turns(events: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return [
        event
        for event in events
        if event.get("event") == "provider_turn_duration"
        and event.get("caller_scope") == "executor"
        and event.get("provider") == "openai"
    ]


def usage(turns: list[dict[str, Any]]) -> dict[str, Any]:
    input_tokens = sum(int(turn.get("prompt_eval_count") or 0) for turn in turns)
    cached = sum(int(turn.get("provider_cached_input_tokens") or 0) for turn in turns)
    output = sum(int(turn.get("eval_count") or 0) for turn in turns)
    reasoning = sum(int(turn.get("provider_reasoning_tokens") or 0) for turn in turns)
    uncached = input_tokens - cached
    if uncached < 0:
        raise SelectionError("cached input exceeds total input")
    cost = (
        uncached * LUNA_PRICE_PER_MILLION["uncached_input"]
        + cached * LUNA_PRICE_PER_MILLION["cached_input"]
        + output * LUNA_PRICE_PER_MILLION["output"]
    ) / 1_000_000
    return {
        "input_tokens": input_tokens,
        "cached_input_tokens": cached,
        "output_tokens": output,
        "reasoning_tokens": reasoning,
        "cost_usd": round(cost, 7),
    }


def native_call_count(events: list[dict[str, Any]]) -> int:
    return sum(
        int(event.get("tool_calls") or 0)
        for event in events
        if event.get("event") == "provider_response"
        and event.get("provider") == "openai"
        and event.get("model") == EXPECTED_EXECUTOR
    )


def model_identity(turns: list[dict[str, Any]]) -> dict[str, Any]:
    requested = sorted({str(turn.get("model")) for turn in turns if turn.get("model")})
    returned = sorted(
        {
            str(turn.get("provider_model_id"))
            for turn in turns
            if turn.get("provider_model_id")
        }
    )
    tiers = sorted(
        {
            str(turn.get("provider_service_tier"))
            for turn in turns
            if turn.get("provider_service_tier")
        }
    )
    fingerprints = sorted(
        {
            "null"
            if turn.get("system_fingerprint") is None
            else str(turn["system_fingerprint"])
            for turn in turns
        }
    )
    return {
        "provider_turns": len(turns),
        "requested_models": requested,
        "returned_models": returned,
        "service_tiers": tiers,
        "system_fingerprints": fingerprints,
        "matches_requested_executor": bool(turns)
        and requested == [EXPECTED_EXECUTOR]
        and returned == [EXPECTED_EXECUTOR],
    }


def evidence_inventory(artifact: Path) -> dict[str, Any]:
    files = sorted(path for path in artifact.rglob("*") if path.is_file())
    event_files = [path for path in files if path.name == "events.jsonl"]
    return {
        "preserved": artifact.is_dir() and bool(event_files),
        "file_count": len(files),
        "event_files": len(event_files),
        "acceptance_sheet": (artifact / "acceptance-sheet.md").is_file(),
    }


def observe_run(
    campaign: Path,
    metadata_run: dict[str, Any],
    binary_sha256: str,
    pack_pin: dict[str, Any] | None,
) -> RunObservation:
    name = str(metadata_run.get("name") or "")
    artifact = campaign / "artifacts" / name
    events = read_events(artifact)
    stop = run_stop(events)
    turns = executor_turns(events)
    measured_usage = usage(turns)
    expected_inputs = metadata_run.get("input_sha256_expected")
    observed_inputs = metadata_run.get("input_sha256_observed")
    input_pin = {"expected": expected_inputs, "observed": observed_inputs}
    identity = {
        "binary_sha256": binary_sha256,
        "input_pin_sha256": canonical_sha256(input_pin),
        "input_expected_observed_equal": expected_inputs == observed_inputs,
        "pack_pin": metadata_run.get("pack"),
        "pack_pin_matches_suite": metadata_run.get("pack") == pack_pin,
        "model": model_identity(turns),
    }
    vector = score_vector(artifact)
    earned_full = (
        stop.get("final_acceptance_status") == "full_success"
        and stop.get("assurance_level") == "full"
        and stop.get("ok") is True
    )
    if earned_full and vector.get("score") != 100.0:
        raise SelectionError(f"{name}: earned full without score 100")
    return RunObservation(
        name=name,
        earned_full=earned_full,
        vector=vector,
        duration_seconds=int(metadata_run.get("duration_seconds") or 0),
        cost_usd=float(measured_usage["cost_usd"]),
        input_tokens=int(measured_usage["input_tokens"]),
        cached_input_tokens=int(measured_usage["cached_input_tokens"]),
        output_tokens=int(measured_usage["output_tokens"]),
        reasoning_tokens=int(measured_usage["reasoning_tokens"]),
        provider_turns=len(turns),
        native_tool_calls=native_call_count(events),
        identity=identity,
        evidence=evidence_inventory(artifact),
    )


def selection_key(run: RunObservation) -> tuple[float, int, float, str]:
    score = run.vector.get("score")
    return (
        -(float(score) if isinstance(score, (int, float)) else float("-inf")),
        run.duration_seconds,
        run.cost_usd,
        run.name,
    )


def five_number(values: list[float]) -> dict[str, float] | None:
    if not values:
        return None
    ordered = sorted(values)

    def quantile(fraction: float) -> float:
        position = (len(ordered) - 1) * fraction
        lower = int(position)
        upper = min(lower + 1, len(ordered) - 1)
        return round(
            ordered[lower] + (ordered[upper] - ordered[lower]) * (position - lower),
            1,
        )

    return {
        "min": round(ordered[0], 1),
        "q1": quantile(0.25),
        "median": round(float(statistics.median(ordered)), 1),
        "q3": quantile(0.75),
        "max": round(ordered[-1], 1),
    }


def calibration_counts(campaign: Path, calibration_root: Path) -> dict[str, Any]:
    per_run: dict[str, int] = {}
    campaign_total = 0
    for records_path in sorted(calibration_root.glob("*/records.jsonl")):
        for line in records_path.read_text(encoding="utf-8").splitlines():
            try:
                record = json.loads(line)
            except json.JSONDecodeError:
                continue
            source = str(record.get("source_run") or "")
            if campaign.name not in source:
                continue
            campaign_total += 1
            for run_dir in (campaign / "artifacts").iterdir():
                if run_dir.name in source:
                    per_run[run_dir.name] = per_run.get(run_dir.name, 0) + 1
                    break
    return {"campaign_records": campaign_total, "by_run": per_run}


def build_selection(
    campaign: Path,
    suite_path: Path,
    baseline_path: Path,
    calibration_root: Path,
) -> dict[str, Any]:
    suite_document, baseline_document, invalid = validate_suite(
        suite_path, baseline_path
    )
    metadata = load_json_object(campaign / "uat-meta.json")
    baseline_suite = baseline_document["suite"]
    metadata_suite = metadata.get("suite")
    metadata_runs = metadata.get("runs")
    if not isinstance(metadata_suite, dict) or not isinstance(metadata_runs, list):
        raise SelectionError("campaign metadata suite/runs are missing")
    if metadata_suite.get("sha256") != sha256_file(suite_path):
        invalid.append("campaign_suite_sha256_mismatch")
    expected_names = [str(run.get("name") or "") for run in suite_document["runs"]]
    if [str(run.get("name") or "") for run in metadata_runs] != expected_names:
        invalid.append("campaign_run_matrix_mismatch")
    if any(run.get("status") != "completed" for run in metadata_runs):
        invalid.append("non_completed_harness_run")
    binary = metadata.get("preflight", {}).get("binary_sha256", {})
    built_binary = str(binary.get("built") or "") if isinstance(binary, dict) else ""
    installed_binary = (
        str(binary.get("installed") or "") if isinstance(binary, dict) else ""
    )
    if not built_binary or built_binary != installed_binary:
        invalid.append("binary_hash_mismatch")
    pack_pin = metadata_suite.get("pack")
    if not isinstance(pack_pin, dict):
        pack_pin = None

    observations = [
        observe_run(campaign, run, built_binary, pack_pin) for run in metadata_runs
    ]
    input_pins = {run.identity["input_pin_sha256"] for run in observations}
    binaries = {run.identity["binary_sha256"] for run in observations}
    packs = {canonical_sha256(run.identity["pack_pin"]) for run in observations}
    model_signatures = {
        canonical_sha256(
            {
                "requested_models": run.identity["model"]["requested_models"],
                "returned_models": run.identity["model"]["returned_models"],
                "service_tiers": run.identity["model"]["service_tiers"],
                "system_fingerprints": run.identity["model"]["system_fingerprints"],
            }
        )
        for run in observations
    }
    if len(input_pins) != 1 or not all(
        run.identity["input_expected_observed_equal"] for run in observations
    ):
        invalid.append("input_pin_mismatch")
    if len(binaries) != 1:
        invalid.append("run_binary_mismatch")
    if len(packs) != 1 or not all(
        run.identity["pack_pin_matches_suite"] for run in observations
    ):
        invalid.append("pack_pin_mismatch")
    if len(model_signatures) != 1 or not all(
        run.identity["model"]["matches_requested_executor"] for run in observations
    ):
        invalid.append("model_metadata_mismatch")

    fulls = sorted((run for run in observations if run.earned_full), key=selection_key)
    candidates = fulls or sorted(observations, key=selection_key)
    selected = candidates[0] if not invalid else None
    selection_kind = (
        "adopted_full"
        if selected is not None and selected.earned_full
        else "most_promising_loser"
        if selected is not None
        else "invalid_measurement"
    )
    calibration = calibration_counts(campaign, calibration_root)
    nonselected = [
        run for run in observations if selected is None or run.name != selected.name
    ]
    scores = [
        float(run.vector["score"])
        for run in observations
        if isinstance(run.vector.get("score"), (int, float))
    ]
    run_rows = []
    for run in observations:
        run_rows.append(
            {
                "name": run.name,
                "earned_full": run.earned_full,
                "selected": selected is not None and run.name == selected.name,
                "score_vector": run.vector,
                "duration_seconds": run.duration_seconds,
                "cost_usd": run.cost_usd,
                "usage": {
                    "input_tokens": run.input_tokens,
                    "cached_input_tokens": run.cached_input_tokens,
                    "output_tokens": run.output_tokens,
                    "reasoning_tokens": run.reasoning_tokens,
                },
                "provider_turns": run.provider_turns,
                "native_tool_calls": run.native_tool_calls,
                "identity": run.identity,
                "evidence": run.evidence,
                "calibration_records": calibration["by_run"].get(run.name, 0),
            }
        )
    return {
        "schema_version": SCHEMA_VERSION,
        "campaign_id": metadata.get("campaign_id"),
        "valid_measurement": not invalid,
        "invalid_reasons": sorted(set(invalid)),
        "definition": {
            "configuration": "bon:6",
            "single_goal": EXPECTED_GOAL,
            "independent_workspaces": True,
            "pruning": False,
            "prediction": False,
            "repair_connected": False,
        },
        "baseline": {
            "suite_id": baseline_suite.get("id"),
            "suite_sha256": sha256_file(baseline_path),
            "goal_sha256": sha256_bytes(
                baseline_document["goals"][EXPECTED_GOAL].encode()
            ),
            "fields": {field: baseline_suite.get(field) for field in BASELINE_FIELDS},
        },
        "identity": {
            "all_equal": not any(reason.endswith("mismatch") for reason in invalid),
            "binary_sha256": built_binary,
            "input_pin_sha256": next(iter(input_pins), None),
            "pack_pin": pack_pin,
            "model_drift_probe": "requested/returned model, service tier, system fingerprint",
        },
        "pricing": {
            "basis": "Luna standard rates fixed by the 2026-08-02 F-2 campaign",
            "usd_per_million_tokens": LUNA_PRICE_PER_MILLION,
        },
        "selection": {
            "kind": selection_kind,
            "run": selected.name if selected is not None else None,
            "tie_break": [
                "earned_full_desc",
                "reached_score_desc",
                "duration_seconds_asc",
                "cost_usd_asc",
                "run_name_asc",
            ],
            "repair_connected": False,
        },
        "summary": {
            "runs": len(observations),
            "earned_full": len(fulls),
            "reached": len(scores),
            "score_five_number": five_number(scores),
            "duration_seconds_total": sum(run.duration_seconds for run in observations),
            "cost_usd_total": round(sum(run.cost_usd for run in observations), 7),
        },
        "retention": {
            "nonselected_runs": len(nonselected),
            "nonselected_evidence_preserved": sum(
                run.evidence["preserved"] for run in nonselected
            ),
            "calibration_records_appended": calibration["campaign_records"],
        },
        "runs": run_rows,
    }


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--campaign", required=True, type=Path)
    result.add_argument("--suite", required=True, type=Path)
    result.add_argument("--baseline-suite", required=True, type=Path)
    result.add_argument("--calibration-root", required=True, type=Path)
    result.add_argument("--output", type=Path)
    return result


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        result = build_selection(
            args.campaign.resolve(),
            args.suite.resolve(),
            args.baseline_suite.resolve(),
            args.calibration_root.resolve(),
        )
    except SelectionError as error:
        print(f"bon-select: {error}", file=sys.stderr)
        return 2
    rendered = json.dumps(result, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    if args.output is not None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(rendered, encoding="utf-8")
    else:
        print(rendered, end="")
    return 0 if result["valid_measurement"] else 3


if __name__ == "__main__":
    raise SystemExit(main())
