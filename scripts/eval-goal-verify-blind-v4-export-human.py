#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_blind_v4 import (
    canonical_sha256,
    independent_human_template,
)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Export an isolated Phase 6 v4 human review packet"
    )
    parser.add_argument("--run-dir", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    run_dir = _resolve(args.run_dir)
    output_dir = _resolve(args.output_dir)
    if output_dir == run_dir or output_dir.is_relative_to(run_dir):
        raise ValueError("isolated human packet must be outside the run directory")
    if output_dir.exists() and any(output_dir.iterdir()):
        raise ValueError("isolated human packet output directory must be empty")

    review_dir = run_dir / "blind-review-v4"
    manifest = _read_json(review_dir / "manifest.json")
    packet = _read_json(review_dir / "human-items-semantic-hidden.json")
    human_items = packet["items"]
    if packet.get("items_sha256") != manifest.get("items_sha256"):
        raise ValueError("human packet does not match the full blind item set")
    if canonical_sha256(human_items) != manifest.get("human_items_sha256"):
        raise ValueError("human packet item hash differs from the blind manifest")
    expected_count = manifest.get("human_sample_count", 10)
    if len(human_items) != expected_count:
        raise ValueError(
            f"isolated human packet must contain exactly {expected_count} items"
        )

    output_dir.mkdir(parents=True, exist_ok=True)
    item_path = output_dir / "human-items-semantic-hidden.json"
    template_path = output_dir / "human-review-independent-template.json"
    instructions_path = output_dir / "README.md"
    _write_json(item_path, packet)
    _write_json(
        template_path,
        independent_human_template(
            items_sha256=manifest["items_sha256"], human_items=human_items
        ),
    )
    instructions_path.write_text(
        _instructions(item_count=expected_count), encoding="utf-8"
    )
    files = {
        path.name: hashlib.sha256(path.read_bytes()).hexdigest()
        for path in (item_path, template_path, instructions_path)
    }
    _write_json(
        output_dir / "packet-manifest.json",
        {
            "schema_version": "commandagent.goal_verify.human_review_packet.v4",
            "item_count": len(human_items),
            "items_sha256": manifest["items_sha256"],
            "human_items_sha256": manifest["human_items_sha256"],
            "files_sha256": files,
            "excluded": [
                "secret mapping",
                "model reviews",
                "execution results",
                "canonicalized output",
                "preflight report",
                "raw records",
                "prior reviewer output",
            ],
        },
    )
    print(output_dir)
    return 0


def _instructions(*, item_count: int) -> str:
    return f"""# Independent source-blind human review

Review only `human-items-semantic-hidden.json`. Do not request or inspect the source run,
model reviews, execution results, canonicalized output, preflight report, raw records, or
prior reviewer output.

Copy `human-review-independent-template.json` to `human-review-independent.json`. Set a
non-empty reviewer ID, keep `reviewer_type` as `human`, set
`contract_authoring_involvement` to `false` only if accurate, and confirm independence.
For all {item_count} items, set the verdict, all five boolean axes, reason codes, and a non-empty
rationale. Return only the completed JSON document.
"""


def _resolve(path: Path) -> Path:
    return path.resolve() if path.is_absolute() else (ROOT / path).resolve()


def _read_json(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def _write_json(path: Path, value) -> None:
    path.write_text(
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


if __name__ == "__main__":
    raise SystemExit(main())
