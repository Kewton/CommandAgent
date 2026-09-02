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
BASE_CONTRACT_PATH = EVAL / "phase6-recovery-v4-a15-a8-smoke-contract.json"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a15-a9-smoke-contract.json"
CONTRACT_ID = "phase6-recovery-v4-20260901-a15-a9-smoke-01"

READINESS_CHECKS = [
    "recovery_handoff_fidelity_v2",
    "recovery_product_mutation_observation",
    "recovery_fix_safety_verification",
    "recovery_bounded_local_repair_max_one",
    "recovery_treatment_delta",
]


def build_contract(
    *, code_sha: str, exact_sha_ci_evidence: str, authorized: bool
) -> dict[str, Any]:
    base = _load(BASE_CONTRACT_PATH)
    if base.get("contract_id") != "phase6-recovery-v4-20260831-a15-a8-smoke-01":
        raise ValueError("unexpected A15-A8 base contract")
    if base.get("status") != "frozen" or "full_experiment" in base:
        raise ValueError("A15-A9 must inherit the frozen A15-A8 smoke contract")

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
            "recovery_handoff_fidelity_v2_policy": (
                "automatic fix Recovery must carry the top-level CompletionContract goal, "
                "registered verify commands, and deterministic repair targets; incomplete "
                "handoffs fail closed before another model run"
            ),
            "recovery_api_preservation_policy": (
                "after a Recovery mutation, host-owned deterministic analysis preserves only "
                "existing caller- or registered-contract-referenced API surface; violations "
                "receive at most one bounded local repair using the failed command and stderr"
            ),
            "recovery_product_mutation_measurement_policy": (
                "mutation is measured by product-artifact bytes while runtime and cache paths "
                "are excluded; Write/Edit tool names are diagnostic only"
            ),
            "recovery_rejected_treatment_delta_policy": (
                "the attempted product delta and treatment runtime-evidence delta are recorded "
                "before promotion or rejection and remain distinct from the adopted control delta"
            ),
            "a15_a8_historical_run_policy": (
                "A15-A8 remains immutable instrument-GO evidence; its data pair-01 and pair-02 "
                "treatments corrected used_rows but removed caller-required write_outputs and "
                "were safely rejected, so adopted delta zero must not be read as no mutation"
            ),
            "recovery_runs_above_one": "out of scope and forbidden",
        }
    )
    smoke = contract["smoke"]
    smoke.update(
        {
            "require_recovery_handoff_fidelity_v2": True,
            "require_recovery_product_mutation_observation": True,
            "require_recovery_fix_safety_verification": True,
            "require_recovery_bounded_local_repair_max_one": True,
            "require_recovery_treatment_delta": True,
        }
    )
    checks = smoke.setdefault("required_readiness_checks", [])
    for check in READINESS_CHECKS:
        if check not in checks:
            checks.append(check)
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A15-A9",
            "reason": (
                "A15-A8 exposed semantically incomplete Recovery handoff data and destructive "
                "whole-file rewrites that fixed the focal defect while deleting a required API"
            ),
            "historical_run_policy": (
                "A15-A8 smoke-01 remains immutable instrument-GO evidence and is not changed, "
                "resumed, or rescored"
            ),
            "frozen_design_policy": (
                "selected pairs, tasks, task registry, model, prompts, source workspaces, "
                "external oracles, Recovery 0-vs-1 arms, exclusions, resource budgets, profile "
                "path coverage, report policy, and effect-claim prohibition remain unchanged"
            ),
            "inference_role": (
                "repeat the same frozen 14-pair instrument smoke under a new run ID; no "
                "Recovery-effect or all-profile quality claim"
            ),
            "product_findings": [
                "A15-A8 completed 14 of 14 records with instrument readiness GO",
                "data pair-01 and pair-02 fixed used_rows but deleted caller-required write_outputs",
                "promotion rejection retained control, making the adopted artifact delta zero",
                "the Recovery handoff reused a nested step prompt and initially carried no commands or targets",
                "one bounded local repair may restore the required API inside the same Recovery",
                "a second automatic Recovery remains forbidden",
            ],
        }
    )
    contract["authorization"].update(
        {
            "smoke_collection_authorized": authorized,
            "full_collection_authorized": False,
            "approved_at": "2026-09-01" if authorized else None,
        }
    )
    return contract


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Freeze the A15-A9 Recovery fidelity and bounded-repair smoke"
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
