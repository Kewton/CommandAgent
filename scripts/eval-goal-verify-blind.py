#!/usr/bin/env python3
from __future__ import annotations

import argparse
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_blind import run_blind_review


def main() -> int:
    parser = argparse.ArgumentParser(description="Run Phase 6 variant-hidden review")
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate-draft", type=Path, required=True)
    parser.add_argument(
        "--contract",
        type=Path,
        default=Path("eval/goal_verify/v0/phase6-blind-review-contract.json"),
    )
    parser.add_argument("--run-dir", type=Path, required=True)
    args = parser.parse_args()

    def absolute(path: Path) -> Path:
        return path if path.is_absolute() else ROOT / path

    report = run_blind_review(
        root=ROOT,
        baseline_path=absolute(args.baseline),
        candidate_draft_path=absolute(args.candidate_draft),
        contract_path=absolute(args.contract),
        run_dir=absolute(args.run_dir),
    )
    print(
        f"[done] reviewed_pairs={report['reviewed_pairs']}/{report['expected_pairs']} "
        f"complete={str(report['complete']).lower()}"
    )
    return 0 if report["complete"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
