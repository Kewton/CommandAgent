#!/usr/bin/env python3
"""Run the fixed F-1 score retrospective over repository-managed history.

The scanner reuses the immutable band adapters as its run inventory and reads
only their typed final evidence. A checkpoint is emitted only when an atom has
an existing integer ``epoch``; file order and mtimes are never promoted to a
clock. The fixed score coefficients are pass=1, absent=0, violation=-1/2.

Full execution writes four deterministic files below a new run directory:

* ``final-vectors.jsonl``
* ``checkpoint-vectors.jsonl``
* ``study-summary.json``
* ``report.md``

The legacy F-1a inventory-only dry-run remains available for its committed
sample. Full execution must be selected explicitly with ``--execute``.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib
import json
import math
import re
import statistics
import sys
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

PLAN_SCHEMA_VERSION = "commandagent.score-retrospective-plan/v0"
STUDY_SCHEMA_VERSION = "commandagent.score-retrospective/v0"
VECTOR_SCHEMA_VERSION = "commandagent.score-vector/v0"
PENDING_ADJUDICATION = "F-1a score institution review adjudication"
STUDY_ID = "f1-retrospective-001"
FIXED_DATE = "2026-08-02"

REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_RUNS_ROOT = REPOSITORY_ROOT / "workspace" / "management" / "runs"
DEFAULT_OUTPUT_DIR = DEFAULT_RUNS_ROOT / STUDY_ID
BAND_SCRIPT_DIR = REPOSITORY_ROOT / "workspace" / "management" / "scripts"

STATE_COEFFICIENT_TWICE = {
    "pass": 2,
    "absent": 0,
    "violation": -1,
    "unobserved": 0,
}
OBSERVED_STATES = {"pass", "absent", "violation"}


class InputError(ValueError):
    """Raised when historical evidence cannot be scanned honestly."""


@dataclass(frozen=True)
class AtomObservation:
    """One registered atom state and its immutable source."""

    state: str
    source_ref: str


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(
        description="Run or dry-plan the fixed F-1 score retrospective."
    )
    mode = result.add_mutually_exclusive_group(required=True)
    mode.add_argument(
        "--execute",
        action="store_true",
        help="scan repository history and write the fixed study outputs",
    )
    mode.add_argument(
        "--dry-run",
        action="store_true",
        help="emit the preserved F-1a single-campaign inventory plan",
    )
    result.add_argument(
        "--campaign-summary",
        type=Path,
        help="campaign-summary.json required by the legacy dry-run mode",
    )
    result.add_argument(
        "--events-root",
        type=Path,
        help="legacy dry-run field; recorded without being read",
    )
    result.add_argument(
        "--score-config",
        type=Path,
        help="legacy dry-run field; recorded without being read",
    )
    result.add_argument(
        "--runs-root",
        type=Path,
        default=DEFAULT_RUNS_ROOT,
        help="repository-managed historical runs root",
    )
    result.add_argument(
        "--output-dir",
        type=Path,
        default=DEFAULT_OUTPUT_DIR,
        help="new study directory below --runs-root",
    )
    result.add_argument(
        "--overwrite",
        action="store_true",
        help="replace only the four generated study files in an existing output dir",
    )
    return result


# ---------------------------------------------------------------------------
# Preserved F-1a dry-run API


def load_campaign_summary(path: Path) -> dict[str, Any]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise InputError(f"campaign summary not found: {path}") from exc
    except json.JSONDecodeError as exc:
        raise InputError(f"campaign summary is not valid JSON: {path}: {exc}") from exc

    if not isinstance(document, dict):
        raise InputError("campaign summary root must be an object")
    for field in ("schema_version", "uat_id", "campaign_id", "revision", "runs"):
        if field not in document:
            raise InputError(f"campaign summary is missing required field: {field}")
    if not isinstance(document["runs"], list) or not document["runs"]:
        raise InputError("campaign summary runs must be a non-empty array")
    return document


def event_hashes(document: dict[str, Any]) -> dict[str, str]:
    source_hashes = document.get("source_hashes")
    if not isinstance(source_hashes, dict):
        raise InputError("campaign summary source_hashes must be an object")
    hashes = source_hashes.get("live_run_events_sha256")
    if not isinstance(hashes, dict):
        raise InputError("campaign summary must inventory live_run_events_sha256")
    if not all(
        isinstance(key, str) and isinstance(value, str) for key, value in hashes.items()
    ):
        raise InputError("live_run_events_sha256 must map run names to hashes")
    return hashes


def display_path(path: Path | None, cwd: Path) -> str | None:
    if path is None:
        return None
    absolute = path.resolve()
    try:
        return absolute.relative_to(cwd.resolve()).as_posix()
    except ValueError:
        return absolute.as_posix()


def run_inventory(document: dict[str, Any]) -> list[dict[str, Any]]:
    hashes = event_hashes(document)
    inventory: list[dict[str, Any]] = []
    seen: set[str] = set()
    for index, run in enumerate(document["runs"]):
        if not isinstance(run, dict):
            raise InputError(f"runs[{index}] must be an object")
        name = run.get("name")
        if not isinstance(name, str) or not name:
            raise InputError(f"runs[{index}].name must be a non-empty string")
        if name in seen:
            raise InputError(f"duplicate run name: {name}")
        seen.add(name)
        digest = hashes.get(name)
        if digest is None:
            raise InputError(f"run is missing an events sha256 inventory entry: {name}")
        if len(digest) != 64 or any(
            character not in "0123456789abcdef" for character in digest
        ):
            raise InputError(f"run has an invalid events sha256: {name}")
        inventory.append(
            {
                "run_id": name,
                "family": run.get("family"),
                "executor": run.get("executor"),
                "expected_events_sha256": digest,
            }
        )
    unexpected = sorted(set(hashes) - seen)
    if unexpected:
        raise InputError(
            f"events sha256 inventory has unknown runs: {', '.join(unexpected)}"
        )
    return inventory


def build_plan(
    summary_path: Path,
    document: dict[str, Any],
    *,
    events_root: Path | None = None,
    score_config: Path | None = None,
    cwd: Path | None = None,
) -> dict[str, Any]:
    base = cwd or Path.cwd()
    inventory = run_inventory(document)
    return {
        "schema_version": PLAN_SCHEMA_VERSION,
        "mode": "dry-run",
        "study_status": "not_executed_pending_adjudication",
        "campaign": {
            "uat_id": document["uat_id"],
            "campaign_id": document["campaign_id"],
            "revision": document["revision"],
        },
        "inputs": {
            "campaign_summary": display_path(summary_path, base),
            "events_root": display_path(events_root, base),
            "score_config": display_path(score_config, base),
        },
        "inventory": {
            "run_count": len(inventory),
            "event_stream_count": len(inventory),
            "runs": inventory,
        },
        "planned_read_set": [
            "one immutable events.jsonl per inventoried run",
            "only evidence files referenced by registered atom producer events",
            "the adjudicated eval.yaml score declaration and registry snapshot",
        ],
        "planned_outputs": [
            "checkpoint-vectors.jsonl",
            "final-vectors.jsonl",
            "study-summary.json",
        ],
        "guards": {
            "historical_files_mutated": False,
            "event_scan_performed": False,
            "evidence_scan_performed": False,
            "score_computed": False,
            "correlation_computed": False,
            "new_judges": 0,
        },
        "blocked_until": PENDING_ADJUDICATION,
    }


# ---------------------------------------------------------------------------
# Fixed retrospective


def relative_source(path: Path, line: int | None = None) -> str:
    try:
        label = path.resolve().relative_to(REPOSITORY_ROOT).as_posix()
    except ValueError:
        label = path.resolve().as_posix()
    return f"{label}:{line}" if line is not None else label


def normalize_state(value: Any) -> str:
    normalized = str(value or "").strip().lower().replace("-", "_")
    if normalized in {"pass", "passed", "success", "full", "complete", "completed"}:
        return "pass"
    if normalized in {
        "absent",
        "claims_absent",
        "inconclusive",
        "not_applicable",
        "unavailable",
    }:
        return "absent"
    if normalized in {
        "fail",
        "failed",
        "failure",
        "violation",
        "violated",
        "mismatch",
        "false",
    }:
        return "violation"
    if normalized in {
        "",
        "not_executed",
        "not_reached",
        "unobserved",
        "unknown",
        "—",
    }:
        return "unobserved"
    raise InputError(f"unsupported historical atom state: {value!r}")


def score_atoms(atoms: dict[str, AtomObservation]) -> dict[str, Any]:
    if not atoms:
        raise InputError("score vector must contain at least one registered atom")
    invalid = sorted(
        {observation.state for observation in atoms.values()}
        - set(STATE_COEFFICIENT_TWICE)
    )
    if invalid:
        raise InputError(f"invalid score states: {', '.join(invalid)}")
    numerator_twice = sum(
        STATE_COEFFICIENT_TWICE[observation.state] for observation in atoms.values()
    )
    observed_weight = sum(
        observation.state in OBSERVED_STATES for observation in atoms.values()
    )
    weight_sum = len(atoms)
    reached = observed_weight > 0
    score = round(50 * numerator_twice / weight_sum, 1) if reached else None
    return {
        "reached": reached,
        "score": score,
        "weighted_state_sum_twice": numerator_twice,
        "weight_sum": weight_sum,
        "observed_weight": observed_weight,
    }


def model_tier(model: str) -> str:
    normalized = model.strip().lower()
    if normalized.startswith("gpt-"):
        return "frontier_reasoning"
    if normalized.endswith("-cloud"):
        return "cloud"
    if normalized and normalized not in {"unknown", "workflow"}:
        return "local"
    return "unrecorded"


def vector(
    *,
    profile: str,
    set_id: str,
    run_id: str,
    family: str,
    model: str,
    final_full: bool,
    final_assurance: str,
    atoms: dict[str, AtomObservation],
    checkpoint_epoch: int | None = None,
    checkpoint_ordinal: int | None = None,
) -> dict[str, Any]:
    result: dict[str, Any] = {
        "schema_version": VECTOR_SCHEMA_VERSION,
        "run_id": f"{set_id}/{run_id}",
        "profile": profile,
        "family": family,
        "model": model,
        "model_tier": model_tier(model),
        "final_verdict": "full" if final_full else "non_full",
        "final_assurance": final_assurance,
        **score_atoms(atoms),
        "atoms": [
            {
                "key": key,
                "state": observation.state,
                "source_ref": observation.source_ref,
            }
            for key, observation in atoms.items()
        ],
    }
    if checkpoint_epoch is not None:
        result["checkpoint_epoch"] = checkpoint_epoch
    if checkpoint_ordinal is not None:
        result["checkpoint_ordinal"] = checkpoint_ordinal
    return result


def json_dict(path: Path) -> dict[str, Any] | None:
    if not path.is_file():
        return None
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise InputError(f"invalid historical JSON: {path}: {exc}") from exc
    if not isinstance(value, dict):
        raise InputError(f"historical JSON root is not an object: {path}")
    return value


def observation_from_json(path: Path) -> AtomObservation:
    document = json_dict(path)
    if document is None:
        return AtomObservation("unobserved", relative_source(path))
    if document.get("ok") is True:
        state = "pass"
    elif document.get("ok") is False:
        state = "violation"
    else:
        state = normalize_state(document.get("status") or document.get("assurance"))
    return AtomObservation(state, relative_source(path))


def read_jsonl(path: Path) -> list[tuple[int, dict[str, Any]]]:
    if not path.is_file():
        return []
    events: list[tuple[int, dict[str, Any]]] = []
    for line_number, line in enumerate(
        path.read_text(encoding="utf-8", errors="replace").splitlines(), 1
    ):
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(event, dict):
            events.append((line_number, event))
    return events


def load_band_module() -> Any:
    path = str(BAND_SCRIPT_DIR)
    if path not in sys.path:
        sys.path.insert(0, path)
    return importlib.import_module("band_aggregate")


def scan_data(module: Any) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    records = module.discover_data_records()[0]
    module.assert_full_data_evidence(records)
    finals: list[dict[str, Any]] = []
    for record in records:
        evidence = record.evidence_dir
        paths = {
            "data_reconciliation": (evidence / "reconciliation.json")
            if evidence
            else Path("<missing>"),
            "data_claims_binding": (evidence / "claims-binding.json")
            if evidence
            else Path("<missing>"),
            "data_rerun_consistency": (evidence / "rerun-consistency.json")
            if evidence
            else Path("<missing>"),
            "data_results_schema": (evidence / "results-schema.json")
            if evidence
            else Path("<missing>"),
        }
        atoms = {key: observation_from_json(path) for key, path in paths.items()}
        finals.append(
            vector(
                profile="data",
                set_id=record.set_id,
                run_id=record.run_name,
                family=record.family,
                model=record.executor,
                final_full=record.is_full,
                final_assurance=record.assurance,
                atoms=atoms,
            )
        )
    return finals, []


FIX_ATOMS = ("before_fails", "after_passes", "no_regression")


def fix_event_path(module: Any, record: Any) -> Path | None:
    return module.events_path_for_run(
        module.RUNS_DIR / record.set_id,
        record.run_name,
        record.event_run_id,
    )


def fix_probe_state(event: dict[str, Any]) -> str:
    if event.get("executed") is not True:
        return "unobserved"
    expected = str(event.get("expected_polarity") or event.get("expected") or "")
    outcome = str(event.get("outcome") or "")
    if (expected == "failure" and outcome == "failure") or (
        expected == "success" and outcome == "success"
    ):
        return "pass"
    return "violation"


def scan_fix(module: Any) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    records = module.discover_fix_records()[0]
    module.assert_full_fix_evidence(records)
    finals: list[dict[str, Any]] = []
    checkpoints: list[dict[str, Any]] = []
    for record in records:
        events_path = fix_event_path(module, record)
        events = read_jsonl(events_path) if events_path is not None else []
        final_statuses: dict[str, Any] = {}
        final_source = (
            relative_source(events_path) if events_path is not None else "missing"
        )
        for line_number, event in events:
            if (
                event.get("event") == "ultra_final_acceptance"
                and event.get("fix_run_id") == record.fix_run_id
                and isinstance(event.get("requirement_statuses"), dict)
            ):
                final_statuses = event["requirement_statuses"]
                final_source = relative_source(events_path, line_number)

        adjudication_path = module.fix_evidence_path(record, "adjudication")
        adjudication = json_dict(adjudication_path)
        if adjudication is not None:
            result = adjudication.get("adjudication")
            if isinstance(result, dict) and isinstance(
                result.get("requirement_statuses"), dict
            ):
                final_statuses = result["requirement_statuses"]
                final_source = relative_source(adjudication_path)

        atoms = {
            atom: AtomObservation(
                normalize_state(final_statuses.get(atom)), final_source
            )
            for atom in FIX_ATOMS
        }
        finals.append(
            vector(
                profile="fix",
                set_id=record.set_id,
                run_id=record.run_name,
                family=record.family,
                model=record.executor,
                final_full=record.is_full,
                final_assurance=record.assurance,
                atoms=atoms,
            )
        )

        checkpoint_atoms = {
            atom: AtomObservation("unobserved", "not_observed_at_checkpoint")
            for atom in FIX_ATOMS
        }
        ordinal = 0
        for line_number, event in events:
            if event.get("event") != "fix_evidence_recorded":
                continue
            if event.get("run_id") != record.fix_run_id:
                continue
            requirement = str(event.get("requirement_id") or "")
            epoch = event.get("epoch")
            if requirement not in FIX_ATOMS or not isinstance(epoch, int):
                continue
            source = relative_source(events_path, line_number)
            state = fix_probe_state(event)
            if requirement == "no_regression":
                previous = checkpoint_atoms[requirement].state
                if previous == "violation" or state == "violation":
                    state = "violation"
            checkpoint_atoms[requirement] = AtomObservation(state, source)
            ordinal += 1
            checkpoints.append(
                vector(
                    profile="fix",
                    set_id=record.set_id,
                    run_id=record.run_name,
                    family=record.family,
                    model=record.executor,
                    final_full=record.is_full,
                    final_assurance=record.assurance,
                    atoms=dict(checkpoint_atoms),
                    checkpoint_epoch=epoch,
                    checkpoint_ordinal=ordinal,
                )
            )
    return finals, checkpoints


def scan_investigation(
    module: Any,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    records = module.discover_investigation_records()[0]
    finals: list[dict[str, Any]] = []
    checkpoints: list[dict[str, Any]] = []
    for record in records:
        i1_path = record.evidence_dir / "investigation-run.json"
        i2_path = record.evidence_dir / "investigation-binding.json"
        if not record.i2_executed:
            i2_state = "unobserved"
        elif record.violation_count > 0:
            i2_state = "violation"
        elif record.claim_count == 0:
            i2_state = "absent"
        elif record.matched_claim_count == record.claim_count:
            i2_state = "pass"
        else:
            i2_state = "violation"
        atoms = {
            "reproducer_fails": AtomObservation(
                "pass" if record.i1_passed else "violation",
                relative_source(i1_path),
            ),
            "diagnosis_bound": AtomObservation(i2_state, relative_source(i2_path)),
        }
        finals.append(
            vector(
                profile="investigation",
                set_id=record.set_id,
                run_id=record.run_name,
                family=record.family,
                model=record.executor,
                final_full=record.is_full,
                final_assurance=record.assurance,
                atoms=atoms,
            )
        )
        i1 = json_dict(i1_path)
        epoch = i1.get("epoch") if i1 is not None else None
        if isinstance(epoch, int):
            checkpoint_atoms = {
                "reproducer_fails": atoms["reproducer_fails"],
                "diagnosis_bound": AtomObservation(
                    "unobserved", "not_observed_at_checkpoint"
                ),
            }
            checkpoints.append(
                vector(
                    profile="investigation",
                    set_id=record.set_id,
                    run_id=record.run_name,
                    family=record.family,
                    model=record.executor,
                    final_full=record.is_full,
                    final_assurance=record.assurance,
                    atoms=checkpoint_atoms,
                    checkpoint_epoch=epoch,
                    checkpoint_ordinal=1,
                )
            )
    return finals, checkpoints


CIRCLE_ATOMS = (
    "create_to_investigate",
    "investigate_to_fix",
    "fix_to_verify_origin",
)
CIRCLE_EDGE_KEYS = {
    "create->investigate": "create_to_investigate",
    "investigate->fix": "investigate_to_fix",
    "fix->verify_origin": "fix_to_verify_origin",
}


def circle_model(record: Any) -> str:
    models = {
        str(event.get("model") or "")
        for _, event in read_jsonl(record.events_path)
        if event.get("event") == "workflow_node_run_created" and event.get("model")
    }
    return next(iter(models)) if len(models) == 1 else "workflow"


def scan_circle(module: Any) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    records = module.discover_circle_records()[0]
    finals: list[dict[str, Any]] = []
    for record in records:
        document = json_dict(record.circle_path)
        if document is None:
            raise InputError(f"missing workflow circle: {record.circle_path}")
        edge_values = document.get("edges")
        edges = edge_values if isinstance(edge_values, list) else []
        atoms = {
            atom: AtomObservation("unobserved", relative_source(record.circle_path))
            for atom in CIRCLE_ATOMS
        }
        for edge in edges:
            if not isinstance(edge, dict):
                continue
            atom = CIRCLE_EDGE_KEYS.get(str(edge.get("edge") or ""))
            if atom is None:
                continue
            checks = edge.get("checks")
            checks_pass = (
                isinstance(checks, dict)
                and bool(checks)
                and all(
                    isinstance(check, dict) and check.get("passed") is True
                    for check in checks.values()
                )
            )
            state = "pass" if edge.get("fired") is True and checks_pass else "violation"
            atoms[atom] = AtomObservation(state, relative_source(record.circle_path))
        finals.append(
            vector(
                profile="circle",
                set_id=record.set_id,
                run_id=record.run_name,
                family=record.arm,
                model=circle_model(record),
                final_full=record.is_full,
                final_assurance=record.verdict,
                atoms=atoms,
            )
        )
    return finals, []


def scan_cli(module: Any) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    records = module.discover_cli_records()[0]
    module.verify_cli_reached_evidence(records)
    module.assert_cli_invariants(records)
    finals: list[dict[str, Any]] = []
    for record in records:
        source = record.evidence_report or record.evidence_dir
        atoms = {
            atom: AtomObservation(normalize_state(status), relative_source(source))
            for atom, status in zip(
                (
                    "cli_probe",
                    "help_binding",
                    "cli_output_claims",
                    "cli_rerun_consistency",
                ),
                (record.c1, record.c2, record.c3, record.c4),
                strict=True,
            )
        }
        finals.append(
            vector(
                profile="cli",
                set_id=record.set_id,
                run_id=record.run_name,
                family=record.family,
                model=record.executor,
                final_full=record.is_full,
                final_assurance=record.assurance,
                atoms=atoms,
            )
        )
    return finals, []


def scan_ingest(module: Any) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    records = module.discover_ingest_records()[0]
    module.verify_ingest_reached_evidence(records)
    module.assert_ingest_invariants(records)
    finals: list[dict[str, Any]] = []
    for record in records:
        atoms = {
            atom: AtomObservation(
                normalize_state(status), relative_source(record.evidence_summary)
            )
            for atom, status in zip(
                (
                    "source_probe",
                    "source_binding",
                    "candidate_accounting",
                    "format_schema",
                    "ingest_rerun_consistency",
                ),
                (record.n1, record.n2, record.n3, record.n4, record.n5),
                strict=True,
            )
        }
        finals.append(
            vector(
                profile="ingest",
                set_id=record.set_id,
                run_id=record.run_name,
                family=record.family,
                model=record.executor,
                final_full=record.is_full,
                final_assurance=record.earned_assurance,
                atoms=atoms,
            )
        )
    return finals, []


def legacy_nextjs_inventory(runs_root: Path) -> dict[str, int]:
    path = runs_root / "band_summary.md"
    text = path.read_text(encoding="utf-8")
    total_match = re.search(r"^- Total run records: `([0-9]+)`$", text, re.MULTILINE)
    if total_match is None:
        raise InputError(f"legacy Next.js total missing from {path}")
    section = text.split("## Scenario x Final State", 1)[1].split("\n## ", 1)[0]
    full_count = 0
    for line in section.splitlines():
        if not line.startswith("| ") or line.startswith("| ---") or "Scenario" in line:
            continue
        cells = [cell.strip() for cell in line.strip("|").split("|")]
        if len(cells) >= 2 and cells[1].isdigit():
            full_count += int(cells[1])
    return {"run_count": int(total_match.group(1)), "full_count": full_count}


def scan_profiles(module: Any) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    finals: list[dict[str, Any]] = []
    checkpoints: list[dict[str, Any]] = []
    for scanner in (
        scan_data,
        scan_fix,
        scan_investigation,
        scan_circle,
        scan_cli,
        scan_ingest,
    ):
        profile_finals, profile_checkpoints = scanner(module)
        finals.extend(profile_finals)
        checkpoints.extend(profile_checkpoints)
    finals.sort(key=lambda item: (item["profile"], item["run_id"]))
    checkpoints.sort(
        key=lambda item: (
            item["profile"],
            item["run_id"],
            item["checkpoint_epoch"],
            item["checkpoint_ordinal"],
        )
    )
    keys = [item["run_id"] for item in finals]
    if len(keys) != len(set(keys)):
        duplicates = sorted(key for key in set(keys) if keys.count(key) > 1)
        raise InputError(f"duplicate historical run ids: {', '.join(duplicates)}")
    return finals, checkpoints


def average_ranks(values: Sequence[float]) -> list[float]:
    order = sorted(range(len(values)), key=values.__getitem__)
    ranks = [0.0] * len(values)
    start = 0
    while start < len(order):
        end = start + 1
        while end < len(order) and values[order[end]] == values[order[start]]:
            end += 1
        rank = (start + 1 + end) / 2
        for index in order[start:end]:
            ranks[index] = rank
        start = end
    return ranks


def spearman(values: Sequence[float], outcomes: Sequence[float]) -> float | None:
    if len(values) != len(outcomes) or len(values) < 2:
        return None
    x = average_ranks(values)
    y = average_ranks(outcomes)
    x_mean = statistics.fmean(x)
    y_mean = statistics.fmean(y)
    numerator = sum((a - x_mean) * (b - y_mean) for a, b in zip(x, y, strict=True))
    x_scale = sum((a - x_mean) ** 2 for a in x)
    y_scale = sum((b - y_mean) ** 2 for b in y)
    if x_scale == 0 or y_scale == 0:
        return None
    return numerator / math.sqrt(x_scale * y_scale)


def intermediate_vectors(
    checkpoints: Sequence[dict[str, Any]],
) -> dict[str, dict[str, Any]]:
    result: dict[str, dict[str, Any]] = {}
    for checkpoint in checkpoints:
        if checkpoint["observed_weight"] == 0:
            continue
        result.setdefault(checkpoint["run_id"], checkpoint)
    return result


def correlation_rows(
    finals: Sequence[dict[str, Any]], checkpoints: Sequence[dict[str, Any]]
) -> list[dict[str, Any]]:
    intermediate = intermediate_vectors(checkpoints)
    strata = sorted({(item["profile"], item["model_tier"]) for item in finals})
    rows: list[dict[str, Any]] = []
    for profile, tier in strata:
        selected = [
            item
            for item in finals
            if item["profile"] == profile
            and item["model_tier"] == tier
            and item["run_id"] in intermediate
        ]
        scores = [float(intermediate[item["run_id"]]["score"]) for item in selected]
        outcomes = [
            1.0 if item["final_verdict"] == "full" else 0.0 for item in selected
        ]
        coefficient = spearman(scores, outcomes)
        if len(selected) < 5:
            display = "hidden (n<5)"
            reason = "minimum_sample_guard"
            coefficient = None
        elif coefficient is None:
            display = "unavailable (constant input)"
            reason = "constant_score_or_final_outcome"
        else:
            coefficient = round(coefficient, 4)
            display = f"{coefficient:.4f}"
            reason = "reported"
        rows.append(
            {
                "profile": profile,
                "model_tier": tier,
                "n": len(selected),
                "spearman": coefficient,
                "display": display,
                "reason": reason,
            }
        )
    return rows


def coverage_rows(
    finals: Sequence[dict[str, Any]], checkpoints: Sequence[dict[str, Any]]
) -> list[dict[str, Any]]:
    checkpoint_runs = {item["run_id"] for item in checkpoints}
    rows: list[dict[str, Any]] = []
    for profile in sorted({item["profile"] for item in finals}):
        selected = [item for item in finals if item["profile"] == profile]
        checkpoint_count = sum(item["run_id"] in checkpoint_runs for item in selected)
        rows.append(
            {
                "profile": profile,
                "inventory_runs": len(selected),
                "scannable_runs": len(selected),
                "final_only_runs": len(selected) - checkpoint_count,
                "checkpoint_capable_runs": checkpoint_count,
                "reached_runs": sum(bool(item["reached"]) for item in selected),
                "full_runs": sum(item["final_verdict"] == "full" for item in selected),
            }
        )
    return rows


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def historical_tree_snapshot(root: Path, output_dir: Path) -> dict[str, Any]:
    digest = hashlib.sha256()
    file_count = 0
    output = output_dir.resolve()
    for path in sorted(
        candidate for candidate in root.rglob("*") if candidate.is_file()
    ):
        resolved = path.resolve()
        if resolved == output or output in resolved.parents:
            continue
        relative = path.relative_to(root).as_posix()
        item_hash = file_sha256(path)
        digest.update(relative.encode("utf-8"))
        digest.update(b"\0")
        digest.update(item_hash.encode("ascii"))
        digest.update(b"\n")
        file_count += 1
    return {"file_count": file_count, "tree_sha256": digest.hexdigest()}


def build_study(
    runs_root: Path,
    output_dir: Path,
) -> tuple[dict[str, Any], list[dict[str, Any]], list[dict[str, Any]]]:
    if runs_root.resolve() != DEFAULT_RUNS_ROOT.resolve():
        raise InputError(
            "full execution is pinned to the repository-managed runs root; "
            f"expected {DEFAULT_RUNS_ROOT}"
        )
    before = historical_tree_snapshot(runs_root, output_dir)
    module = load_band_module()
    finals, checkpoints = scan_profiles(module)
    legacy = legacy_nextjs_inventory(runs_root)
    coverage = coverage_rows(finals, checkpoints)
    checkpoint_runs = {item["run_id"] for item in checkpoints}
    full_vectors = [item for item in finals if item["final_verdict"] == "full"]
    full_failures = [
        item["run_id"]
        for item in full_vectors
        if item["score"] != 100.0
        or any(atom["state"] != "pass" for atom in item["atoms"])
    ]
    if full_failures:
        raise InputError(
            "full=100 consistency failed: " + ", ".join(sorted(full_failures))
        )
    after = historical_tree_snapshot(runs_root, output_dir)
    if before != after:
        raise InputError("historical input bytes changed during the retrospective scan")
    scannable = len(finals)
    checkpoint_capable = len(checkpoint_runs)
    summary = {
        "schema_version": STUDY_SCHEMA_VERSION,
        "study_id": STUDY_ID,
        "status": "complete",
        "fixed_date": FIXED_DATE,
        "score_spec": {
            "weights": "equal required-atom weights",
            "state_coefficients": {
                "pass": 1,
                "absent": 0,
                "violation": -0.5,
                "unobserved": 0,
            },
            "scale": [-50, 100],
            "weights_changed_after_scan": False,
            "new_judges": 0,
        },
        "coverage": {
            "historical_inventory_runs": scannable + legacy["run_count"],
            "scannable_runs": scannable,
            "final_only_runs": scannable - checkpoint_capable,
            "checkpoint_capable_runs": checkpoint_capable,
            "unscannable_aggregate_only_runs": legacy["run_count"],
            "profiles": [
                {
                    "profile": "nextjs",
                    "inventory_runs": legacy["run_count"],
                    "scannable_runs": 0,
                    "final_only_runs": 0,
                    "checkpoint_capable_runs": 0,
                    "reached_runs": 0,
                    "full_runs": legacy["full_count"],
                    "gap": "aggregate summary retained; run-level atom rows unavailable",
                },
                *coverage,
            ],
        },
        "correlation": {
            "method": (
                "Spearman rank correlation between the first epoch-backed vector "
                "with at least one required atom observed and final full=1/non-full=0"
            ),
            "minimum_display_n": 5,
            "strata": correlation_rows(finals, checkpoints),
        },
        "full_score_consistency": {
            "status": "pass",
            "scannable_full_runs": len(full_vectors),
            "score_100_runs": len(full_vectors),
            "failures": [],
            "legacy_aggregate_only_full_runs_not_reconstructable": legacy["full_count"],
        },
        "guards": {
            "historical_files_mutated": False,
            "historical_tree_before": before,
            "historical_tree_after": after,
            "checkpoint_time_invented": False,
            "mtime_used_as_checkpoint": False,
            "weights_changed": False,
            "new_judges": 0,
        },
    }
    return summary, finals, checkpoints


def markdown_report(summary: dict[str, Any]) -> str:
    coverage = summary["coverage"]
    full = summary["full_score_consistency"]
    lines = [
        "# F-1 Retrospective 001",
        "",
        f"Status: complete ({summary['fixed_date']})",
        "",
        "固定済みの等配点と`fail / violation = -w/2`だけを用い、既存judgeを追加せず",
        "repository-managed historyをread-only走査した。これは重み選定ではなく、粗い",
        "得点で十分かを検証する走査であり、結果を見てweightは動かしていない。",
        "",
        "## Coverage",
        "",
        "| profile | inventory | scannable | final-only | checkpoint-capable | reached | full |",
        "|---|---:|---:|---:|---:|---:|---:|",
    ]
    for row in coverage["profiles"]:
        lines.append(
            "| {profile} | {inventory_runs} | {scannable_runs} | {final_only_runs} | "
            "{checkpoint_capable_runs} | {reached_runs} | {full_runs} |".format(**row)
        )
    lines.extend(
        [
            "",
            (
                f"総inventoryは{coverage['historical_inventory_runs']}、run-level走査可能は"
                f"{coverage['scannable_runs']}、final-onlyは"
                f"{coverage['final_only_runs']}、checkpoint可能は"
                f"{coverage['checkpoint_capable_runs']}。旧Next.js 78 runは"
            ),
            "集計表だけが現checkoutに残り、run ID・原子列を復元できないため推測せずgapとした。",
            "",
            "## Intermediate → final correlation",
            "",
            "| profile | model tier | n | Spearman | rule |",
            "|---|---|---:|---:|---|",
        ]
    )
    for row in summary["correlation"]["strata"]:
        lines.append(
            f"| {row['profile']} | {row['model_tier']} | {row['n']} | "
            f"{row['display']} | {row['reason']} |"
        )
    reported = [
        row for row in summary["correlation"]["strata"] if row["reason"] == "reported"
    ]
    if reported:
        reading = "; ".join(
            f"{row['profile']} × {row['model_tier']} ρ={row['spearman']:.4f} (n={row['n']})"
            for row in reported
        )
    else:
        reading = "係数を表示できる層はなかった"
    lines.extend(
        [
            "",
            f"読み: {reading}。constant outcomeの層は相関を捏造せずunavailable、n<5は",
            "裁定どおり非表示にした。この結果からweightは変更しない。",
            "",
            "## Full = 100 consistency",
            "",
            (
                f"run-level原子を復元できたfullは{full['scannable_full_runs']}件で、"
                f"全{full['score_100_runs']}件がscore 100かつ全required atom passだった。"
            ),
            (
                "旧Next.jsのaggregate-only full "
                f"{full['legacy_aggregate_only_full_runs_not_reconstructable']}件は"
            ),
            "run-level原子がないため検算分母へ混ぜていない。",
            "",
            "## Read-only and anti-overfitting guards",
            "",
            f"- historical files: {summary['guards']['historical_tree_before']['file_count']} files",
            (
                f"- tree SHA-256 before/after: "
                f"`{summary['guards']['historical_tree_before']['tree_sha256']}` / "
                f"`{summary['guards']['historical_tree_after']['tree_sha256']}`"
            ),
            "- historical mutation: false",
            "- invented checkpoint timestamps: false",
            "- new judges: 0",
            "- weights changed after scan: false",
            "",
        ]
    )
    return "\n".join(lines)


GENERATED_FILES = (
    "final-vectors.jsonl",
    "checkpoint-vectors.jsonl",
    "study-summary.json",
    "report.md",
)


def write_study(
    output_dir: Path,
    summary: dict[str, Any],
    finals: Sequence[dict[str, Any]],
    checkpoints: Sequence[dict[str, Any]],
    *,
    overwrite: bool,
) -> None:
    if output_dir.exists() and not overwrite:
        raise InputError(
            f"output directory already exists: {output_dir}; pass --overwrite"
        )
    output_dir.mkdir(parents=True, exist_ok=True)
    final_text = "".join(
        json.dumps(item, ensure_ascii=False, sort_keys=True) + "\n" for item in finals
    )
    checkpoint_text = "".join(
        json.dumps(item, ensure_ascii=False, sort_keys=True) + "\n"
        for item in checkpoints
    )
    contents = {
        "final-vectors.jsonl": final_text,
        "checkpoint-vectors.jsonl": checkpoint_text,
        "study-summary.json": json.dumps(
            summary, ensure_ascii=False, indent=2, sort_keys=True
        )
        + "\n",
        "report.md": markdown_report(summary),
    }
    for name in GENERATED_FILES:
        (output_dir / name).write_text(contents[name], encoding="utf-8")


def validate_output_location(runs_root: Path, output_dir: Path) -> None:
    root = runs_root.resolve()
    output = output_dir.resolve()
    if output == root or root not in output.parents:
        raise InputError("--output-dir must be a child of --runs-root")


def main(argv: Sequence[str] | None = None) -> int:
    argument_parser = parser()
    args = argument_parser.parse_args(argv)
    try:
        if args.dry_run:
            if args.campaign_summary is None:
                raise InputError("--campaign-summary is required with --dry-run")
            document = load_campaign_summary(args.campaign_summary)
            plan = build_plan(
                args.campaign_summary,
                document,
                events_root=args.events_root,
                score_config=args.score_config,
            )
            print(json.dumps(plan, ensure_ascii=False, indent=2, sort_keys=True))
            return 0

        validate_output_location(args.runs_root, args.output_dir)
        summary, finals, checkpoints = build_study(args.runs_root, args.output_dir)
        write_study(
            args.output_dir,
            summary,
            finals,
            checkpoints,
            overwrite=args.overwrite,
        )
    except InputError as exc:
        argument_parser.error(str(exc))
    print(
        json.dumps(
            {
                "status": "complete",
                "output_dir": display_path(args.output_dir, REPOSITORY_ROOT),
                "scannable_runs": summary["coverage"]["scannable_runs"],
                "checkpoint_capable_runs": summary["coverage"][
                    "checkpoint_capable_runs"
                ],
                "full_score_consistency": summary["full_score_consistency"],
            },
            ensure_ascii=False,
            indent=2,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
