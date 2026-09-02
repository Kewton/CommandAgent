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
BASE_CONTRACT_PATH = EVAL / "phase6-recovery-v4-a15-a5-smoke-contract.json"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a15-a6-smoke-contract.json"
CONTRACT_ID = "phase6-recovery-v4-20260831-a15-a6-smoke-01"


def build_contract(
    *, code_sha: str, exact_sha_ci_evidence: str, authorized: bool
) -> dict[str, Any]:
    base = _load(BASE_CONTRACT_PATH)
    if base.get("contract_id") != "phase6-recovery-v4-20260831-a15-a5-smoke-01":
        raise ValueError("unexpected A15-A5 base contract")
    if base.get("status") != "frozen" or "full_experiment" in base:
        raise ValueError("A15-A6 must inherit the frozen A15-A5 smoke contract")

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
    contract["analysis"].update(
        {
            "host_owned_recovery_verify_profile_policy": (
                "after Recovery StepPlan binding, host-owned final-success steps bypass "
                "profile runtime command canonicalization and execute the complete bound "
                "registered command list without shrinkage, substitution, or model rewrite"
            ),
            "a15_a5_partial_run_policy": (
                "A15-A5 stopped after 7 of 14 records when data profile runtime "
                "canonicalization shrank a three-command host final-success step to pytest; "
                "the partial ledger remains immutable and is never resumed or rescored"
            ),
        }
    )
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A15-A6",
            "reason": (
                "A15-A5 live observation showed that data profile runtime canonicalization "
                "ran after Recovery binding and replaced three registered host checks with "
                "one pytest command"
            ),
            "historical_run_policy": (
                "A15-A5 smoke-01 remains immutable incomplete diagnostic evidence at 7/14; "
                "it is never resumed, completed under a new binary, or rescored"
            ),
            "frozen_design_policy": (
                "selected pairs, tasks, task registry, model, prompts, source workspaces, "
                "external oracles, Recovery 0-vs-1 arms, exclusions, resource budgets, "
                "profile path coverage, and effect-claim prohibition remain unchanged"
            ),
            "inference_role": (
                "repeat the same frozen 14-pair instrument smoke under a new run ID; no "
                "effect claim"
            ),
            "product_findings": [
                "Recovery binding recorded all three data commands correctly",
                "data profile runtime canonicalization subsequently shrank the executable host step to pytest",
                "the later F2 reproducer still failed, so promotion was safely rejected",
                "host-owned Recovery steps must treat the bound command list as final authority",
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
    return contract


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Freeze the A15-A6 post-binding host verification smoke"
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
