#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_live import _load_record_ledger
from eval_lib.goal_verify_main_report_v4 import (
    build_main_report,
    build_main_smoke_report,
    evaluate_main_semantic_review,
)
from eval_lib.goal_verify_preflight_report_v4 import load_records, semantic_review_gate


def main() -> int:
    parser = argparse.ArgumentParser(description="Build Phase 6 v4 main report")
    parser.add_argument("--run-dir", type=Path, required=True)
    parser.add_argument(
        "--contract",
        type=Path,
        default=Path("eval/goal_verify/v0/phase6-main-v4-contract.json"),
    )
    parser.add_argument("--output-name", help="run-relative output name")
    parser.add_argument(
        "--smoke",
        action="store_true",
        help="validate only the preregistered smoke instrument gates",
    )
    parser.add_argument("--require-go", action="store_true")
    args = parser.parse_args()
    run_dir = _resolve(args.run_dir)
    contract_path = _resolve(args.contract)
    contract = _read_json(contract_path)
    summary = _read_json(run_dir / "campaign-summary.json")
    entries, ledger_head = _load_record_ledger(
        root=ROOT,
        run_dir=run_dir,
        ledger_path=run_dir / contract["integrity"]["record_ledger"],
    )
    if summary.get("record_ledger_entries") != len(entries):
        raise ValueError("campaign summary record count differs from ledger")
    if summary.get("record_ledger_head_sha256") != ledger_head:
        raise ValueError("campaign summary differs from ledger head")
    records = load_records(run_dir)
    if len(records) != len(entries):
        raise ValueError("raw record count differs from ledger")
    config = _read_json(ROOT / contract["resource_budget_config"])
    if args.smoke:
        manifest = _read_json(run_dir / "campaign-manifest.json")
        report_a = build_main_smoke_report(
            contract=contract,
            records=records,
            manifest=manifest,
        )
        report_b = build_main_smoke_report(
            contract=contract,
            records=records,
            manifest=manifest,
        )
        default_output = "main-smoke-report-v4.json"
    else:
        blind_path = run_dir / "blind-review-v4/blind-review-report-v4.json"
        blind_report = _read_json(blind_path) if blind_path.is_file() else None
        semantic_complete = semantic_review_gate(
            contract=contract,
            blind_report=blind_report,
        )
        semantic_evaluation = evaluate_main_semantic_review(
            contract=contract,
            blind_report=blind_report,
            semantic_review_complete=semantic_complete,
        )
        report_a = build_main_report(
            contract=contract,
            config=config,
            records=records,
            semantic_review_complete=semantic_complete,
            semantic_review_evaluation=semantic_evaluation,
        )
        report_b = build_main_report(
            contract=contract,
            config=config,
            records=records,
            semantic_review_complete=semantic_complete,
            semantic_review_evaluation=semantic_evaluation,
        )
        default_output = "main-report-v4.json"
    encoded_a = _encode(report_a)
    encoded_b = _encode(report_b)
    if encoded_a != encoded_b:
        raise ValueError("same-script replay differs")
    report_a["same_script_replay"] = {
        "byte_identical": True,
        "sha256": hashlib.sha256(encoded_a).hexdigest(),
        "script": "scripts/eval-goal-verify-main-v4-report.py",
    }
    output = run_dir / (args.output_name or default_output)
    output.write_bytes(_encode(report_a))
    print(output.relative_to(ROOT))
    return 2 if args.require_go and report_a["final_decision"] != "GO" else 0


def _encode(value: dict) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")


def _resolve(path: Path) -> Path:
    return path.resolve() if path.is_absolute() else (ROOT / path).resolve()


def _read_json(path: Path) -> dict:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise TypeError(f"expected object:{path}")
    return value


if __name__ == "__main__":
    raise SystemExit(main())
