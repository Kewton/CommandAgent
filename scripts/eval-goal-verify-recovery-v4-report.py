#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from pathlib import Path

from eval_lib.goal_verify_live import _atomic_json, _load_record_ledger, load_json
from eval_lib.goal_verify_recovery_report_v4 import build_recovery_report


def main() -> int:
    parser = argparse.ArgumentParser(description="Report A14 Recovery 0-vs-1 smoke")
    parser.add_argument(
        "--root", type=Path, default=Path(__file__).resolve().parents[1]
    )
    parser.add_argument("--contract", type=Path, required=True)
    parser.add_argument("--run-dir", type=Path, required=True)
    args = parser.parse_args()
    contract = load_json(args.contract.resolve())
    root = args.root.resolve()
    run_dir = args.run_dir.resolve()
    _load_record_ledger(
        root=root,
        run_dir=run_dir,
        ledger_path=run_dir / contract["integrity"]["record_ledger"],
    )
    records = [
        load_json(path) for path in sorted((run_dir / "raw").glob("**/pair-*.json"))
    ]
    report = build_recovery_report(records=records, contract=contract)
    _atomic_json(run_dir / "recovery-report-v4.json", report)
    print(json.dumps(report, ensure_ascii=False, sort_keys=True))
    return 0 if report["instrument_ready"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
