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
BASE_CONTRACT_PATH = EVAL / "phase6-recovery-v4-a15-a7-smoke-contract.json"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a15-a8-smoke-contract.json"
CONTRACT_ID = "phase6-recovery-v4-20260831-a15-a8-smoke-01"


def build_contract(
    *, code_sha: str, exact_sha_ci_evidence: str, authorized: bool
) -> dict[str, Any]:
    base = _load(BASE_CONTRACT_PATH)
    if base.get("contract_id") != "phase6-recovery-v4-20260831-a15-a7-smoke-01":
        raise ValueError("unexpected A15-A7 base contract")
    if base.get("status") != "frozen" or "full_experiment" in base:
        raise ValueError("A15-A8 must inherit the frozen A15-A7 smoke contract")

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
            "recovery_fix_typed_mutation_gate_policy": (
                "the immutable fix-origin plus implement-step binding explicitly activates "
                "the Write/Edit completion and mutation-before-short-circuit gates; this "
                "typed state does not depend on action words in model-generated instructions"
            ),
            "a15_a7_historical_run_policy": (
                "A15-A7 remains immutable instrument-GO evidence with three improvements "
                "and zero harms, but it is not evidence that the typed mutation gate fired: "
                "data pair-03 used a Repair instruction, performed only Read/Bash actions, "
                "and was safely rejected by host final verification"
            ),
        }
    )
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A15-A8",
            "reason": (
                "A15-A7 showed that the typed Recovery write policy was still gated by a "
                "legacy action-word heuristic, so a model-generated Repair instruction could "
                "avoid the intended completion feedback"
            ),
            "historical_run_policy": (
                "A15-A7 smoke-01 remains immutable instrument-GO evidence and is not changed, "
                "resumed, or rescored; its typed-mutation effect claim remains disallowed"
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
                "A15-A7 completed 14 of 14 records with instrument readiness GO",
                "A15-A7 observed three attributed improvements and zero harms",
                "A15-A7 data pair-03 retained control after a read-only Recovery failed",
                "the action-word predicate did not recognize the generated Repair instruction",
                "typed Recovery state must activate mutation enforcement without prompt parsing",
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
        description="Freeze the A15-A8 typed Recovery mutation-gate smoke"
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
