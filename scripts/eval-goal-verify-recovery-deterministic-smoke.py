#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from pathlib import Path

from eval_lib.goal_verify_recovery_deterministic_smoke import run_smoke


def main() -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Run an instrument-only deterministic Recovery transaction path smoke"
        )
    )
    parser.add_argument("--commandagent-bin", type=Path, required=True)
    parser.add_argument("--run-dir", type=Path, required=True)
    parser.add_argument("--execution-root", type=Path)
    parser.add_argument("--timeout-sec", type=int, default=60)
    parser.add_argument(
        "--scenario",
        choices=("generic-create", "data-fix"),
        default="generic-create",
    )
    args = parser.parse_args()
    report = run_smoke(
        commandagent_bin=args.commandagent_bin,
        run_dir=args.run_dir,
        execution_root=args.execution_root,
        timeout_sec=args.timeout_sec,
        scenario=args.scenario,
    )
    print(json.dumps(report, ensure_ascii=False, sort_keys=True))
    return 0 if report["instrument_ready"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
