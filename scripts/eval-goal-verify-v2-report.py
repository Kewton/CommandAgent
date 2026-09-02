#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_preflight_report_v2 import build_preflight_report


def load_json(path: Path):
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise TypeError(f"expected JSON object: {path}")
    return value


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser(description="Build Phase 6 v2 preflight report")
    parser.add_argument("--run-dir", type=Path, required=True)
    parser.add_argument(
        "--contract",
        type=Path,
        default=Path("eval/goal_verify/v0/phase6-preflight-v2-contract.json"),
    )
    parser.add_argument(
        "--adapters",
        type=Path,
        default=Path("eval/goal_verify/v0/phase6-command-adapters-v2.json"),
    )
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    contract_path = args.contract if args.contract.is_absolute() else ROOT / args.contract
    adapters_path = args.adapters if args.adapters.is_absolute() else ROOT / args.adapters
    run_dir = args.run_dir if args.run_dir.is_absolute() else ROOT / args.run_dir
    output = args.output if args.output.is_absolute() else ROOT / args.output
    if output.exists():
        raise FileExistsError(f"refusing to overwrite preflight report: {output}")
    record_paths = sorted((run_dir / "raw").glob("**/pair-*.json"))
    records = [load_json(path) for path in record_paths]
    report = build_preflight_report(
        records=records,
        contract=load_json(contract_path),
        adapters=load_json(adapters_path)["adapters"],
    )
    report["integrity"] = {
        "contract_sha256": sha256(contract_path),
        "adapter_registry_sha256": sha256(adapters_path),
        "record_sha256": {
            str(path.relative_to(ROOT)): sha256(path) for path in record_paths
        },
        "reporter_source_sha256": sha256(Path(__file__)),
        "reporter_library_sha256": sha256(
            ROOT / "scripts/eval_lib/goal_verify_preflight_report_v2.py"
        ),
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(
        json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(json.dumps(report, ensure_ascii=False, sort_keys=True))
    return 0 if report["ready_for_full_experiment_design"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
