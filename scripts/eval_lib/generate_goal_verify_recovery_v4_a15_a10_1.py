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
BASE_CONTRACT_PATH = EVAL / "phase6-recovery-v4-a15-a10-full-contract.json"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a15-a10-1-full-contract.json"
CONTRACT_ID = "phase6-recovery-v4-20260901-a15-a10-1-live-01"
INHERITED_SMOKE_RUN_ID = "phase6-recovery-v4-20260901-a15-a9-smoke-01"


def build_contract(
    *, code_sha: str, exact_sha_ci_evidence: str, authorized: bool
) -> dict[str, Any]:
    base = _load(BASE_CONTRACT_PATH)
    if base.get("contract_id") != "phase6-recovery-v4-20260901-a15-a10-live-01":
        raise ValueError("unexpected A15-A10 base contract")
    if base.get("status") != "frozen" or "full_experiment" not in base:
        raise ValueError("A15-A10.1 must inherit the frozen A15-A10 full contract")
    if base.get("smoke_run_id") != INHERITED_SMOKE_RUN_ID:
        raise ValueError("A15-A10 no longer contains the inherited run-id defect")

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
        }
    )
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A15-A10.1",
            "reason": (
                "A15-A10 inherited the A15-A9 smoke_run_id and was rejected before "
                "manifest creation or product execution because the requested run "
                "directory did not match that stale identifier"
            ),
            "historical_run_policy": (
                "the frozen A15-A10 contract remains immutable; its attempted collection "
                "created no run directory, manifest, ledger, raw record, or product outcome"
            ),
            "correction_scope": (
                "change only contract_id, smoke_run_id, code_sha, exact-SHA evidence, "
                "supersession metadata, and authorization timestamp"
            ),
            "frozen_design_policy": (
                "all 140 selected pairs, task registry, inputs, model, prompts, external "
                "oracles, Recovery 0-vs-1 arms, maximum one Recovery, exclusions, four "
                "resource budgets, stopping rule, and 2,000-sample bootstrap are unchanged"
            ),
            "inference_role": (
                "pre-collection identifier correction with zero observed outcomes"
            ),
        }
    )
    contract["authorization"].update(
        {
            "full_collection_authorized": authorized,
            "approved_by": "repository owner" if authorized else None,
            "approved_at": "2026-09-01" if authorized else None,
        }
    )
    return contract


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Correct the inherited A15-A10 full-evaluation run ID"
    )
    parser.add_argument("--code-sha", required=True)
    parser.add_argument("--exact-sha-ci-evidence", required=True)
    parser.add_argument("--full-collection-authorized", action="store_true")
    args = parser.parse_args()
    contract = build_contract(
        code_sha=args.code_sha,
        exact_sha_ci_evidence=args.exact_sha_ci_evidence,
        authorized=args.full_collection_authorized,
    )
    _write_json(CONTRACT_PATH, contract)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
