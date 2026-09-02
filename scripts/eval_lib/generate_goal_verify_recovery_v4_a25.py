from __future__ import annotations

import argparse
import copy
import hashlib
from typing import Any

from eval_lib.generate_goal_verify_main_v4 import _load, _write_json
from eval_lib.generate_goal_verify_recovery_v4_a14_a2 import ROOT
from eval_lib.generate_goal_verify_recovery_v4_a15_a1 import (
    _validate_exact_sha_evidence,
)
from eval_lib.goal_verify_recovery_a25_report import (
    RECOVERY_INNER_VERIFY_BINDING_POLICY_V2,
)

EVAL = ROOT / "eval/goal_verify/v0"
BASE_CONTRACT_PATH = EVAL / "phase6-recovery-v4-a24-pilot-contract.json"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a25-pilot-contract.json"
CONTRACT_ID = "phase6-recovery-v4-20260902-a25-pilot-01"
MODEL = "qwen3.5:9b"
MODEL_DIGEST = "6488c96fa5faab64bb65cbd30d4289e20e6130ef535a93ef9a49f42eda893ea7"

SELECTED_CASE_IDS = [
    "phase6-main-c07-task-01",
    "phase6-main-c07-task-05",
    "phase6-main-c13-task-01",
    "phase6-main-c13-task-05",
    "phase6-main-c14-task-01",
    "phase6-main-c14-task-05",
]
CASE_REPRODUCERS = {
    "phase6-main-c07-task-01": "python3 app.py fixture/task-01.json",
    "phase6-main-c07-task-05": "python3 app.py fixture/task-05.json",
    "phase6-main-c13-task-01": "python3 scripts/repro.py data/task-01.csv",
    "phase6-main-c13-task-05": "python3 scripts/repro.py data/task-05.csv",
    "phase6-main-c14-task-01": "node scripts/repro.mjs fixture/task-01.json",
    "phase6-main-c14-task-05": "node scripts/repro.mjs fixture/task-05.json",
}
SELECTED_PAIR_IDS = [f"{case_id}--pair-01" for case_id in SELECTED_CASE_IDS]
TYPED_FIX_REPRODUCER_COMMANDS = {
    f"{case_id}--pair-01": CASE_REPRODUCERS[case_id] for case_id in SELECTED_CASE_IDS
}
AUTHORITATIVE_REPORT_SOURCES = [
    "scripts/eval-goal-verify-recovery-a25-report.py",
    "scripts/eval_lib/goal_verify_live.py",
    "scripts/eval_lib/goal_verify_recovery_a25_report.py",
    "scripts/eval_lib/goal_verify_recovery_a23_report.py",
    "scripts/eval_lib/goal_verify_recovery_a15_report.py",
    "scripts/eval_lib/goal_verify_recovery_report_v4.py",
    "scripts/eval_lib/goal_verify_recovery_experiment_v4.py",
    "scripts/eval_lib/goal_verify_recovery_full_report_v4.py",
    "scripts/eval_lib/goal_verify_stats_v2.py",
]


def build_contract(
    *,
    code_sha: str,
    exact_sha_ci_evidence: str,
    authorized: bool,
) -> dict[str, Any]:
    base = _load(BASE_CONTRACT_PATH)
    if base.get("contract_id") != "phase6-recovery-v4-20260902-a24-pilot-01":
        raise ValueError("unexpected A24 base contract")
    if base.get("status") != "frozen" or "full_experiment" in base:
        raise ValueError("A25 must inherit the frozen A24 pilot contract")

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
            "model": MODEL,
            "model_digest": MODEL_DIGEST,
            "status": "frozen",
            "supersedes_contract": base["contract_id"],
            "supersedes_smoke_run": base["smoke_run_id"],
        }
    )
    pilot = contract["pilot_design"]
    pilot.update(
        {
            "schema_version": (
                "commandagent.goal_verify.recovery_natural_exposure_pilot.v3"
            ),
            "inference_role": (
                "forward-only inner-binding instrument validation and natural-exposure "
                "design only"
            ),
            "selection_basis": (
                "two A17/A21/A23/A24-unseen task clusters per profile, selected before "
                "the first A25 model invocation"
            ),
            "model_selection_basis": (
                "retain the pre-A24 digest-pinned exploratory model without using A24 "
                "outcomes for model selection"
            ),
            "forbidden_uses": [
                "Recovery effect estimate",
                "cross-profile generalization claim",
                "pooling with A21, A23, A24, or a later confirmatory run",
                "post-collection pair replacement within A25",
                "rescoring A23 or A24 with the A25 report policy",
            ],
            "authoritative_report_command": (
                "scripts/eval-goal-verify-recovery-a25-report.py "
                "--contract eval/goal_verify/v0/"
                "phase6-recovery-v4-a25-pilot-contract.json --run-dir "
                "dev-reports/issue-399/runs/"
                "phase6-recovery-v4-20260902-a25-pilot-01"
            ),
            "authoritative_report_source_sha256": {
                relative: hashlib.sha256((ROOT / relative).read_bytes()).hexdigest()
                for relative in AUTHORITATIVE_REPORT_SOURCES
            },
            "fallback_when_threshold_not_met": (
                "preregister a deterministic fault-boundary conditional Recovery "
                "experiment instead of enlarging this natural-exposure sample"
            ),
            "invalid_pilot_policy": (
                "diagnose instrument failure without pair replacement, denominator "
                "resizing, effect estimation, or next-design selection"
            ),
        }
    )
    smoke = contract["smoke"]
    smoke.update(
        {
            "expected_pair_count": len(SELECTED_PAIR_IDS),
            "inference_role": (
                "independent forward-only inner-binding Recovery exposure pilot"
            ),
            "selected_pair_ids": SELECTED_PAIR_IDS,
            "typed_fix_reproducer_commands": TYPED_FIX_REPRODUCER_COMMANDS,
            "recovery_inner_verify_binding_policy": copy.deepcopy(
                RECOVERY_INNER_VERIFY_BINDING_POLICY_V2
            ),
        }
    )
    contract["analysis"].update(
        {
            "a24_historical_run_policy": (
                "A24 remains immutable six-pair INVALID pilot evidence and is never "
                "rescored, resumed, or pooled with A25"
            ),
            "a25_pilot_denominator_policy": (
                "all six preregistered A17/A21/A23/A24-unseen pilot pairs remain in "
                "the denominator; exclusion, replacement, duplication, and "
                "post-collection selection within A25 are forbidden"
            ),
            "a25_inner_binding_policy": (
                "read-only-only binding is valid solely for a typed pre-mutation "
                "rejection with an empty attempted product delta and retained control"
            ),
            "a25_effect_limitation": (
                "A25 informs exposure design only and cannot authorize an effect or "
                "generalization claim"
            ),
        }
    )
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A25",
            "reason": (
                "A24 exposed an inner-binding validator that required a future "
                "final-success phase from an honest pre-mutation scaffold rejection"
            ),
            "historical_run_policy": (
                "A23 and A24 raw, ledger, contract, and INVALID reports remain unchanged"
            ),
            "selected_pair_policy": (
                "two previously unused source-task clusters per generic, data, and "
                "Next.js profile, one pilot repetition each"
            ),
            "go_rule": (
                "GO requires complete exact-denominator instrumentation, versioned "
                "inner-binding and honest-rejection semantics, and zero adopted safety "
                "harm; the separate exposure threshold selects the next design"
            ),
            "inference_role": (
                "independent forward-only instrument and exposure pilot only"
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
        description="Freeze the A25 forward-only inner-binding Recovery exposure pilot"
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
