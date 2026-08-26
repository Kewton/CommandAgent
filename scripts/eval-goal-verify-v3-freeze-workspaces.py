#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_workspaces_v3 import (
    load_workspace_registry,
    validate_provisioning,
    workspace_file_hashes,
)


def main() -> int:
    parser = argparse.ArgumentParser(description="Freeze Phase 6 v3 workspace hashes")
    parser.add_argument(
        "--registry",
        type=Path,
        default=Path("eval/goal_verify/v0/phase6-real-workspaces-v3.json"),
    )
    parser.add_argument("--execution-root", type=Path, required=True)
    args = parser.parse_args()
    registry_path = args.registry if args.registry.is_absolute() else ROOT / args.registry
    execution_root = (
        args.execution_root
        if args.execution_root.is_absolute()
        else ROOT / args.execution_root
    )
    registry = load_workspace_registry(registry_path)
    errors = validate_provisioning(registry, execution_root / "provisioned")
    if errors:
        raise ValueError("provisioning is not freeze-ready: " + ",".join(errors))
    for workspace in registry["workspaces"]:
        workspace["frozen_file_sha256"] = workspace_file_hashes(ROOT, workspace)
        workspace["status"] = "frozen"
    temporary = registry_path.with_suffix(registry_path.suffix + ".tmp")
    temporary.write_text(
        json.dumps(registry, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    os.replace(temporary, registry_path)
    print(f"froze {len(registry['workspaces'])} workspaces: {registry_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
