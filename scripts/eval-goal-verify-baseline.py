#!/usr/bin/env python3
from __future__ import annotations

import argparse
from pathlib import Path

from eval_lib.goal_verify_baseline import write_report


def main() -> int:
    parser = argparse.ArgumentParser(description="Replay the frozen goal-to-verify v0 baseline")
    parser.add_argument("--corpus", type=Path, default=Path("eval/goal_verify/v0/corpus.json"))
    parser.add_argument("--config", type=Path, default=Path("eval/goal_verify/v0/baseline-config.json"))
    parser.add_argument("--run-dir", type=Path, required=True)
    args = parser.parse_args()
    report = write_report(corpus_path=args.corpus, config_path=args.config, run_dir=args.run_dir)
    print(f"[write] {args.run_dir / 'baseline.json'}")
    print(f"[done] go_no_go={report['go_no_go']['status']} cases={report['metrics']['case_count']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
