#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_live import run_campaign
from eval_lib.goal_verify_live_v3 import run_campaign_v3
from eval_lib.goal_verify_live_v4 import run_campaign_v4


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Run resumable Phase 6 local-provider pairs"
    )
    parser.add_argument(
        "--corpus", type=Path, default=Path("eval/goal_verify/v0/corpus.json")
    )
    parser.add_argument(
        "--contract",
        type=Path,
        default=Path("eval/goal_verify/v0/phase6-paired-contract.json"),
    )
    parser.add_argument(
        "--schema",
        type=Path,
        default=Path("eval/goal_verify/v0/verification-spec.schema.json"),
    )
    parser.add_argument(
        "--prompt", type=Path, help="must equal contract.generation.prompt when set"
    )
    parser.add_argument(
        "--validator",
        type=Path,
        default=Path("target/release/verification_spec_validate"),
    )
    parser.add_argument("--run-dir", type=Path, required=True)
    parser.add_argument("--execution-root", type=Path)
    parser.add_argument("--commandagent-bin", type=Path)
    parser.add_argument("--limit", type=int)
    args = parser.parse_args()
    paths = {
        name: path if path.is_absolute() else ROOT / path
        for name, path in vars(args).items()
        if isinstance(path, Path)
    }
    contract_value = json.loads(paths["contract"].read_text(encoding="utf-8"))
    schema_version = contract_value.get("schema_version")
    runner = {
        "commandagent.goal_verify.phase6_preflight_contract.v3": run_campaign_v3,
        "commandagent.goal_verify.phase6_preflight_contract.v4": run_campaign_v4,
    }.get(schema_version, run_campaign)
    if runner in {run_campaign_v3, run_campaign_v4} and "execution_root" not in paths:
        parser.error("v3/v4 requires --execution-root")
    runner_args = {
        "root": ROOT,
        "corpus_path": paths["corpus"],
        "contract_path": paths["contract"],
        "schema_path": paths["schema"],
        "prompt_path": paths.get("prompt"),
        "validator": paths["validator"],
        "run_dir": paths["run_dir"],
        "execution_root": paths.get("execution_root"),
        "limit": args.limit,
    }
    if runner in {run_campaign_v3, run_campaign_v4}:
        runner_args["commandagent_bin"] = paths.get("commandagent_bin")
    summary = runner(**runner_args)
    print(
        f"[done] completed_pairs={summary['completed_pairs']}/{summary['target_pairs']} "
        f"completed_proposals={summary.get('completed_proposals', 'n/a')}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
