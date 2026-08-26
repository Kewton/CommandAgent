#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_preflight_report_v3 import build_report, load_records


def main() -> int:
    parser = argparse.ArgumentParser(description="Build Phase 6 v3 preflight report")
    parser.add_argument("--run-dir", type=Path, required=True)
    parser.add_argument(
        "--contract",
        type=Path,
        default=Path("eval/goal_verify/v0/phase6-preflight-v3-contract.json"),
    )
    args = parser.parse_args()
    run_dir = args.run_dir if args.run_dir.is_absolute() else ROOT / args.run_dir
    contract_path = (
        args.contract if args.contract.is_absolute() else ROOT / args.contract
    )
    contract = json.loads(contract_path.read_text(encoding="utf-8"))
    blind_path = run_dir / "blind-review/report.json"
    blind = (
        json.loads(blind_path.read_text(encoding="utf-8"))
        if blind_path.is_file()
        else {}
    )
    report = build_report(
        contract=contract,
        records=load_records(run_dir),
        blind_complete=blind.get("semantic_blind_review_complete") is True,
    )
    output = run_dir / "preflight-report-v3.json"
    output.write_text(
        json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True))
    return 0 if report["ready_for_full_experiment_design"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
