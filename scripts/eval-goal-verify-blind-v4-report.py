#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_blind_v4 import build_blind_report, canonical_sha256


def main() -> int:
    parser = argparse.ArgumentParser(description="Report Phase 6 v4 blind review")
    parser.add_argument("--run-dir", type=Path, required=True)
    parser.add_argument(
        "--human-review",
        type=Path,
        help="Independent human review JSON (defaults inside blind-review-v4)",
    )
    parser.add_argument("--require-complete", action="store_true")
    args = parser.parse_args()
    run_dir = args.run_dir if args.run_dir.is_absolute() else ROOT / args.run_dir
    review_dir = run_dir / "blind-review-v4"
    manifest = _read_json(review_dir / "manifest.json")
    items = _read_json(review_dir / "items-semantic-hidden.json")
    human_items = _read_json(review_dir / "human-items-semantic-hidden.json")
    if manifest["items_sha256"] != canonical_sha256(items):
        raise ValueError("blind item hash differs from preparation manifest")
    if manifest["human_items_sha256"] != canonical_sha256(human_items["items"]):
        raise ValueError("human item hash differs from preparation manifest")
    if human_items["items_sha256"] != manifest["items_sha256"]:
        raise ValueError("human item packet is not bound to the full item set")
    human_path = args.human_review or review_dir / "human-review-independent.json"
    if not human_path.is_absolute():
        human_path = ROOT / human_path
    human = _read_json(human_path)
    model_documents = [
        _read_json(path) for path in sorted(review_dir.glob("review-model-*.json"))
    ]
    report = build_blind_report(
        items=items,
        model_documents=model_documents,
        human_document=human,
        human_items=human_items["items"],
    )
    output = review_dir / "blind-review-report-v4.json"
    output.write_text(
        json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(output.relative_to(ROOT))
    if args.require_complete and not report["semantic_review_complete"]:
        return 1
    return 0


def _read_json(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


if __name__ == "__main__":
    raise SystemExit(main())
