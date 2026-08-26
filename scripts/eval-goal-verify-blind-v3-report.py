#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_blind_v3 import build_blind_review_report


def load(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def main() -> int:
    parser = argparse.ArgumentParser(description="Validate Phase 6 v3 blind reviews")
    parser.add_argument("--run-dir", type=Path, required=True)
    parser.add_argument("--model-review", type=Path, action="append", required=True)
    parser.add_argument("--human-review", type=Path, required=True)
    args = parser.parse_args()
    run_dir = args.run_dir if args.run_dir.is_absolute() else ROOT / args.run_dir
    blind = run_dir / "blind-review"
    items = load(blind / "items-semantic_hidden.json")
    mapping = load(blind / "secret/mapping.json")["semantic_hidden"]
    human_template = load(blind / "human-calibration-template.json")
    report = build_blind_review_report(
        items=items,
        mapping=mapping,
        model_reviews=[load(path if path.is_absolute() else ROOT / path) for path in args.model_review],
        human_review=load(
            args.human_review
            if args.human_review.is_absolute()
            else ROOT / args.human_review
        ),
        required_human_ids=human_template["item_ids"],
    )
    output = blind / "report.json"
    output.write_text(
        json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True))
    return 0 if report["semantic_blind_review_complete"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
