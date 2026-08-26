#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_preflight_v2 import assess_v2_readiness


def main() -> int:
    parser = argparse.ArgumentParser(description="Check Phase 6 contract-v2 readiness")
    parser.add_argument(
        "--contract",
        type=Path,
        default=Path("eval/goal_verify/v0/phase6-preflight-v2-contract.json"),
    )
    args = parser.parse_args()
    contract = args.contract if args.contract.is_absolute() else ROOT / args.contract
    result = assess_v2_readiness(root=ROOT, contract_path=contract)
    print(json.dumps(result, ensure_ascii=False, indent=2, sort_keys=True))
    return 0 if result["ready"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
