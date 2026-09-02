#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from pathlib import Path

from eval_lib.goal_verify_recovery_exposure_corpus_pilot import (
    DEFAULT_TASK_REGISTRY,
    DEFAULT_WORKSPACE_REGISTRY,
    run_pilot,
)


def main() -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Qualify generic/data/Next.js candidate-visible Recovery failures "
            "without running a model"
        )
    )
    parser.add_argument("--run-dir", type=Path, required=True)
    parser.add_argument("--execution-root", type=Path, required=True)
    parser.add_argument("--provisioned-root", type=Path, required=True)
    parser.add_argument("--task-registry", type=Path, default=DEFAULT_TASK_REGISTRY)
    parser.add_argument(
        "--workspace-registry", type=Path, default=DEFAULT_WORKSPACE_REGISTRY
    )
    parser.add_argument("--timeout-sec", type=int, default=120)
    args = parser.parse_args()
    report = run_pilot(
        run_dir=args.run_dir,
        execution_root=args.execution_root,
        provisioned_root=args.provisioned_root,
        task_registry_path=args.task_registry,
        workspace_registry_path=args.workspace_registry,
        timeout_sec=args.timeout_sec,
    )
    print(json.dumps(report, ensure_ascii=False, sort_keys=True))
    return 0 if report["corpus_ready_for_preregistration"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
