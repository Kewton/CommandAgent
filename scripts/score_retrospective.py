#!/usr/bin/env python3
"""Plan an F-1 score retrospective without scanning historical run evidence.

Inputs:
  --campaign-summary PATH  Repository campaign-summary.json to inventory.
  --events-root PATH       Optional future live-run root; never read in dry-run.
  --score-config PATH      Optional future eval.yaml; never read in dry-run.

Output:
  A deterministic JSON dry-run plan on stdout. This draft performs no event or
  evidence scan, computes no score or correlation, and writes no files.
"""

from __future__ import annotations

import argparse
import json
from collections.abc import Sequence
from pathlib import Path
from typing import Any

PLAN_SCHEMA_VERSION = "commandagent.score-retrospective-plan/v0"
PENDING_ADJUDICATION = "F-1a score institution review adjudication"


class InputError(ValueError):
    """Raised when a campaign inventory cannot be planned honestly."""


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(
        description=(
            "Emit an inventory-only F-1 retrospective plan. Full event scanning "
            "is disabled until the score institution is adjudicated."
        )
    )
    result.add_argument(
        "--campaign-summary",
        required=True,
        type=Path,
        help="repository campaign-summary.json used only for run/hash inventory",
    )
    result.add_argument(
        "--events-root",
        type=Path,
        help="future live-run root to record in the plan without reading it",
    )
    result.add_argument(
        "--score-config",
        type=Path,
        help="future eval.yaml to record in the plan without parsing it",
    )
    result.add_argument(
        "--dry-run",
        action="store_true",
        help="required safety switch; emits a plan and performs no retrospective",
    )
    return result


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
    if not all(isinstance(key, str) and isinstance(value, str) for key, value in hashes.items()):
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
        if len(digest) != 64 or any(character not in "0123456789abcdef" for character in digest):
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
        raise InputError(f"events sha256 inventory has unknown runs: {', '.join(unexpected)}")
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


def main(argv: Sequence[str] | None = None) -> int:
    argument_parser = parser()
    args = argument_parser.parse_args(argv)
    if not args.dry_run:
        argument_parser.error(
            "retrospective execution is disabled pending F-1a adjudication; pass --dry-run"
        )
    try:
        document = load_campaign_summary(args.campaign_summary)
        plan = build_plan(
            args.campaign_summary,
            document,
            events_root=args.events_root,
            score_config=args.score_config,
        )
    except InputError as exc:
        argument_parser.error(str(exc))
    print(json.dumps(plan, ensure_ascii=False, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
