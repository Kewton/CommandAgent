from __future__ import annotations

import argparse
import copy
from typing import Any

from eval_lib.generate_goal_verify_main_v4 import _load, _write_json
from eval_lib.generate_goal_verify_recovery_v4_a14_a2 import ROOT
from eval_lib.generate_goal_verify_recovery_v4_a15_a1 import (
    _validate_exact_sha_evidence,
)

EVAL = ROOT / "eval/goal_verify/v0"
BASE_CONTRACT_PATH = EVAL / "phase6-recovery-v4-a15-a1-smoke-contract.json"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a15-a1-1-smoke-contract.json"
CONTRACT_ID = "phase6-recovery-v4-20260831-a15-a1-1-smoke-01"
NON_RUNTIME_GENERATOR = (
    "scripts/eval_lib/generate_goal_verify_recovery_v4_a15_a1.py"
)


def build_contract(
    *, code_sha: str, exact_sha_ci_evidence: str, authorized: bool
) -> dict[str, Any]:
    base = _load(BASE_CONTRACT_PATH)
    if base.get("contract_id") != "phase6-recovery-v4-20260831-a15-a1-smoke-01":
        raise ValueError("unexpected A15-A1 base contract")
    if base.get("status") != "frozen" or "full_experiment" in base:
        raise ValueError("A15-A1.1 must inherit the frozen A15-A1 smoke contract")

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
    contract["runner_sources"] = [
        source
        for source in contract["runner_sources"]
        if source != NON_RUNTIME_GENERATOR
    ]
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A15-A1.1",
            "reason": (
                "remove a contract-generation-only file from runner_sources after "
                "the A15-A1 frozen-input preflight rejected it before run creation"
            ),
            "historical_run_policy": (
                "A15-A1 remains an immutable pre-execution contract failure; no run "
                "directory, record, product invocation, or outcome was created"
            ),
            "inference_role": (
                "repeat the same frozen 14-pair instrument smoke; no effect claim"
            ),
            "frozen_design_policy": (
                "product SHA, selected pairs, external oracles, thresholds, exclusions, "
                "resource budgets, and Recovery 0-vs-1 treatment remain unchanged"
            ),
        }
    )
    contract["authorization"].update(
        {
            "smoke_collection_authorized": authorized,
            "full_collection_authorized": False,
            "approved_at": "2026-08-31" if authorized else None,
        }
    )
    return contract


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Freeze the A15-A1.1 runner-source correction"
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
