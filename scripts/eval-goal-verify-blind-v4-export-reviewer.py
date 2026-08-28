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
    authorized_ai_reviewer_template,
    canonical_sha256,
)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Export an isolated Phase 6 v4 authorized reviewer packet"
    )
    parser.add_argument("--run-dir", type=Path, required=True)
    parser.add_argument("--contract", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    run_dir = _resolve(args.run_dir)
    contract_path = _resolve(args.contract)
    output_dir = _resolve(args.output_dir)
    if output_dir == run_dir or output_dir.is_relative_to(run_dir):
        raise ValueError("isolated reviewer packet must be outside the run directory")
    if output_dir.exists() and any(output_dir.iterdir()):
        raise ValueError("isolated reviewer packet output directory must be empty")

    review_dir = run_dir / "blind-review-v4"
    manifest = _read_json(review_dir / "manifest.json")
    contract = _read_json(contract_path)
    contract_sha256 = hashlib.sha256(contract_path.read_bytes()).hexdigest()
    if contract_sha256 != manifest.get("contract_sha256"):
        raise ValueError("reviewer packet contract differs from preparation manifest")
    policy = contract.get("semantic_review", {}).get("calibration_reviewer_policy")
    sample_spec = contract.get("semantic_review", {}).get("main_sample")
    expected_count = sample_spec.get("size") if isinstance(sample_spec, dict) else 10
    if not isinstance(policy, dict) or "ai" not in policy.get(
        "allowed_reviewer_types", []
    ):
        raise ValueError("contract does not authorize an AI calibration reviewer")
    if canonical_sha256(policy) != manifest.get("calibration_reviewer_policy_sha256"):
        raise ValueError("reviewer policy differs from preparation manifest")

    packet = _read_json(review_dir / "human-items-semantic-hidden.json")
    items = packet["items"]
    if packet.get("items_sha256") != manifest.get("items_sha256"):
        raise ValueError("reviewer packet does not match the full blind item set")
    if canonical_sha256(items) != manifest.get("human_items_sha256"):
        raise ValueError("reviewer packet item hash differs from the blind manifest")
    if len(items) != expected_count:
        raise ValueError(
            f"isolated calibration packet must contain exactly {expected_count} items"
        )

    output_dir.mkdir(parents=True, exist_ok=True)
    item_path = output_dir / "calibration-items-semantic-hidden.json"
    template_path = output_dir / "calibration-review-authorized-ai-template.json"
    instructions_path = output_dir / "README.md"
    _write_json(item_path, packet)
    template = authorized_ai_reviewer_template(
        items_sha256=manifest["items_sha256"],
        human_items=items,
        reviewer_policy=policy,
    )
    _write_json(template_path, template)
    instructions_path.write_text(
        _instructions(template, item_count=expected_count), encoding="utf-8"
    )
    files = {
        path.name: hashlib.sha256(path.read_bytes()).hexdigest()
        for path in (item_path, template_path, instructions_path)
    }
    _write_json(
        output_dir / "packet-manifest.json",
        {
            "schema_version": (
                "commandagent.goal_verify.authorized_ai_review_packet.v4"
            ),
            "contract_sha256": contract_sha256,
            "reviewer_policy_sha256": canonical_sha256(policy),
            "item_count": len(items),
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


def _instructions(template: dict, *, item_count: int) -> str:
    return f"""# User-authorized source-blind AI calibration review

The repository owner explicitly authorized this AI reviewer for the frozen contract:

- reviewer_id: `{template["reviewer_id"]}`
- provider: `{template["provider"]}`
- model_family: `{template["model_family"]}`
- model_id_or_version: `{template["model_id_or_version"]}`
- authorization_id: `{template["authorization_id"]}`
- contract_authoring_involvement: `{str(template["contract_authoring_involvement"]).lower()}`

Review only `calibration-items-semantic-hidden.json`. Do not request or inspect the
source run, model reviews, execution results, canonicalized output, preflight report,
raw records, secret mapping, or prior reviewer output. Contract-authoring involvement
is disclosed and does not imply access to those forbidden result materials.

Copy `calibration-review-authorized-ai-template.json` to
`calibration-review-authorized-ai.json`. Do not alter authorization metadata, hashes,
item IDs, or item order. Set `source_blind_confirmed`,
`forbidden_materials_not_accessed`, and `reviewer_output_independence_confirmed` to true
only if accurate, set a non-empty ISO-8601 `invoked_at`, and complete all {item_count} reviews.
Every review requires one verdict (`acceptable`, `needs_revision`, or `unusable`), all
five boolean axes, a reason-code array, and a non-empty rationale. Return only the
completed JSON document.
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
