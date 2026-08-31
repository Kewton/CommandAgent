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
BASE_CONTRACT_PATH = EVAL / "phase6-recovery-v4-a15-a6-smoke-contract.json"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a15-a7-smoke-contract.json"
CONTRACT_ID = "phase6-recovery-v4-20260831-a15-a7-smoke-01"


def build_contract(
    *, code_sha: str, exact_sha_ci_evidence: str, authorized: bool
) -> dict[str, Any]:
    base = _load(BASE_CONTRACT_PATH)
    if base.get("contract_id") != "phase6-recovery-v4-20260831-a15-a6-smoke-01":
        raise ValueError("unexpected A15-A6 base contract")
    if base.get("status") != "frozen" or "full_experiment" in base:
        raise ValueError("A15-A7 must inherit the frozen A15-A6 smoke contract")

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
            "recovery_fix_implement_mutation_policy": (
                "an automatic Recovery implement step carrying the immutable fix-origin "
                "binding may not complete successfully before a Write or Edit tool call; "
                "a read-only completion receives bounded host feedback and must either "
                "perform the repair or terminate honestly"
            ),
            "a15_a6_historical_run_policy": (
                "A15-A6 remains immutable instrument evidence: two data Recovery plans "
                "targeted pipeline/main.py but performed no Write or Edit, host verification "
                "rejected both treatments, and the run is never rescored"
            ),
        }
    )
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A15-A7",
            "reason": (
                "A15-A6 live observation showed that a semantically targeted Recovery plan "
                "could finish its implement step after inspection only, without mutating the "
                "broken artifact"
            ),
            "historical_run_policy": (
                "A15-A6 smoke-01 remains immutable NO-GO diagnostic evidence and is not "
                "changed, resumed, or rescored"
            ),
            "frozen_design_policy": (
                "selected pairs, tasks, task registry, model, prompts, source workspaces, "
                "external oracles, Recovery 0-vs-1 arms, exclusions, resource budgets, "
                "profile path coverage, report policy, and effect-claim prohibition remain "
                "unchanged"
            ),
            "inference_role": (
                "repeat the same frozen 14-pair instrument smoke under a new run ID; no "
                "effect claim"
            ),
            "product_findings": [
                "A15-A6 host final verification preserved all registered data commands",
                "data pair-02 and pair-03 Recovery plans named the correct repair artifact",
                "both Recovery implement steps used inspection tools but no Write or Edit",
                "host final verification rejected both unchanged treatments without harm",
                "automatic fix Recovery implement completion must require an actual mutation",
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
        description="Freeze the A15-A7 Recovery implement-mutation smoke"
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
