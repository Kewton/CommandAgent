from __future__ import annotations

import argparse
from typing import Any

from eval_lib.generate_goal_verify_main_v4 import _write_json
from eval_lib.generate_goal_verify_recovery_v4_a14_a2 import ROOT
from eval_lib.generate_goal_verify_recovery_v4_a14_a7 import (
    _build_contract as _build_a14_a7_contract,
)

EVAL = ROOT / "eval/goal_verify/v0"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a14-a8-contract.json"

CONTRACT_ID = "phase6-recovery-v4-20260830-a14-a8-live-01"
SMOKE_ID = "phase6-recovery-v4-20260830-a14-a8-smoke-01"


def _build_contract(
    *,
    status: str,
    code_sha: str,
    exact_sha_ci_evidence: str,
    live_collection_authorized: bool,
) -> dict[str, Any]:
    contract = _build_a14_a7_contract(
        status=status,
        code_sha=code_sha,
        exact_sha_ci_evidence=exact_sha_ci_evidence,
        live_collection_authorized=live_collection_authorized,
    )
    contract.update(
        {
            "schema_version": (
                "commandagent.goal_verify.recovery_experiment.v4_a14_a8"
            ),
            "contract_id": CONTRACT_ID,
            "smoke_run_id": SMOKE_ID,
            "supersedes_contract": (
                "phase6-recovery-v4-20260830-a14-a7-live-01"
            ),
            "supersedes_smoke_run": (
                "phase6-recovery-v4-20260830-a14-a7-smoke-01"
            ),
        }
    )
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A14-A8",
            "reason": (
                "A14-A7 executed one Recovery in two pairs but both isolated "
                "treatments retained the control workspace completion-contract path"
            ),
            "historical_run_policy": (
                "A14-A7 smoke-01 remains immutable preflight-integration evidence; "
                "its two unchanged-fail treatments are not rescored as A14-A8"
            ),
            "inference_role": (
                "isolated treatment contract binding and conditional 0-vs-1 "
                "effect diagnostic; no population effect claim"
            ),
            "instrument_findings": [
                "A14-A7 naturally executed Recovery in two of three pairs",
                "both treatment executions failed on an outside-workspace contract path",
                "both controls were retained with zero promoted path changes",
                "A14-A8 copies exact validated contract bytes into a treatment-owned runtime path",
                "copy or validation failure remains a fail-closed treatment preparation stop",
            ],
        }
    )
    contract["smoke"]["inference_role"] = (
        "isolated treatment completion-contract binding; conditional Recovery "
        "0-vs-1 evidence only for naturally executed Recovery pairs"
    )
    contract["analysis"].update(
        {
            "treatment_completion_contract_binding": (
                "exact bytes copied by host to "
                ".commandagent/recovery-runtime/completion-contract.json"
            ),
            "treatment_contract_source_in_promotion_manifest": False,
            "treatment_contract_copy_failure_policy": "fail_closed_before_execution",
        }
    )
    contract["authorization"]["approved_at"] = (
        "2026-08-30" if live_collection_authorized else None
    )
    contract["runner_sources"].append(
        "scripts/eval_lib/generate_goal_verify_recovery_v4_a14_a8.py"
    )
    return contract


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate A14-A8 isolated Recovery contract-binding contract"
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
