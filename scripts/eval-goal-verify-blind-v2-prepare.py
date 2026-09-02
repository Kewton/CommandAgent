#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_blind_v2 import (
    prepare_semantic_items,
    semantic_arms_from_paired_records,
)


def _load(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise TypeError(f"expected JSON object: {path}")
    return value


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _write_new(path: Path, value: dict[str, Any]) -> None:
    if path.exists():
        raise FileExistsError(f"refusing to overwrite blind artifact: {path}")
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    temporary.replace(path)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Prepare anonymous Phase 6 v2 semantic blind-review items"
    )
    parser.add_argument("--run-dir", type=Path, required=True)
    parser.add_argument(
        "--contract",
        type=Path,
        default=Path("eval/goal_verify/v0/phase6-semantic-blind-v2-contract.json"),
    )
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()

    run_dir = args.run_dir if args.run_dir.is_absolute() else ROOT / args.run_dir
    contract_path = (
        args.contract if args.contract.is_absolute() else ROOT / args.contract
    )
    output_dir = (
        args.output_dir if args.output_dir.is_absolute() else ROOT / args.output_dir
    )
    contract = _load(contract_path)
    if contract.get("status") != "frozen_contract_integration_preflight":
        raise ValueError("semantic blind contract is not frozen")
    summary = _load(run_dir / "campaign-summary.json")
    if summary.get("complete") is not True:
        raise ValueError("campaign is incomplete")

    records = {}
    for path in sorted((run_dir / "raw").glob("*/*.json")):
        record = _load(path)
        pair_id = record.get("pair_id")
        if not isinstance(pair_id, str) or pair_id in records:
            raise ValueError(f"invalid or duplicate pair ID: {path}")
        records[pair_id] = record
    if len(records) != summary.get("target_pairs"):
        raise ValueError("raw record count differs from campaign target")

    corpus = _load(run_dir / "candidate-corpus.draft.json")
    cases = {case["case_id"]: case for case in corpus["cases"]}
    baseline, candidate = semantic_arms_from_paired_records(records)
    contract_sha256 = _sha256(contract_path)
    seed = int(contract_sha256[:16], 16)
    items, mapping = prepare_semantic_items(
        baseline,
        candidate,
        cases_by_pair_id=cases,
        seed=seed,
    )

    items_path = output_dir / "items.json"
    mapping_path = output_dir / "secret-mapping.json"
    manifest_path = output_dir / "manifest.json"
    _write_new(
        items_path,
        {
            "schema_version": "commandagent.goal_verify.semantic_blind_items.v2",
            "contract_id": contract.get("contract_id"),
            "item_count": len(items),
            "items": items,
        },
    )
    _write_new(
        mapping_path,
        {
            "schema_version": "commandagent.goal_verify.semantic_blind_mapping.v2",
            "access": "withhold_from_reviewers_until_reviews_are_final",
            "mapping": mapping,
        },
    )
    _write_new(
        manifest_path,
        {
            "schema_version": "commandagent.goal_verify.semantic_blind_manifest.v2",
            "contract_sha256": contract_sha256,
            "randomization": {
                "derivation": "unsigned big-endian integer from first 16 hex characters of contract_sha256",
                "seed": seed,
            },
            "preparer_source": str(Path(__file__).resolve().relative_to(ROOT)),
            "preparer_source_sha256": _sha256(Path(__file__).resolve()),
            "items_sha256": _sha256(items_path),
            "mapping_sha256": _sha256(mapping_path),
            "item_count": len(items),
        },
    )
    print(f"[done] blind_items={len(items)} seed={seed}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
