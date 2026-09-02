from __future__ import annotations

import argparse
import json
from pathlib import Path

from eval_lib.goal_verify_recovery_confirmatory import run_confirmatory


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Run the preregistered conditional Recovery experiment"
    )
    parser.add_argument("--contract", type=Path, required=True)
    parser.add_argument("--commandagent-bin", type=Path, required=True)
    parser.add_argument("--nextjs-node-modules", type=Path, required=True)
    parser.add_argument("--run-dir", type=Path, required=True)
    parser.add_argument("--execution-root", type=Path)
    parser.add_argument("--timeout-sec", type=int, default=180)
    args = parser.parse_args()
    contract = json.loads(args.contract.read_text(encoding="utf-8"))
    report = run_confirmatory(
        contract=contract,
        contract_path=args.contract,
        commandagent_bin=args.commandagent_bin,
        node_modules_source=args.nextjs_node_modules,
        run_dir=args.run_dir,
        execution_root=args.execution_root,
        timeout_sec=args.timeout_sec,
    )
    print(json.dumps(report, ensure_ascii=False, sort_keys=True))
    return 0 if report["status"] == "GO" else 1


if __name__ == "__main__":
    raise SystemExit(main())
