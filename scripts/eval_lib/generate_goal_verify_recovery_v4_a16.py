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
BASE_CONTRACT_PATH = EVAL / "phase6-recovery-v4-a15-a10-2-full-contract.json"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a16-smoke-contract.json"
CONTRACT_ID = "phase6-recovery-v4-20260902-a16-smoke-01"

SELECTED_PAIR_IDS = [
    "phase6-main-c05-task-05--pair-01",
    "phase6-main-c07-task-02--pair-02",
    "phase6-main-c13-task-02--pair-02",
    "phase6-main-c14-task-08--pair-01",
    "phase6-main-c06-task-01--pair-01",
    "phase6-main-c08-task-01--pair-01",
]

TYPED_FIX_REPRODUCER_COMMANDS = {
    "phase6-main-c05-task-05--pair-01": "python3 cli.py 11",
    "phase6-main-c07-task-02--pair-02": "python3 app.py fixture/task-02.json",
    "phase6-main-c13-task-02--pair-02": ("python3 scripts/repro.py data/task-02.csv"),
    "phase6-main-c14-task-08--pair-01": ("node scripts/repro.mjs fixture/task-08.json"),
}

READINESS_CHECK = "discarded_valid_treatment_zero"


def build_contract(
    *, code_sha: str, exact_sha_ci_evidence: str, authorized: bool
) -> dict[str, Any]:
    base = _load(BASE_CONTRACT_PATH)
    if base.get("contract_id") != ("phase6-recovery-v4-20260901-a15-a10-2-live-01"):
        raise ValueError("unexpected A15-A10.2 base contract")
    if base.get("status") != "frozen" or "full_experiment" not in base:
        raise ValueError("A16 must inherit the frozen A15-A10.2 full contract")

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
    contract.pop("full_experiment", None)
    contract.pop("partial_run_evidence", None)

    smoke = contract["smoke"]
    smoke.update(
        {
            "effect_claim_allowed": False,
            "expected_pair_count": len(SELECTED_PAIR_IDS),
            "inference_role": (
                "post-A15-A10.2 contract-bound Recovery correction smoke; "
                "instrument diagnostic only"
            ),
            "minimum_executed_recovery_pairs": 4,
            "minimum_executed_recovery_pairs_per_real_profile": 1,
            "minimum_pairs_per_real_profile": 1,
            "required_real_profiles": ["cli", "generic", "data", "nextjs"],
            "selected_pair_ids": SELECTED_PAIR_IDS,
            "typed_fix_reproducer_commands": TYPED_FIX_REPRODUCER_COMMANDS,
            "require_discarded_valid_treatment_zero": True,
        }
    )
    smoke.pop("real_profile_path_coverage_policy", None)
    checks = smoke.setdefault("required_readiness_checks", [])
    if READINESS_CHECK not in checks:
        checks.append(READINESS_CHECK)

    contract["analysis"].update(
        {
            "a15_a10_2_historical_run_policy": (
                "A15-A10.2 remains immutable NO-GO evidence and is never resumed, "
                "rescored, or pooled with A16"
            ),
            "registered_contract_probe_binding_policy": (
                "profile probes use the registered reproducer fixture and argv; "
                "fix acceptance admits only registered completion-contract checks"
            ),
            "typed_recovery_observation_policy": (
                "pre- and post-Recovery observations exclude only contract-declared "
                "generated outputs while continuing to reject protected source changes"
            ),
            "discarded_valid_treatment_policy": (
                "a treatment satisfying every registered oracle must not be discarded; "
                "the smoke gate requires zero such attempts"
            ),
        }
    )
    runner_source = "scripts/eval_lib/generate_goal_verify_recovery_v4_a16.py"
    contract["runner_sources"] = list(
        dict.fromkeys([*contract["runner_sources"], runner_source])
    )
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A16",
            "reason": (
                "A15-A10.2 identified registered-oracle-valid data treatments that "
                "product-internal probes and Recovery observations discarded"
            ),
            "historical_run_policy": (
                "A15-A10.2 manifest, summary, ledger, report, and raw records remain "
                "immutable and retain their frozen NO-GO result"
            ),
            "correction_scope": (
                "bind data probes to the registered input, constrain fix regressions "
                "to the completion contract, type generated-output observation effects, "
                "and correct final acceptance telemetry"
            ),
            "selected_pair_policy": (
                "one prior candidate-visible failure per real profile, including the "
                "data preflight-blocked task-02 pair, plus two ineligible sentinels"
            ),
            "inference_role": (
                "six-pair instrument smoke only; no Recovery effect or generalization claim"
            ),
        }
    )
    contract["authorization"].update(
        {
            "smoke_collection_authorized": authorized,
            "full_collection_authorized": False,
            "approved_by": "repository owner" if authorized else None,
            "approved_at": "2026-09-02" if authorized else None,
        }
    )
    return contract


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Freeze the A16 contract-bound Recovery correction smoke"
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
