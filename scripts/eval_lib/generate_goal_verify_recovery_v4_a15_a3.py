from __future__ import annotations

import argparse
import copy
from typing import Any

from eval_lib.generate_goal_verify_main_v4 import _load, _write_json
from eval_lib.generate_goal_verify_recovery_v4_a14_a2 import ROOT
from eval_lib.generate_goal_verify_recovery_v4_a15_a1 import (
    _validate_exact_sha_evidence,
)
from eval_lib.goal_verify_recovery_experiment_v4 import (
    RECOVERY_FIX_TERMINAL_OUTCOME_POLICY,
    SMOKE_PROFILE_PATH_COVERAGE_POLICY,
)

EVAL = ROOT / "eval/goal_verify/v0"
BASE_CONTRACT_PATH = EVAL / "phase6-recovery-v4-a15-a2-smoke-contract.json"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a15-a3-smoke-contract.json"
CONTRACT_ID = "phase6-recovery-v4-20260831-a15-a3-smoke-01"


def build_contract(
    *, code_sha: str, exact_sha_ci_evidence: str, authorized: bool
) -> dict[str, Any]:
    base = _load(BASE_CONTRACT_PATH)
    if base.get("contract_id") != "phase6-recovery-v4-20260831-a15-a2-smoke-01":
        raise ValueError("unexpected A15-A2 base contract")
    if base.get("status") != "frozen" or "full_experiment" in base:
        raise ValueError("A15-A3 must inherit the frozen A15-A2 smoke contract")

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
    contract["smoke"]["recovery_fix_terminal_outcome_policy"] = copy.deepcopy(
        RECOVERY_FIX_TERMINAL_OUTCOME_POLICY
    )
    contract["smoke"]["real_profile_path_coverage_policy"] = copy.deepcopy(
        SMOKE_PROFILE_PATH_COVERAGE_POLICY
    )
    contract["analysis"].update(
        {
            "recovery_fix_terminal_gate_semantics": (
                "a successful treatment must pass completion and be promoted; a failed "
                "treatment is instrument-valid only when it terminates honestly as "
                "not_recoverable, is rejected, and retains the unchanged control"
            ),
            "real_profile_smoke_path_semantics": (
                "executed Recovery establishes repair-path coverage; all-pass coverage "
                "with an explicit current-success suppression establishes safety-path "
                "coverage only and is never a profile-specific repair-effect claim"
            ),
        }
    )
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A15-A3",
            "reason": (
                "A15-A2 omitted its prereviewed honest-terminal alternative and made "
                "instrument readiness depend on every executed Recovery succeeding; "
                "its per-profile repair-only coverage gate also conflicted with the "
                "mandatory current-success suppression path"
            ),
            "historical_run_policy": (
                "A15-A2 smoke-01 remains immutable NO-GO evidence and is never rescored"
            ),
            "frozen_design_policy": (
                "selected pairs, model, prompts, task inputs, external oracles, Recovery "
                "0-vs-1 arms, exclusions, resource budgets, and effect-claim prohibition "
                "remain unchanged"
            ),
            "inference_role": (
                "repeat the same frozen 14-pair instrument smoke; no effect claim"
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
        description="Freeze the A15-A3 honest-terminal Recovery smoke"
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
