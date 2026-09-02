#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from pathlib import Path

from eval_lib.goal_verify_live import _atomic_json, _load_record_ledger, load_json
from eval_lib.goal_verify_recovery_a23_report import (
    authoritative_report_source_errors,
)
from eval_lib.goal_verify_recovery_a25_report import (
    build_recovery_a25_pilot_report,
    recovery_a25_contract_errors,
)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Report A25 corrected natural Recovery exposure pilot"
    )
    parser.add_argument(
        "--root", type=Path, default=Path(__file__).resolve().parents[1]
    )
    parser.add_argument("--contract", type=Path, required=True)
    parser.add_argument("--run-dir", type=Path, required=True)
    args = parser.parse_args()
    contract = load_json(args.contract.resolve())
    contract_errors = recovery_a25_contract_errors(contract)
    if contract_errors:
        raise ValueError("; ".join(contract_errors))
    root = args.root.resolve()
    source_errors = authoritative_report_source_errors(root=root, contract=contract)
    if source_errors:
        raise ValueError("; ".join(source_errors))
    run_dir = args.run_dir.resolve()
    _load_record_ledger(
        root=root,
        run_dir=run_dir,
        ledger_path=run_dir / contract["integrity"]["record_ledger"],
    )
    records = [
        load_json(path) for path in sorted((run_dir / "raw").glob("**/pair-*.json"))
    ]
    preflight_path = run_dir / "oracle-executability-preflight.json"
    preflight = load_json(preflight_path) if preflight_path.is_file() else None
    report = build_recovery_a25_pilot_report(
        records=records,
        contract=contract,
        oracle_executability_preflight=preflight,
    )
    _atomic_json(run_dir / "recovery-a25-pilot-report.json", report)
    print(json.dumps(report, ensure_ascii=False, sort_keys=True))
    return 0 if report["pilot_instrument_ready"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
