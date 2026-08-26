#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_preflight_v4 import readiness_report


def main() -> int:
    parser = argparse.ArgumentParser(description="Check Phase 6 v4 additive readiness")
    parser.add_argument(
        "--contract",
        type=Path,
        default=Path("eval/goal_verify/v0/phase6-preflight-v4-contract.json"),
    )
    parser.add_argument("--execution-root", type=Path)
    args = parser.parse_args()
    contract = args.contract if args.contract.is_absolute() else ROOT / args.contract
    execution_root = args.execution_root
    if execution_root is not None and not execution_root.is_absolute():
        execution_root = ROOT / execution_root
    report = readiness_report(
        root=ROOT, contract_path=contract, execution_root=execution_root
    )
    print(json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True))
    return 0 if report["ready"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
