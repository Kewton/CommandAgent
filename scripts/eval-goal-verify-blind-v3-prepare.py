#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_blind_v3 import (
    human_sample,
    prepare_blind_items,
    records_to_blind_inputs,
)


def write_json(path: Path, value) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Prepare Phase 6 v3 blind review packets"
    )
    parser.add_argument("--run-dir", type=Path, required=True)
    parser.add_argument(
        "--contract",
        type=Path,
        default=Path("eval/goal_verify/v0/phase6-semantic-blind-v3-contract.json"),
    )
    args = parser.parse_args()
    run_dir = args.run_dir if args.run_dir.is_absolute() else ROOT / args.run_dir
    contract_path = (
        args.contract if args.contract.is_absolute() else ROOT / args.contract
    )
    contract_bytes = contract_path.read_bytes()
    contract_sha = hashlib.sha256(contract_bytes).hexdigest()
    records = [
        json.loads(path.read_text(encoding="utf-8"))
        for path in sorted((run_dir / "raw").glob("**/pair-*.json"))
    ]
    converted = records_to_blind_inputs(records)
    output = run_dir / "blind-review"
    secret = output / "secret"
    mappings = {}
    lane_items = {}
    for lane in ("semantic_hidden", "execution_visible"):
        items, mapping = prepare_blind_items(
            records=converted, contract_sha256=contract_sha, lane=lane
        )
        lane_items[lane] = items
        mappings[lane] = mapping
        write_json(output / f"items-{lane}.json", items)
    primary = sorted(
        {
            record["source_case_id"]
            for record in records
            if record.get("cell_lane") == "primary"
        }
    )
    if len(primary) != 7:
        raise ValueError(f"expected 7 primary cells, found {len(primary)}")
    sample_ids = human_sample(lane_items["semantic_hidden"], primary)
    write_json(
        output / "human-calibration-template.json",
        {
            "lane": "semantic_hidden",
            "item_ids": sample_ids,
            "reviews": [
                {
                    "item_id": item_id,
                    "reviewer_id": "",
                    "reviewed_at": "",
                    "preferred": "",
                    "reason_codes": [],
                    "rationale": "",
                }
                for item_id in sample_ids
            ],
        },
    )
    write_json(secret / "mapping.json", mappings)
    write_json(
        output / "manifest.json",
        {
            "contract_sha256": contract_sha,
            "record_count": len(records),
            "semantic_hidden_items": len(lane_items["semantic_hidden"]),
            "semantic_hidden_items_sha256": hashlib.sha256(
                json.dumps(
                    lane_items["semantic_hidden"],
                    ensure_ascii=False,
                    separators=(",", ":"),
                    sort_keys=True,
                ).encode()
            ).hexdigest(),
            "execution_visible_items": len(lane_items["execution_visible"]),
            "human_sample_items": len(sample_ids),
            "secret_mapping": "secret/mapping.json",
        },
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
