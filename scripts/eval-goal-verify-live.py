#!/usr/bin/env python3
from __future__ import annotations

import argparse
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_live import run_campaign


def main() -> int:
    parser = argparse.ArgumentParser(description="Run resumable Phase 6 local-provider pairs")
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
        "--prompt",
        type=Path,
        default=Path("eval/goal_verify/v0/verification-spec.prompt.txt"),
    )
    parser.add_argument(
        "--validator", type=Path, default=Path("target/release/verification_spec_validate")
    )
    parser.add_argument("--run-dir", type=Path, required=True)
    parser.add_argument("--execution-root", type=Path)
    parser.add_argument("--limit", type=int)
    args = parser.parse_args()
    paths = {
        name: path if path.is_absolute() else ROOT / path
        for name, path in vars(args).items()
        if isinstance(path, Path)
    }
    summary = run_campaign(
        root=ROOT,
        corpus_path=paths["corpus"],
        contract_path=paths["contract"],
        schema_path=paths["schema"],
        prompt_path=paths["prompt"],
        validator=paths["validator"],
        run_dir=paths["run_dir"],
        execution_root=paths.get("execution_root"),
        limit=args.limit,
    )
    print(
        f"[done] completed_pairs={summary['completed_pairs']}/{summary['target_pairs']} "
        f"valid_candidate_specs={summary['valid_candidate_specs']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
