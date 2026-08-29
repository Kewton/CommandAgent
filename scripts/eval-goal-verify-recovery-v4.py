#!/usr/bin/env python3
from __future__ import annotations

import argparse
from pathlib import Path

from eval_lib.goal_verify_recovery_live_v4 import run_recovery_smoke


def main() -> int:
    parser = argparse.ArgumentParser(description="Run A14 Recovery 0-vs-1 smoke")
    parser.add_argument(
        "--root", type=Path, default=Path(__file__).resolve().parents[1]
    )
    parser.add_argument("--contract", type=Path, required=True)
    parser.add_argument("--run-dir", type=Path, required=True)
    parser.add_argument("--execution-root", type=Path, required=True)
    parser.add_argument("--commandagent-bin", type=Path, required=True)
    parser.add_argument("--limit", type=int)
    args = parser.parse_args()
    summary = run_recovery_smoke(
        root=args.root.resolve(),
        contract_path=args.contract.resolve(),
        run_dir=args.run_dir.resolve(),
        execution_root=args.execution_root.resolve(),
        commandagent_bin=args.commandagent_bin.resolve(),
        limit=args.limit,
    )
    print(summary)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
