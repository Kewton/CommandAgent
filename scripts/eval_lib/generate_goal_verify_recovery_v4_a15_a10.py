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
BASE_SMOKE_PATH = EVAL / "phase6-recovery-v4-a15-a9-smoke-contract.json"
BASE_FULL_PATH = EVAL / "phase6-recovery-v4-a15-full-contract.json"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a15-a10-full-contract.json"
CONTRACT_ID = "phase6-recovery-v4-20260901-a15-a10-live-01"

SMOKE_CONTRACT_ID = "phase6-recovery-v4-20260901-a15-a9-smoke-01"
SMOKE_RECORD_COUNT = 14
RESOURCE_BUDGETS = {
    "total_tokens": {"p50": 60_000, "p95": 70_000},
    "wall_time_ms": {"p50": 210_000, "p95": 210_000},
}
RESOURCE_BUDGET_BASIS = (
    "fixed before full collection from A15-A9 executed-Recovery profile maxima: "
    "total-token p50 47,882 and p95 48,068.3; wall-time p50 145,470 ms and "
    "p95 162,478.2 ms; apply 25% headroom and round upward to 10,000 tokens "
    "or 30,000 ms; use the resulting conservative budgets for every profile"
)


def _validate_smoke_report(report: dict[str, Any]) -> None:
    if report.get("contract_id") != SMOKE_CONTRACT_ID:
        raise ValueError("A15-A10 requires the frozen A15-A9 smoke report")
    if report.get("record_count") != SMOKE_RECORD_COUNT:
        raise ValueError("A15-A9 smoke report must contain all 14 records")
    if report.get("instrument_ready") is not True or report.get("go_no_go") != "GO":
        raise ValueError("A15-A9 instrument smoke must be GO")
    if report.get("effect_claim_allowed") is not False:
        raise ValueError("A15-A9 smoke must remain instrument-only evidence")
    checks = report.get("checks")
    if (
        not isinstance(checks, dict)
        or not checks
        or not all(value is True for value in checks.values())
    ):
        raise ValueError("every A15-A9 instrument check must be true")


def build_contract(
    *,
    code_sha: str,
    exact_sha_ci_evidence: str,
    smoke_report: dict[str, Any],
    smoke_report_path: str,
    smoke_report_sha256: str,
    authorized: bool,
) -> dict[str, Any]:
    smoke = _load(BASE_SMOKE_PATH)
    old_full = _load(BASE_FULL_PATH)
    if smoke.get("contract_id") != SMOKE_CONTRACT_ID:
        raise ValueError("unexpected A15-A9 smoke contract")
    if smoke.get("status") != "frozen" or "full_experiment" in smoke:
        raise ValueError("A15-A10 must inherit the frozen A15-A9 smoke contract")
    _validate_smoke_report(smoke_report)

    evidence_path = (ROOT / exact_sha_ci_evidence).resolve()
    report_path = (ROOT / smoke_report_path).resolve()
    for path, label in (
        (evidence_path, "exact-SHA evidence"),
        (report_path, "A15-A9 report"),
    ):
        try:
            path.relative_to(ROOT.resolve())
        except ValueError as error:
            raise ValueError(f"{label} must be inside the repository") from error
    _validate_exact_sha_evidence(code_sha=code_sha, evidence_path=evidence_path)
    if _file_sha256(report_path) != smoke_report_sha256:
        raise ValueError("A15-A9 report sha256 does not match the frozen evidence")

    contract = copy.deepcopy(smoke)
    contract.update(
        {
            "contract_id": CONTRACT_ID,
            "smoke_run_id": CONTRACT_ID,
            "code_sha": code_sha,
            "exact_sha_ci_evidence": exact_sha_ci_evidence,
            "status": "frozen",
            "supersedes_contract": smoke["contract_id"],
            "instrument_smoke_evidence": {
                "contract_id": SMOKE_CONTRACT_ID,
                "report_path": smoke_report_path,
                "report_sha256": smoke_report_sha256,
                "record_count": SMOKE_RECORD_COUNT,
                "instrument_checks": len(smoke_report["checks"]),
                "instrument_ready": True,
                "go_no_go": "GO",
            },
        }
    )
    full_smoke = copy.deepcopy(old_full["smoke"])
    for key, value in smoke["smoke"].items():
        if key.startswith("require_recovery_") or key in {
            "recovery_fix_terminal_outcome_policy",
            "real_profile_path_coverage_policy",
        }:
            full_smoke[key] = copy.deepcopy(value)
    full_smoke["required_readiness_checks"] = copy.deepcopy(
        smoke["smoke"]["required_readiness_checks"]
    )
    contract["smoke"] = full_smoke
    design = copy.deepcopy(old_full["full_experiment"])
    design.update(
        {
            "resource_budgets": copy.deepcopy(RESOURCE_BUDGETS),
            "resource_budget_basis": RESOURCE_BUDGET_BASIS,
            "full_freeze_prerequisites": (
                "A15-A9 instrument smoke GO with every check true, exact-SHA CI, "
                "and four wall/token budgets fixed without inspecting A15-A10 outcomes"
            ),
            "go_rule": (
                "all instrument gates; each of cli, generic, data, and nextjs has "
                "at least five executed Recoveries and a 2,000-sample profile-specific "
                "95% CI lower bound above zero; pooled CI lower above zero; zero harm, "
                "regression, instrumentation-unusable, and sentinel Recovery; all "
                "profile resource budgets met"
            ),
            "claim_scope": (
                "the preregistered offline-executable fix-task families in this corpus; "
                "not every task or defect that can select the same product profile"
            ),
            "task_family_limitation": (
                "within-profile variants can share a defect template; source_task_id is "
                "the frozen bootstrap cluster, but cross-family generalization requires "
                "a later corpus with additional independent defect families"
            ),
        }
    )
    contract["full_experiment"] = design
    contract["analysis"].update(
        {
            "full_effect_claim_scope": design["claim_scope"],
            "full_effect_task_family_limitation": design["task_family_limitation"],
            "a15_a9_historical_run_policy": (
                "A15-A9 remains immutable 14-pair instrument-GO evidence; it is not "
                "pooled into the A15-A10 effect estimate or rescored"
            ),
            "full_resource_budget_policy": RESOURCE_BUDGET_BASIS,
        }
    )
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A15-A10",
            "reason": (
                "A15-A9 passed every instrument check after typed handoff, referenced-API "
                "preservation, one bounded local repair, byte-based mutation observation, "
                "and rejected-treatment delta recording"
            ),
            "historical_run_policy": (
                "A15-A9 smoke-01 remains immutable instrument evidence and is not changed, "
                "resumed, rescored, or included in the full estimate"
            ),
            "frozen_design_policy": (
                "reuse the already preregistered 120 eligible pairs and 20 sentinels; "
                "preserve task registry, model, prompts, source workspaces, external "
                "oracles, Recovery 0-vs-1 arms, maximum one Recovery, exclusions, and "
                "2,000-sample task-cluster bootstrap"
            ),
            "resource_budget_policy": RESOURCE_BUDGET_BASIS,
            "inference_role": (
                "profile-specific and pooled effect estimate for the frozen task families; "
                "no claim about unrepresented defect families"
            ),
            "instrument_findings": [
                "A15-A9 completed 14 of 14 records and all 35 instrument checks were true",
                "three CLI pairs improved after exactly one Recovery",
                "three data pairs remained failed without harm or regression",
                "generic and Next.js selected pairs passed before Recovery and were not mutated",
                "dependency and profile-contract sentinels executed zero Recoveries",
            ],
        }
    )
    contract["authorization"].update(
        {
            "full_collection_authorized": authorized,
            "approved_by": "repository owner" if authorized else None,
            "approved_at": "2026-09-01" if authorized else None,
        }
    )
    return contract


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Freeze the A15-A10 cross-profile Recovery effect experiment"
    )
    parser.add_argument("--code-sha", required=True)
    parser.add_argument("--exact-sha-ci-evidence", required=True)
    parser.add_argument("--a15-a9-report", required=True)
    parser.add_argument("--full-collection-authorized", action="store_true")
    args = parser.parse_args()
    report_path = (ROOT / args.a15_a9_report).resolve()
    report = _load(report_path)
    contract = build_contract(
        code_sha=args.code_sha,
        exact_sha_ci_evidence=args.exact_sha_ci_evidence,
        smoke_report=report,
        smoke_report_path=args.a15_a9_report,
        smoke_report_sha256=_file_sha256(report_path),
        authorized=args.full_collection_authorized,
    )
    _write_json(CONTRACT_PATH, contract)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
