from __future__ import annotations

import argparse
from typing import Any

from eval_lib.generate_goal_verify_main_v4 import _write_json
from eval_lib.generate_goal_verify_recovery_v4_a14_a2 import ROOT
from eval_lib.generate_goal_verify_recovery_v4_a14_a11 import (
    _build_contract as _build_a14_a11_contract,
)

EVAL = ROOT / "eval/goal_verify/v0"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a14-a12-contract.json"

CONTRACT_ID = "phase6-recovery-v4-20260830-a14-a12-live-01"
SMOKE_ID = "phase6-recovery-v4-20260830-a14-a12-smoke-01"


def _build_contract(
    *,
    status: str,
    code_sha: str,
    exact_sha_ci_evidence: str,
    live_collection_authorized: bool,
) -> dict[str, Any]:
    contract = _build_a14_a11_contract(
        status=status,
        code_sha=code_sha,
        exact_sha_ci_evidence=exact_sha_ci_evidence,
        live_collection_authorized=live_collection_authorized,
    )
    contract.update(
        {
            "schema_version": (
                "commandagent.goal_verify.recovery_experiment.v4_a14_a12"
            ),
            "contract_id": CONTRACT_ID,
            "smoke_run_id": SMOKE_ID,
            "supersedes_contract": (
                "phase6-recovery-v4-20260830-a14-a11-live-01"
            ),
            "supersedes_smoke_run": (
                "phase6-recovery-v4-20260830-a14-a11-smoke-01"
            ),
        }
    )
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A14-A12",
            "reason": (
                "A14-A11 resumed the exact fix contract and passed registered "
                "completion verification, but reported IntentFinalized from the "
                "standard Recovery FinalAcceptance state and rejected treatment"
            ),
            "historical_run_policy": (
                "A14-A11 smoke-01 remains immutable 29-of-29 instrument evidence; "
                "task-10 is not rescored after phase-transition repair"
            ),
            "inference_role": (
                "resumed-fix terminal-transition and transaction-promotion diagnostic; "
                "conditional 0-vs-1 evidence only, with no population effect claim"
            ),
            "instrument_findings": [
                "task-05 suppressed Recovery after frozen external success",
                "task-10 emitted one valid fix-contract continuation",
                "task-10 registered completion verification passed",
                "task-10 treatment was rejected by an invalid host phase transition",
                "task-10 external fail observed the retained control artifact",
            ],
        }
    )
    contract["smoke"].update(
        {
            "require_recovery_fix_terminal_completion": True,
            "inference_role": (
                "fix-contract continuity, state-aware terminal completion, "
                "transaction promotion, and external outcome diagnostics"
            ),
        }
    )
    readiness_checks = contract["smoke"]["required_readiness_checks"]
    if "recovery_fix_terminal_completion" not in readiness_checks:
        readiness_checks.append("recovery_fix_terminal_completion")
    contract["analysis"].update(
        {
            "recovery_fix_phase_completion_policy": (
                "successful resumed fix runtime emits AcceptancePassed from the "
                "standard Recovery FinalAcceptance state; ordinary fix and "
                "investigation runtimes retain IntentFinalized"
            ),
            "recovery_fix_promotion_gate": (
                "require process success, completion verification pass, successful "
                "Recovery attempt telemetry, and exactly one promoted transaction"
            ),
        }
    )
    contract["authorization"]["approved_at"] = (
        "2026-08-30" if live_collection_authorized else None
    )
    contract["runner_sources"].append(
        "scripts/eval_lib/generate_goal_verify_recovery_v4_a14_a12.py"
    )
    return contract


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate A14-A12 Recovery terminal-transition contract"
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
