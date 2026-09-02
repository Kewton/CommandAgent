#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
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
        help="Legacy alias for --calibration-review",
    )
    parser.add_argument(
        "--calibration-review",
        type=Path,
        help="Authorized calibration review JSON",
    )
    parser.add_argument(
        "--contract",
        type=Path,
        help="Contract containing semantic_review.calibration_reviewer_policy",
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
    if args.human_review and args.calibration_review:
        raise ValueError("use only one review input option")
    reviewer_policy = None
    semantic_review = None
    if args.contract:
        contract_path = (
            args.contract if args.contract.is_absolute() else ROOT / args.contract
        )
        if hashlib.sha256(contract_path.read_bytes()).hexdigest() != manifest.get(
            "contract_sha256"
        ):
            raise ValueError("review contract differs from preparation manifest")
        contract = _read_json(contract_path)
        semantic_review = contract.get("semantic_review", {})
        reviewer_policy = semantic_review.get("calibration_reviewer_policy")
        policy_sha256 = manifest.get("calibration_reviewer_policy_sha256")
        if isinstance(reviewer_policy, dict):
            if canonical_sha256(reviewer_policy) != policy_sha256:
                raise ValueError("reviewer policy differs from preparation manifest")
        elif policy_sha256 is not None:
            raise ValueError("reviewer policy differs from preparation manifest")
    default_name = (
        "calibration-review-authorized-ai.json"
        if isinstance(reviewer_policy, dict)
        and "ai" in reviewer_policy.get("allowed_reviewer_types", [])
        else "human-review-independent.json"
    )
    human_path = (
        args.calibration_review or args.human_review or review_dir / default_name
    )
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
        reviewer_policy=reviewer_policy,
        review_contract=semantic_review,
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
