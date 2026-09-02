from __future__ import annotations

import argparse
from typing import Any

from eval_lib.generate_goal_verify_main_v4 import _write_json
from eval_lib.generate_goal_verify_recovery_v4_a14_a2 import ROOT
from eval_lib.generate_goal_verify_recovery_v4_a14_a9 import (
    _build_contract as _build_a14_a9_contract,
)

EVAL = ROOT / "eval/goal_verify/v0"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a14-a10-contract.json"

CONTRACT_ID = "phase6-recovery-v4-20260830-a14-a10-live-01"
SMOKE_ID = "phase6-recovery-v4-20260830-a14-a10-smoke-01"


def _build_contract(
    *,
    status: str,
    code_sha: str,
    exact_sha_ci_evidence: str,
    live_collection_authorized: bool,
) -> dict[str, Any]:
    contract = _build_a14_a9_contract(
        status=status,
        code_sha=code_sha,
        exact_sha_ci_evidence=exact_sha_ci_evidence,
        live_collection_authorized=live_collection_authorized,
    )
    contract.update(
        {
            "schema_version": (
                "commandagent.goal_verify.recovery_experiment.v4_a14_a10"
            ),
            "contract_id": CONTRACT_ID,
            "smoke_run_id": SMOKE_ID,
            "supersedes_contract": (
                "phase6-recovery-v4-20260830-a14-a9-live-01"
            ),
            "supersedes_smoke_run": (
                "phase6-recovery-v4-20260830-a14-a9-smoke-01"
            ),
        }
    )
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A14-A10",
            "reason": (
                "A14-A9 proved top-level Recovery command provenance, but inner "
                "Recovery StepPlans could introduce unregistered stateful shell "
                "fragments, and pytest was not classified as bound test evidence"
            ),
            "historical_run_policy": (
                "A14-A9 smoke-01 remains immutable command-provenance evidence; "
                "its two unchanged-fail treatments are not rescored as A14-A10"
            ),
            "inference_role": (
                "inner Recovery plan-contract and pytest evidence diagnostic; "
                "conditional 0-vs-1 evidence only, with no population effect claim"
            ),
            "instrument_findings": [
                "A14-A9 passed all 27 instrument checks and executed two Recovery treatments",
                "task-05 introduced an unregistered test-dollar-question-mark fragment inside repair-unknown",
                "task-10 repaired the implementation and passed all three registered commands inside treatment",
                "task-10 was rejected because python3 -m pytest was classified as Other and bound_verify_command remained absent",
                "inspection-phase verification could trigger bounded mutation despite the read-only phase contract",
            ],
        }
    )
    contract["smoke"].update(
        {
            "require_registered_inner_recovery_verify_commands": True,
            "inference_role": (
                "inner Recovery command binding, read-only inspection, pytest "
                "evidence classification, and isolated treatment diagnostics"
            ),
        }
    )
    readiness_checks = contract["smoke"]["required_readiness_checks"]
    if "registered_inner_recovery_verify_commands" not in readiness_checks:
        readiness_checks.append("registered_inner_recovery_verify_commands")
    contract["analysis"].update(
        {
            "inner_recovery_verify_command_authority": (
                "CompletionContract.verify_commands via host-owned StepPlan binding"
            ),
            "recovery_inspection_phase_policy": (
                "inspect steps only; no verify command, expected path, or mutation-triggering step"
            ),
            "recovery_repair_phase_policy": (
                "preserve inspect and implementation work, remove model verify steps, "
                "and append one host-owned final-success step with the complete registered set"
            ),
            "pytest_evidence_policy": (
                "classify pytest invocation as Test only when a test artifact exists"
            ),
        }
    )
    contract["authorization"]["approved_at"] = (
        "2026-08-30" if live_collection_authorized else None
    )
    contract["runner_sources"].append(
        "scripts/eval_lib/generate_goal_verify_recovery_v4_a14_a10.py"
    )
    return contract


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate A14-A10 inner Recovery contract binding contract"
    )
    parser.add_argument("--code-sha")
    parser.add_argument("--exact-sha-ci-evidence")
    parser.add_argument("--smoke-collection-authorized", action="store_true")
    args = parser.parse_args()
    if bool(args.code_sha) != bool(args.exact_sha_ci_evidence):
        parser.error("--code-sha and --exact-sha-ci-evidence must be paired")
    if args.smoke_collection_authorized and not args.code_sha:
        parser.error("smoke authorization requires exact-SHA inputs")
    contract = _build_contract(
        status="frozen" if args.code_sha else "draft",
        code_sha=args.code_sha or "",
        exact_sha_ci_evidence=args.exact_sha_ci_evidence or "",
        live_collection_authorized=args.smoke_collection_authorized,
    )
    _write_json(CONTRACT_PATH, contract)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
