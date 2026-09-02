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

EVAL = ROOT / "eval/goal_verify/v0"
BASE_CONTRACT_PATH = EVAL / "phase6-recovery-v4-a21-smoke-contract.json"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a23-pilot-contract.json"
CONTRACT_ID = "phase6-recovery-v4-20260902-a23-pilot-01"
MODEL = "qwen3.5:9b"
MODEL_DIGEST = "6488c96fa5faab64bb65cbd30d4289e20e6130ef535a93ef9a49f42eda893ea7"

SELECTED_CASE_IDS = [
    "phase6-main-c07-task-03",
    "phase6-main-c07-task-07",
    "phase6-main-c13-task-03",
    "phase6-main-c13-task-07",
    "phase6-main-c14-task-03",
    "phase6-main-c14-task-07",
]
CASE_PROFILES = {
    **{case_id: "generic" for case_id in SELECTED_CASE_IDS[:2]},
    **{case_id: "data" for case_id in SELECTED_CASE_IDS[2:4]},
    **{case_id: "nextjs" for case_id in SELECTED_CASE_IDS[4:]},
}
CASE_REPRODUCERS = {
    "phase6-main-c07-task-03": "python3 app.py fixture/task-03.json",
    "phase6-main-c07-task-07": "python3 app.py fixture/task-07.json",
    "phase6-main-c13-task-03": "python3 scripts/repro.py data/task-03.csv",
    "phase6-main-c13-task-07": "python3 scripts/repro.py data/task-07.csv",
    "phase6-main-c14-task-03": "node scripts/repro.mjs fixture/task-03.json",
    "phase6-main-c14-task-07": "node scripts/repro.mjs fixture/task-07.json",
}
SELECTED_PAIR_IDS = [f"{case_id}--pair-01" for case_id in SELECTED_CASE_IDS]
TYPED_FIX_REPRODUCER_COMMANDS = {
    f"{case_id}--pair-01": CASE_REPRODUCERS[case_id] for case_id in SELECTED_CASE_IDS
}
AUTHORITATIVE_REPORT_SOURCES = [
    "scripts/eval-goal-verify-recovery-a23-report.py",
    "scripts/eval_lib/goal_verify_recovery_a23_report.py",
]


def build_contract(
    *,
    code_sha: str,
    exact_sha_ci_evidence: str,
    authorized: bool,
) -> dict[str, Any]:
    base = _load(BASE_CONTRACT_PATH)
    if base.get("contract_id") != "phase6-recovery-v4-20260902-a21-smoke-01":
        raise ValueError("unexpected A21 base contract")
    if base.get("status") != "frozen" or "full_experiment" in base:
        raise ValueError("A23 must inherit the frozen A21 smoke contract")

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
            "pilot_design": {
                "schema_version": (
                    "commandagent.goal_verify.recovery_natural_exposure_pilot.v1"
                ),
                "inference_role": "exploratory_model_and_exposure_design_only",
                "effect_claim_allowed": False,
                "full_effect_execution_authorized": False,
                "selection_basis": (
                    "two A20/A21-unseen task clusters per profile, selected before "
                    "the first A23 model invocation"
                ),
                "model_selection_basis": (
                    "locally available tool-capable smaller model selected to measure "
                    "whether ceiling-limited 27B tasks expose automatic Recovery"
                ),
                "allowed_uses": [
                    "estimate initial success and executed Recovery counts",
                    "select between natural-exposure and deterministic-fault designs",
                    "set a new confirmatory denominator before collection",
                ],
                "forbidden_uses": [
                    "Recovery effect estimate",
                    "cross-profile generalization claim",
                    "pooling with A21 or a later confirmatory run",
                    "post-collection pair replacement within A23",
                ],
                "natural_exposure_confirmation_threshold": {
                    "minimum_executed_recovery_clusters_per_profile": 1,
                    "minimum_profiles_meeting_threshold": 3,
                    "maximum_instrumentation_unusable_pairs": 0,
                    "maximum_safety_violations": 0,
                    "safety_check_names": [
                        "attributed_harm_zero",
                        "regression_introduced_zero",
                        "existing_artifact_harm_zero",
                        "discarded_valid_treatment_zero",
                        "transaction_control_retention",
                        "isolated_recovery_treatment",
                        "recovery_fix_safety_verification",
                    ],
                },
                "authoritative_report_command": (
                    "scripts/eval-goal-verify-recovery-a23-report.py "
                    "--contract eval/goal_verify/v0/"
                    "phase6-recovery-v4-a23-pilot-contract.json --run-dir "
                    "dev-reports/issue-399/runs/"
                    "phase6-recovery-v4-20260902-a23-pilot-01"
                ),
                "authoritative_report_source_sha256": {
                    relative: hashlib.sha256((ROOT / relative).read_bytes()).hexdigest()
                    for relative in AUTHORITATIVE_REPORT_SOURCES
                },
                "report_exit_semantics": (
                    "zero iff pilot instrumentation is valid; a valid pilot whose "
                    "natural exposure threshold is not met exits zero and freezes the "
                    "deterministic fault-boundary next-design decision"
                ),
                "fallback_when_threshold_not_met": (
                    "preregister a deterministic fault-boundary conditional Recovery "
                    "experiment instead of enlarging this natural-exposure sample"
                ),
                "invalid_pilot_policy": (
                    "diagnose instrument failure without pair replacement, denominator "
                    "resizing, effect estimation, or next-design selection"
                ),
            },
        }
    )
    contract.pop("candidate_exposure_evidence", None)
    smoke = contract["smoke"]
    smoke.update(
        {
            "effect_claim_allowed": False,
            "expected_pair_count": len(SELECTED_PAIR_IDS),
            "inference_role": (
                "exploratory natural-model Recovery exposure pilot; design input only"
            ),
            "minimum_executed_recovery_pairs": 0,
            "minimum_executed_recovery_pairs_per_real_profile": 0,
            "minimum_executed_recovery_clusters_per_real_profile": 0,
            "minimum_pairs_per_real_profile": 2,
            "required_real_profiles": ["generic", "data", "nextjs"],
            "selected_pair_ids": SELECTED_PAIR_IDS,
            "typed_fix_reproducer_commands": TYPED_FIX_REPRODUCER_COMMANDS,
            "includes_dependency_exclusion_sentinel": False,
            "require_preselected_pair_denominator_exact": True,
            "require_recovery_safety_zero": True,
        }
    )
    smoke.pop("real_profile_path_coverage_policy", None)
    smoke["required_readiness_checks"] = [
        check
        for check in smoke.get("required_readiness_checks", [])
        if check != "minimum_executed_recovery_clusters_in_every_real_profile"
    ]

    contract["analysis"].update(
        {
            "a21_historical_run_policy": (
                "A21 remains immutable 27-pair natural-exposure NO-GO evidence and is "
                "never resumed, rescored, or pooled with A23"
            ),
            "a22_regression_role": (
                "A22 proves the corrected generic fix transaction path only and is not "
                "a natural-model effect or exposure sample"
            ),
            "a23_pilot_denominator_policy": (
                "all six preregistered pilot pairs remain in the denominator; runtime "
                "exclusion, replacement, duplication, and post-collection selection "
                "within A23 are forbidden"
            ),
            "a23_model_policy": (
                "qwen3.5:9b digest-pinned exploratory pilot; any later confirmatory run "
                "must separately freeze its model, cases, repetitions, and thresholds"
            ),
            "a23_effect_limitation": (
                "A23 informs exposure design only and cannot authorize an effect or "
                "generalization claim"
            ),
        }
    )
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A23",
            "reason": (
                "A21 showed a 27B-model ceiling effect: generic and Next.js usually "
                "resolved before an automatic Recovery boundary"
            ),
            "historical_run_policy": (
                "A21 and A22 remain immutable and are not pooled with A23"
            ),
            "selected_pair_policy": (
                "two previously unused source-task clusters per generic, data, and "
                "Next.js profile, one pilot repetition each"
            ),
            "go_rule": (
                "GO means complete, safe, exact-denominator instrumentation only; the "
                "separate pilot threshold decides the next design"
            ),
            "inference_role": "exploratory exposure and model selection only",
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
        description="Freeze the A23 exploratory natural Recovery exposure pilot"
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
