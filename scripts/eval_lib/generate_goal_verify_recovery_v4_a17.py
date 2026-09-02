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
BASE_CONTRACT_PATH = EVAL / "phase6-recovery-v4-a16-1-smoke-contract.json"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a17-smoke-contract.json"
CONTRACT_ID = "phase6-recovery-v4-20260902-a17-smoke-01"

SELECTED_PAIR_IDS = [
    "phase6-main-c05-task-05--pair-01",
    "phase6-main-c07-task-02--pair-02",
    "phase6-main-c07-task-08--pair-01",
    "phase6-main-c13-task-02--pair-02",
    "phase6-main-c14-task-08--pair-01",
    "phase6-main-c14-task-08--pair-02",
    "phase6-main-c14-task-08--pair-03",
    "phase6-main-c06-task-01--pair-01",
    "phase6-main-c08-task-01--pair-01",
]

TYPED_FIX_REPRODUCER_COMMANDS = {
    "phase6-main-c05-task-05--pair-01": "python3 cli.py 11",
    "phase6-main-c07-task-02--pair-02": "python3 app.py fixture/task-02.json",
    "phase6-main-c07-task-08--pair-01": "python3 app.py fixture/task-08.json",
    "phase6-main-c13-task-02--pair-02": ("python3 scripts/repro.py data/task-02.csv"),
    "phase6-main-c14-task-08--pair-01": ("node scripts/repro.mjs fixture/task-08.json"),
    "phase6-main-c14-task-08--pair-02": ("node scripts/repro.mjs fixture/task-08.json"),
    "phase6-main-c14-task-08--pair-03": ("node scripts/repro.mjs fixture/task-08.json"),
}


def build_contract(
    *, code_sha: str, exact_sha_ci_evidence: str, authorized: bool
) -> dict[str, Any]:
    base = _load(BASE_CONTRACT_PATH)
    if base.get("contract_id") != "phase6-recovery-v4-20260902-a16-1-smoke-01":
        raise ValueError("unexpected A16.1 base contract")
    if base.get("status") != "frozen" or "full_experiment" in base:
        raise ValueError("A17 must inherit the frozen A16.1 smoke contract")

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
    smoke = contract["smoke"]
    smoke.update(
        {
            "expected_pair_count": len(SELECTED_PAIR_IDS),
            "inference_role": (
                "post-A16.1 Recovery regression-lineage correction smoke; "
                "instrument diagnostic only"
            ),
            "minimum_executed_recovery_pairs": 4,
            "minimum_executed_recovery_pairs_per_real_profile": 1,
            "minimum_pairs_per_real_profile": 1,
            "selected_pair_ids": SELECTED_PAIR_IDS,
            "typed_fix_reproducer_commands": TYPED_FIX_REPRODUCER_COMMANDS,
        }
    )
    contract["analysis"].update(
        {
            "a16_1_historical_run_policy": (
                "A16.1 remains immutable six-pair NO-GO evidence and is never resumed, "
                "rescored, or pooled with A17"
            ),
            "recovery_regression_lineage_policy": (
                "initial fix and Recovery continuation resolve the identical registered "
                "completion-contract regression set and compare the same IDs and lineages"
            ),
            "candidate_failure_sampling_policy": (
                "retain the prior known generic failure pairs and all three repetitions of "
                "the prior Next.js task-08 failure to observe at least one Recovery path per "
                "real profile without changing any oracle"
            ),
        }
    )
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A17",
            "reason": (
                "A16.1 exposed that Recovery continuation recomputed profile-catalog "
                "regressions after the initial fix had bound registered contract checks"
            ),
            "historical_run_policy": (
                "A16.1 manifest, summary, ledger, report, and raw records remain immutable "
                "and retain their frozen NO-GO result"
            ),
            "correction_scope": (
                "reuse the same completion-contract regression resolver during Recovery and "
                "record its source and IDs in additive telemetry"
            ),
            "selected_pair_policy": (
                "preserve the A16.1 CLI/data targets and sentinels; add the second known "
                "generic failure and all task-08 Next.js repetitions to reduce initial-pass "
                "sampling gaps while requiring one executed Recovery per real profile"
            ),
            "inference_role": (
                "nine-pair instrument smoke only; no Recovery effect or generalization claim"
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
        description="Freeze the A17 Recovery regression-lineage correction smoke"
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
