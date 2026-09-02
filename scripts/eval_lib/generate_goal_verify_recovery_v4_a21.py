from __future__ import annotations

import argparse
import copy
from typing import Any

from eval_lib.generate_goal_verify_main_v4 import _load, _write_json
from eval_lib.generate_goal_verify_recovery_v4_a14_a2 import ROOT, _file_sha256
from eval_lib.generate_goal_verify_recovery_v4_a15_a1 import (
    _validate_exact_sha_evidence,
)

EVAL = ROOT / "eval/goal_verify/v0"
BASE_CONTRACT_PATH = EVAL / "phase6-recovery-v4-a17-smoke-contract.json"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a21-smoke-contract.json"
CONTRACT_ID = "phase6-recovery-v4-20260902-a21-smoke-01"

SELECTED_CASE_IDS = [
    "phase6-main-c07-task-02",
    "phase6-main-c07-task-06",
    "phase6-main-c07-task-09",
    "phase6-main-c13-task-02",
    "phase6-main-c13-task-06",
    "phase6-main-c13-task-09",
    "phase6-main-c14-task-02",
    "phase6-main-c14-task-06",
    "phase6-main-c14-task-08",
]
CASE_PROFILES = {
    **{case_id: "generic" for case_id in SELECTED_CASE_IDS[:3]},
    **{case_id: "data" for case_id in SELECTED_CASE_IDS[3:6]},
    **{case_id: "nextjs" for case_id in SELECTED_CASE_IDS[6:]},
}
CASE_REPRODUCERS = {
    "phase6-main-c07-task-02": "python3 app.py fixture/task-02.json",
    "phase6-main-c07-task-06": "python3 app.py fixture/task-06.json",
    "phase6-main-c07-task-09": "python3 app.py fixture/task-09.json",
    "phase6-main-c13-task-02": "python3 scripts/repro.py data/task-02.csv",
    "phase6-main-c13-task-06": "python3 scripts/repro.py data/task-06.csv",
    "phase6-main-c13-task-09": "python3 scripts/repro.py data/task-09.csv",
    "phase6-main-c14-task-02": "node scripts/repro.mjs fixture/task-02.json",
    "phase6-main-c14-task-06": "node scripts/repro.mjs fixture/task-06.json",
    "phase6-main-c14-task-08": "node scripts/repro.mjs fixture/task-08.json",
}
SELECTED_PAIR_IDS = [
    f"{case_id}--pair-{sample:02d}"
    for case_id in SELECTED_CASE_IDS
    for sample in range(1, 4)
]
TYPED_FIX_REPRODUCER_COMMANDS = {
    f"{case_id}--pair-{sample:02d}": CASE_REPRODUCERS[case_id]
    for case_id in SELECTED_CASE_IDS
    for sample in range(1, 4)
}

EXPOSURE_SCHEMA = "commandagent.goal_verify.recovery_exposure_corpus_pilot.v2"
TASK_REGISTRY_SHA256 = (
    "0e75a63bcf2b9f93dcb7564af62bdbdb7114b0a7ab1bb568d5e0eabb15ec5b1e"
)
WORKSPACE_REGISTRY_SHA256 = (
    "a85571e96c4c59ce6c2d4e2439dbf31676faab8f0a0abf5cb67236edccac82cf"
)
PROVISIONING_SHA256 = "f71db0db8aaeffefd48589f568ce99f08ef8236112fd7325bb8cb7b3ff70a729"
EXPECTED_EXPOSURE_CHECKS = {
    "all_cases_ready",
    "all_preselected_case_ids_present_once",
    "every_case_candidate_visible_before_failure",
    "every_case_reference_passes_same_reproducer",
    "every_case_regressions_and_immutability_pass",
    "every_nextjs_route_polarity_is_distinct",
    "exactly_three_cases_per_target_profile",
}
READINESS_CHECKS = (
    "preselected_pair_denominator_exact",
    "minimum_executed_recovery_clusters_in_every_real_profile",
    "attributed_harm_zero",
    "regression_introduced_zero",
    "existing_artifact_harm_zero",
    "instrumentation_unusable_zero",
)


def _validate_exposure_report(report: dict[str, Any]) -> None:
    expected_scalars = {
        "schema_version": EXPOSURE_SCHEMA,
        "inference_role": "candidate_visible_failure_corpus_qualification_only",
        "effect_claim_allowed": False,
        "full_effect_execution_authorized": False,
        "corpus_ready_for_preregistration": True,
        "go_no_go": "GO",
        "case_count": len(SELECTED_CASE_IDS),
        "task_registry_sha256": TASK_REGISTRY_SHA256,
        "workspace_registry_sha256": WORKSPACE_REGISTRY_SHA256,
        "provisioning_sha256": PROVISIONING_SHA256,
    }
    if any(report.get(key) != value for key, value in expected_scalars.items()):
        raise ValueError("unexpected A20 exposure report identity or result")
    if report.get("case_ids") != sorted(SELECTED_CASE_IDS):
        raise ValueError("A20 exposure report case population mismatch")
    if report.get("profile_case_counts") != {
        "generic": 3,
        "data": 3,
        "nextjs": 3,
    }:
        raise ValueError("A20 exposure report profile counts mismatch")
    selection = report.get("selection_policy")
    if selection != {
        "all_preselected_cases_remain_in_denominator": True,
        "runtime_case_exclusion_allowed": False,
    }:
        raise ValueError("A20 exposure selection policy mismatch")
    checks = report.get("checks")
    if (
        not isinstance(checks, dict)
        or set(checks) != EXPECTED_EXPOSURE_CHECKS
        or not all(value is True for value in checks.values())
    ):
        raise ValueError("A20 exposure checks are incomplete")
    evidence = report.get("evidence_sha256")
    if (
        not isinstance(evidence, dict)
        or set(evidence) != {"case-evidence.json"}
        or not isinstance(evidence["case-evidence.json"], str)
        or len(evidence["case-evidence.json"]) != 64
    ):
        raise ValueError("A20 case evidence hash is invalid")


def _validate_exposure_bindings(
    bindings: list[tuple[str, dict[str, Any], str]],
) -> None:
    if len(bindings) != 2 or len({path for path, _, _ in bindings}) != 2:
        raise ValueError("A21 requires two distinct A20 exposure reports")
    semantic_reports = []
    for path, report, report_sha256 in bindings:
        evidence_path = (ROOT / path).resolve()
        try:
            evidence_path.relative_to(ROOT.resolve())
        except ValueError as error:
            raise ValueError("A20 report must be inside the repository") from error
        if not isinstance(report_sha256, str) or len(report_sha256) != 64:
            raise ValueError("A20 report sha256 is invalid")
        _validate_exposure_report(report)
        semantic_reports.append(
            {key: value for key, value in report.items() if key != "evidence_sha256"}
        )
    if semantic_reports[0] != semantic_reports[1]:
        raise ValueError("A20 repeated exposure reports disagree semantically")


def build_contract(
    *,
    code_sha: str,
    exact_sha_ci_evidence: str,
    exposure_bindings: list[tuple[str, dict[str, Any], str]],
    authorized: bool,
) -> dict[str, Any]:
    base = _load(BASE_CONTRACT_PATH)
    if base.get("contract_id") != "phase6-recovery-v4-20260902-a17-smoke-01":
        raise ValueError("unexpected A17 base contract")
    if base.get("status") != "frozen" or "full_experiment" in base:
        raise ValueError("A21 must inherit the frozen A17 smoke contract")

    evidence_path = (ROOT / exact_sha_ci_evidence).resolve()
    try:
        evidence_path.relative_to(ROOT.resolve())
    except ValueError as error:
        raise ValueError("exact-SHA evidence must be inside the repository") from error
    _validate_exact_sha_evidence(code_sha=code_sha, evidence_path=evidence_path)
    _validate_exposure_bindings(exposure_bindings)

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
            "candidate_exposure_evidence": {
                "schema_version": EXPOSURE_SCHEMA,
                "case_count": len(SELECTED_CASE_IDS),
                "profile_case_counts": {"generic": 3, "data": 3, "nextjs": 3},
                "repeated_semantic_go": True,
                "effect_claim_allowed": False,
                "reports": [
                    {
                        "path": path,
                        "sha256": report_sha256,
                        "case_evidence_sha256": report["evidence_sha256"][
                            "case-evidence.json"
                        ],
                    }
                    for path, report, report_sha256 in exposure_bindings
                ],
            },
        }
    )
    smoke = contract["smoke"]
    smoke.update(
        {
            "effect_claim_allowed": False,
            "expected_pair_count": len(SELECTED_PAIR_IDS),
            "inference_role": (
                "candidate-visible natural Recovery exposure qualification; "
                "instrument diagnostic only"
            ),
            "minimum_executed_recovery_pairs": 12,
            "minimum_executed_recovery_pairs_per_real_profile": 3,
            "minimum_executed_recovery_clusters_per_real_profile": 2,
            "minimum_pairs_per_real_profile": 9,
            "required_real_profiles": ["generic", "data", "nextjs"],
            "selected_pair_ids": SELECTED_PAIR_IDS,
            "typed_fix_reproducer_commands": TYPED_FIX_REPRODUCER_COMMANDS,
            "includes_dependency_exclusion_sentinel": False,
            "require_preselected_pair_denominator_exact": True,
            "require_recovery_safety_zero": True,
        }
    )
    smoke.pop("real_profile_path_coverage_policy", None)
    checks = smoke.setdefault("required_readiness_checks", [])
    for check in READINESS_CHECKS:
        if check not in checks:
            checks.append(check)

    contract["analysis"].update(
        {
            "a17_historical_run_policy": (
                "A17 remains immutable nine-pair NO-GO evidence and is never resumed, "
                "rescored, or pooled with A21"
            ),
            "a20_candidate_exposure_policy": (
                "two independent model-free runs qualified the same nine frozen cases; "
                "each registered before reproducer failed, each reference repair and "
                "regression set passed, immutable inputs matched, and every Next.js "
                "route observation had distinct before/reference polarity"
            ),
            "a21_denominator_policy": (
                "all 27 preregistered pairs remain in the denominator; runtime case "
                "exclusion, pair substitution, duplication, and post-collection selection "
                "are forbidden"
            ),
            "a21_exposure_gate": (
                "require at least 12 executed Recoveries overall, at least three per "
                "profile, and at least two source-task clusters per profile"
            ),
            "a21_effect_limitation": (
                "A21 qualifies natural model failure exposure and instrument usability; "
                "it does not authorize a Recovery effect or cross-family generalization claim"
            ),
            "a21_authoritative_report_command": (
                "scripts/eval-goal-verify-recovery-a15-report.py; the generic v4 report "
                "does not evaluate the preregistered per-profile pair and task-cluster gates"
            ),
        }
    )
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A21",
            "reason": (
                "A17 executed zero Recoveries because selected generic and Next.js tasks "
                "resolved initially, while the selected data failure did not expose a "
                "typed candidate; A20 independently qualified nine candidate-visible "
                "before/reference failure pairs"
            ),
            "historical_run_policy": (
                "A17 and both A20 qualification runs remain immutable and are not pooled "
                "with A21 outcomes"
            ),
            "selected_pair_policy": (
                "three frozen cases per generic, data, and Next.js profile with three "
                "replications each; all 27 pair IDs are fixed before collection"
            ),
            "go_rule": (
                "all instrument checks, exact 27-pair denominator, at least 12 executed "
                "Recoveries overall, at least three and two task clusters per profile, "
                "and zero discarded-valid treatment, harm, regression, or unusable record"
            ),
            "inference_role": (
                "natural Recovery exposure qualification only; no effect or "
                "generalization claim"
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
        description="Freeze the A21 candidate-visible Recovery exposure smoke"
    )
    parser.add_argument("--code-sha", required=True)
    parser.add_argument("--exact-sha-ci-evidence", required=True)
    parser.add_argument("--exposure-report", action="append", required=True)
    parser.add_argument("--smoke-collection-authorized", action="store_true")
    args = parser.parse_args()
    exposure_bindings = []
    for relative in args.exposure_report:
        report_path = (ROOT / relative).resolve()
        exposure_bindings.append(
            (relative, _load(report_path), _file_sha256(report_path))
        )
    contract = build_contract(
        code_sha=args.code_sha,
        exact_sha_ci_evidence=args.exact_sha_ci_evidence,
        exposure_bindings=exposure_bindings,
        authorized=args.smoke_collection_authorized,
    )
    _write_json(CONTRACT_PATH, contract)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
