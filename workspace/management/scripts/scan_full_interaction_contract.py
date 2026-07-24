#!/usr/bin/env python3
"""Audit archived full runs for contract-instrumented interaction evidence.

The report is intentionally conservative: a file is compliant only when it
explicitly declares contract probe mode, a primary hook, and non-empty state
dimensions.  It scans management archives (and an optional extra root) without
rewriting historical evidence.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path


def assess(path: Path) -> tuple[bool, str]:
    try:
        row = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError):
        return False, "invalid_json"
    mode = str(row.get("probe_mode") or "")
    hooks = row.get("contract_hooks") or {}
    primary = bool(row.get("primary_present") is True)
    action_hooks = row.get("action_hooks") or []
    if isinstance(action_hooks, list):
        primary = primary or "primary" in action_hooks
    if isinstance(hooks, dict):
        primary = primary or hooks.get("primary_present") is True
        primary = primary or str(hooks.get("primary", "")).lower() in {
            "present",
            "ok",
            "true",
        }
    primary = primary or str(row.get("contract_hook_status", "")).lower() == "usable"
    dims = row.get("state_dimensions_changed") or row.get("state_dimensions") or []
    if mode != "contract":
        return False, "probe_mode_not_contract"
    if not primary:
        return False, "contract_instrumentation_missing:primary"
    if not isinstance(dims, (list, tuple, set)) or not dims:
        return False, "state_dimensions_changed_empty"
    return True, "ok"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, help="additional archive root")
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[3]
    roots = [repo / "workspace" / "management" / "runs"]
    if args.root:
        roots.append(args.root)
    files = sorted(
        {
            p
            for root in roots
            if root.exists()
            for p in root.rglob("browser-interaction.json")
        }
    )
    compliant = 0
    print("path\tcontract_full_eligible\treason")
    for path in files:
        ok, reason = assess(path)
        compliant += ok
        print(
            f"{path.relative_to(repo) if path.is_relative_to(repo) else path}\t{str(ok).lower()}\t{reason}"
        )
    print(f"summary\t{compliant}/{len(files)} compliant")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
