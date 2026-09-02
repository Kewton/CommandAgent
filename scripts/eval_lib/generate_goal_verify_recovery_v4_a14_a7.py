from __future__ import annotations

import argparse
from typing import Any

from eval_lib.generate_goal_verify_main_v4 import _write_json
from eval_lib.generate_goal_verify_recovery_v4_a14_a2 import ROOT
from eval_lib.generate_goal_verify_recovery_v4_a14_a6 import (
    _build_contract as _build_a14_a6_1_contract,
)

EVAL = ROOT / "eval/goal_verify/v0"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a14-a7-contract.json"

CONTRACT_ID = "phase6-recovery-v4-20260830-a14-a7-live-01"
SMOKE_ID = "phase6-recovery-v4-20260830-a14-a7-smoke-01"


def _build_contract(
    *,
    status: str,
    code_sha: str,
    exact_sha_ci_evidence: str,
    live_collection_authorized: bool,
) -> dict[str, Any]:
    contract = _build_a14_a6_1_contract(
        status=status,
        code_sha=code_sha,
        exact_sha_ci_evidence=exact_sha_ci_evidence,
        live_collection_authorized=live_collection_authorized,
    )
    contract.update(
        {
            "schema_version": (
                "commandagent.goal_verify.recovery_experiment.v4_a14_a7"
            ),
            "contract_id": CONTRACT_ID,
            "smoke_run_id": SMOKE_ID,
            "supersedes_contract": (
                "phase6-recovery-v4-20260830-a14-a6-1-live-01"
            ),
            "supersedes_smoke_run": (
                "phase6-recovery-v4-20260830-a14-a6-1-smoke-01"
            ),
        }
    )
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A14-A7",
            "reason": (
                "A14-A6.1 bound the exact typed reproducer in 3/3 pairs but "
                "Recovery preflight rejected input_output_contract before running "
                "the registered read-only final-success suite"
            ),
            "historical_run_policy": (
                "A14-A6.1 smoke-01 remains immutable typed-binding evidence; its "
                "three executed-zero records are not rescored as A14-A7"
            ),
            "inference_role": (
                "Recovery preflight integration and conditional 0-vs-1 effect "
                "diagnostic; no population effect claim"
            ),
            "instrument_findings": [
                "all three typed fix reproducers were bound and executed before failure",
                "all three pairs stopped before Recovery with input_output_contract unavailable",
                "the product used a blanket non-empty required-capability rejection",
                "A14-A7 admits input_output_contract only when the typed reproducer exactly matches a registered verify command",
                "unbound, mismatched, browser, and other unsupported capabilities remain unavailable",
            ],
        }
    )
    contract["smoke"].update(
        {
            "minimum_executed_recovery_pairs": 1,
            "require_executed_recovery_for_attribution": True,
            "inference_role": (
                "typed input-output preflight integration; conditional Recovery "
                "0-vs-1 evidence only for naturally executed Recovery pairs"
            ),
        }
    )
    contract["analysis"].update(
        {
            "smoke_readiness_requires_live_recovery_execution": True,
            "input_output_contract_observation_binding": (
                "normalized fix_reproducer_command must equal one registered "
                "verify_commands entry"
            ),
            "unsupported_required_capability_policy": "fail_closed",
        }
    )
    contract["authorization"]["approved_at"] = (
        "2026-08-30" if live_collection_authorized else None
    )
    contract["runner_sources"].append(
        "scripts/eval_lib/generate_goal_verify_recovery_v4_a14_a7.py"
    )
    return contract


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate A14-A7 typed Recovery preflight contract"
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
