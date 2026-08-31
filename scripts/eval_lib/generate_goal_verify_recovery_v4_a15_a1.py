from __future__ import annotations

import argparse
import copy
from pathlib import Path
from typing import Any

from eval_lib.generate_goal_verify_main_v4 import _load, _write_json
from eval_lib.generate_goal_verify_recovery_v4_a14_a2 import ROOT

EVAL = ROOT / "eval/goal_verify/v0"
BASE_CONTRACT_PATH = EVAL / "phase6-recovery-v4-a15-smoke-contract.json"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a15-a1-smoke-contract.json"
CONTRACT_ID = "phase6-recovery-v4-20260831-a15-a1-smoke-01"


def _validate_exact_sha_evidence(
    *, code_sha: str, evidence_path: Path
) -> dict[str, Any]:
    evidence = _load(evidence_path)
    if evidence.get("head_sha") != code_sha:
        raise ValueError("exact-SHA evidence head_sha does not match --code-sha")
    workflows = {
        row.get("name"): (row.get("status"), row.get("conclusion"))
        for row in evidence.get("workflows", [])
        if isinstance(row, dict)
    }
    for name in ("CI", "acceptance"):
        if workflows.get(name) != ("completed", "success"):
            raise ValueError(f"exact-SHA evidence lacks successful {name}")
    return evidence


def build_contract(
    *, code_sha: str, exact_sha_ci_evidence: str, authorized: bool
) -> dict[str, Any]:
    base = _load(BASE_CONTRACT_PATH)
    if base.get("contract_id") != "phase6-recovery-v4-20260831-a15-smoke-01":
        raise ValueError("unexpected A15 base contract")
    if base.get("status") != "frozen" or "full_experiment" in base:
        raise ValueError("A15-A1 must inherit the frozen A15 smoke contract")

    evidence_path = (ROOT / exact_sha_ci_evidence).resolve()
    try:
        evidence_path.relative_to(ROOT.resolve())
    except ValueError as error:
        raise ValueError("exact-SHA evidence must be inside the repository") from error
    _validate_exact_sha_evidence(code_sha=code_sha, evidence_path=evidence_path)

    contract = copy.deepcopy(base)
    contract.update(
        {
            "contract_id": CONTRACT_ID,
            "smoke_run_id": CONTRACT_ID,
            "code_sha": code_sha,
            "exact_sha_ci_evidence": exact_sha_ci_evidence,
            "status": "frozen",
            "supersedes_contract": base["contract_id"],
            "supersedes_smoke_run": base["smoke_run_id"],
        }
    )
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A15-A1",
            "reason": (
                "correct Recovery observation isolation, typed data capability "
                "preflight, Next.js existing-tree preservation, read-only inspect, "
                "and safe existing-read path resolution found by A15 smoke"
            ),
            "historical_run_policy": (
                "A15 smoke-01 remains immutable NO-GO instrument evidence and is "
                "never rescored with the corrected product"
            ),
            "inference_role": (
                "repeat the same frozen 14-pair instrument smoke; no effect claim"
            ),
            "frozen_design_policy": (
                "selected pairs, external oracles, thresholds, exclusions, resource "
                "budgets, and Recovery 0-vs-1 treatment remain unchanged"
            ),
            "product_findings": [
                "Recovery preflight executed candidate-visible checks in the control workspace",
                "typed data capabilities were rejected before their registered checks ran",
                "Next.js inspection could replace an existing root app tree with src/app",
                "Inspect tool calls could mutate source artifacts",
                "a nonexistent app-prefixed read did not resolve to the unique registered existing path",
            ],
        }
    )
    contract["authorization"].update(
        {
            "smoke_collection_authorized": authorized,
            "full_collection_authorized": False,
            "approved_at": "2026-08-31" if authorized else None,
        }
    )
    contract["smoke"]["effect_claim_allowed"] = False
    contract["runner_sources"] = list(
        dict.fromkeys(
            [
                *contract["runner_sources"],
                "scripts/eval_lib/generate_goal_verify_recovery_v4_a15_a1.py",
            ]
        )
    )
    return contract


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Freeze the A15-A1 post-correction Recovery smoke contract"
    )
    parser.add_argument("--code-sha", required=True)
    parser.add_argument("--exact-sha-ci-evidence", required=True)
    parser.add_argument("--smoke-collection-authorized", action="store_true")
    args = parser.parse_args()
    contract = build_contract(
        code_sha=args.code_sha,
        exact_sha_ci_evidence=args.exact_sha_ci_evidence,
        authorized=args.smoke_collection_authorized,
    )
    _write_json(CONTRACT_PATH, contract)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
