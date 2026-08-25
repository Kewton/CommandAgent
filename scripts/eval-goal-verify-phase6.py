#!/usr/bin/env python3
from __future__ import annotations

import argparse
from pathlib import Path

from eval_lib.goal_verify_phase6 import write_phase6_report


def main() -> int:
    parser = argparse.ArgumentParser(description="Aggregate goal-to-verify Phase 6 A/B UAT")
    parser.add_argument("--manifest", type=Path, default=Path("eval/goal_verify/v0/phase6-matrix.json"))
    parser.add_argument("--config", type=Path, default=Path("eval/goal_verify/v0/baseline-config.json"))
    parser.add_argument("--run-dir", type=Path, required=True)
    args = parser.parse_args()
    report = write_phase6_report(
        manifest_path=args.manifest,
        config_path=args.config,
        run_dir=args.run_dir,
        root=Path.cwd(),
    )
    print(f"[write] {args.run_dir / 'phase6-report.json'}")
    print(f"[done] final_decision={report['final_decision']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
