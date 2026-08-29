from __future__ import annotations

import argparse
from pathlib import Path
from typing import Any

from eval_lib.generate_goal_verify_main_v4 import _write_json
from eval_lib.generate_goal_verify_recovery_v4_a14 import (
    _build_contract as _build_a14_contract,
)

ROOT = Path(__file__).resolve().parents[2]
CONTRACT_PATH = ROOT / "eval/goal_verify/v0/phase6-recovery-v4-a14-a1-contract.json"
CONTRACT_ID = "phase6-recovery-v4-20260829-a14-a1-live-01"
SMOKE_ID = "phase6-recovery-v4-20260829-a14-a1-smoke-01"


def _build_contract(
    *,
    status: str,
    code_sha: str,
    exact_sha_ci_evidence: str,
    live_collection_authorized: bool,
) -> dict[str, Any]:
    contract = _build_a14_contract(
        status=status,
        code_sha=code_sha,
        exact_sha_ci_evidence=exact_sha_ci_evidence,
        live_collection_authorized=live_collection_authorized,
    )
    contract.update(
        {
            "contract_id": CONTRACT_ID,
            "smoke_run_id": SMOKE_ID,
            "supersedes_contract": "phase6-recovery-v4-20260829-a14-live-01",
            "supersedes_smoke_run": "phase6-recovery-v4-20260829-a14-smoke-01",
            "pre_live_amendments": [
                {
                    "amendment_id": "v4-A14-A1",
                    "reason": (
                        "A14 smoke-01 passed --recovery-plan-auto-runs to "
                        "--plan-run, but automatic Recovery is implemented at the "
                        "--ultra-plan-run boundary; no Recovery attempt executed"
                    ),
                    "historical_run_policy": (
                        "smoke-01 remains immutable diagnostic evidence and is not "
                        "used as Recovery-effect evidence"
                    ),
                }
            ],
        }
    )
    paired = contract["paired_run_contract"]
    paired["execution_action"] = "ultra_plan_run"
    paired["same_execution_action_required"] = True
    paired["estimand"] = (
        "effect of enabling at most one automatic Recovery Plan during the same "
        "product UltraPlan execution boundary"
    )
    smoke = contract["smoke"]
    smoke["minimum_executed_recovery_pairs"] = 1
    smoke["require_recovery_capable_execution_action"] = True
    smoke["effect_claim_allowed"] = False
    contract["analysis"]["smoke_readiness_requires_live_recovery_execution"] = True
    contract["runner_sources"].append(
        "scripts/eval_lib/generate_goal_verify_recovery_v4_a14_a1.py"
    )
    return contract


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate A14-A1 Recovery 0-vs-1 inputs"
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
