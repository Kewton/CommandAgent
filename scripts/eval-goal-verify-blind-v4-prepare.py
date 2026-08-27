#!/usr/bin/env python3
from __future__ import annotations

import argparse
import collections
import hashlib
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_blind_v4 import (
    blank_review_row,
    canonical_sha256,
    human_sample,
    independent_human_template,
    prepare_semantic_items,
)


def write_json(path: Path, value) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def main() -> int:
    parser = argparse.ArgumentParser(description="Prepare Phase 6 v4 raw semantic review")
    parser.add_argument("--run-dir", type=Path, required=True)
    parser.add_argument(
        "--contract",
        type=Path,
        default=Path("eval/goal_verify/v0/phase6-preflight-v4-contract.json"),
    )
    args = parser.parse_args()
    run_dir = args.run_dir if args.run_dir.is_absolute() else ROOT / args.run_dir
    contract_path = args.contract if args.contract.is_absolute() else ROOT / args.contract
    contract_sha = hashlib.sha256(contract_path.read_bytes()).hexdigest()
    records = [
        json.loads(path.read_text(encoding="utf-8"))
        for path in sorted((run_dir / "raw").glob("**/pair-*.json"))
    ]
    items, mapping = prepare_semantic_items(
        records=records, contract_sha256=contract_sha
    )
    group_counts = collections.Counter(item["group_kind"] for item in items)
    oracle_references = [
        (row["pair_id"], row["source_lane"], oracle_index)
        for row in mapping.values()
        for oracle_index in row["source_oracle_indexes"]
    ]
    duplicate_oracle_references = len(oracle_references) - len(
        set(oracle_references)
    )
    if duplicate_oracle_references:
        raise ValueError(
            "semantic items contain duplicate source oracle references:"
            f"{duplicate_oracle_references}"
        )
    output = run_dir / "blind-review-v4"
    sample_ids = human_sample(items=items, mapping=mapping)
    write_json(output / "items-semantic-hidden.json", items)
    write_json(output / "secret" / "mapping.json", mapping)
    human_items = [
        next(item for item in items if item["item_id"] == item_id)
        for item_id in sample_ids
    ]
    write_json(
        output / "human-items-semantic-hidden.json",
        {
            "items_sha256": canonical_sha256(items),
            "source_boundary": "raw candidate claim-oracle groups only",
            "hidden_source_fields": True,
            "items": human_items,
        },
    )
    (output / "human-review-instructions.md").write_text(
        _human_markdown(human_items), encoding="utf-8"
    )
    write_json(
        output / "human-review-independent-template.json",
        independent_human_template(
            items_sha256=canonical_sha256(items), human_items=human_items
        ),
    )
    write_json(
        output / "model-review-template.json",
        {
            "items_sha256": canonical_sha256(items),
            "reviewer": {
                "provider": "",
                "model_id_or_version": "",
                "model_family": "",
                "invoked_at": "",
                "independent": True,
            },
            "reviews": [blank_review_row(item["item_id"]) for item in items],
        },
    )
    write_json(
        output / "manifest.json",
        {
            "schema_version": "commandagent.goal_verify.semantic_blind_manifest.v4",
            "contract_sha256": contract_sha,
            "campaign_manifest_sha256": hashlib.sha256(
                (run_dir / "campaign-manifest.json").read_bytes()
            ).hexdigest(),
            "record_ledger_head_sha256": json.loads(
                (run_dir / "campaign-summary.json").read_text(encoding="utf-8")
            )["record_ledger_head_sha256"],
            "record_count": len(records),
            "proposal_count": sum(len(record["lanes"]) for record in records),
            "item_count": len(items),
            "item_group_counts": dict(sorted(group_counts.items())),
            "oracle_reference_count": len(oracle_references),
            "unique_oracle_reference_count": len(set(oracle_references)),
            "duplicate_oracle_reference_count": duplicate_oracle_references,
            "items_sha256": canonical_sha256(items),
            "human_sample_count": len(sample_ids),
            "human_items_sha256": canonical_sha256(human_items),
            "preparation_script_sha256": hashlib.sha256(
                Path(__file__).read_bytes()
            ).hexdigest(),
            "preparation_module_sha256": hashlib.sha256(
                (ROOT / "scripts/eval_lib/goal_verify_blind_v4.py").read_bytes()
            ).hexdigest(),
            "source_boundary": "raw candidate claim-oracle groups only",
            "hidden": [
                "pair_id",
                "source_case_id",
                "source_lane",
                "provider",
                "model",
                "request_id",
                "execution results",
                "canonicalized spec",
                "machine score",
            ],
            "baseline_atomic_items": 0,
            "baseline_atomic_items_reason": "the predeclared semantic review unit is a raw candidate claim-oracle group; baseline execution observations are outside this source-blind review boundary",
            "secret_mapping": "secret/mapping.json",
        },
    )
    return 0


def _human_markdown(items: list[dict]) -> str:
    lines = [
        "# Phase 6 v4 source-blind human review",
        "",
        "Reviewer: independent human (identity recorded in the review document)",
        "",
        (
            "Judge only the visible raw claim-oracle group. Do not inspect model reviews, "
            "execution results, canonicalized output, machine scores, or "
            "`secret/mapping.json`."
        ),
        "",
        (
            "For every item, set `verdict` to `acceptable`, `needs_revision`, or "
            "`unusable`; set all five axes to true/false; and provide reason codes plus a "
            "non-empty rationale in `human-review-independent.json`."
        ),
        "",
    ]
    for index, item in enumerate(items, 1):
        lines.extend(
            [
                f"## {index}. {item['item_id']}",
                "",
                "```json",
                json.dumps(item, ensure_ascii=False, indent=2, sort_keys=True),
                "```",
                "",
            ]
        )
    return "\n".join(lines) + "\n"


if __name__ == "__main__":
    raise SystemExit(main())
