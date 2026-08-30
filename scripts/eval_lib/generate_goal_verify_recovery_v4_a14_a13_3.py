from __future__ import annotations

import argparse
from typing import Any

from eval_lib.generate_goal_verify_main_v4 import _write_json
from eval_lib.generate_goal_verify_recovery_v4_a14_a13_2 import ROOT
from eval_lib.generate_goal_verify_recovery_v4_a14_a13_2 import (
    _build_contract as _build_a14_a13_2_contract,
)

EVAL = ROOT / "eval/goal_verify/v0"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a14-a13-3-contract.json"

CONTRACT_ID = "phase6-recovery-v4-20260830-a14-a13-3-live-01"
SMOKE_ID = "phase6-recovery-v4-20260830-a14-a13-3-smoke-01"


def _build_contract(
    *,
    status: str,
    code_sha: str,
    exact_sha_ci_evidence: str,
    live_collection_authorized: bool,
) -> dict[str, Any]:
    contract = _build_a14_a13_2_contract(
        status=status,
        code_sha=code_sha,
        exact_sha_ci_evidence=exact_sha_ci_evidence,
        live_collection_authorized=live_collection_authorized,
    )
    contract.update(
        {
            "schema_version": (
                "commandagent.goal_verify.recovery_experiment.v4_a14_a13_3"
            ),
            "contract_id": CONTRACT_ID,
            "smoke_run_id": SMOKE_ID,
            "supersedes_contract": (
                "phase6-recovery-v4-20260830-a14-a13-2-live-01"
            ),
            "supersedes_smoke_run": (
                "phase6-recovery-v4-20260830-a14-a13-2-smoke-01"
            ),
        }
    )
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A14-A13-3",
            "reason": (
                "A14-A13-2 passed 29 of 30 instrument checks, but one generic "
                "Recovery treatment passed its registered command and was promoted "
                "while the product-visible completion contract still lacked the "
                "verification and acceptance-evidence obligations"
            ),
            "historical_run_policy": (
                "A14-A13-2 smoke-01 remains immutable 10-pair NO-GO evidence and "
                "is never rescored after Recovery handoff or promotion changes"
            ),
            "inference_role": (
                "completion-obligation handoff and promotion-safety diagnostic; "
                "no population effect claim"
            ),
            "instrument_findings": [
                "A14-A13-2 completed all 10 frozen pairs and passed 29 of 30 checks",
                "profile-contract and dependency sentinels executed zero Recoveries",
                "two CLI pairs improved with zero harm and zero regression",
                "one generic pair was external pass before and after Recovery",
                "the generic treatment was promoted with completion_verify_passed false",
            ],
        }
    )
    contract["analysis"].update(
        {
            "completion_obligation_handoff_policy": (
                "bind missing product-visible obligations and their deterministic "
                "repair target paths before generating the Recovery plan"
            ),
            "recovery_promotion_policy": (
                "promote only after the registered final-success observation and "
                "the remaining product-visible completion contract both pass"
            ),
            "registered_observation_scope": (
                "an executed exact registered fix reproducer may satisfy only "
                "bound_verify_command; it cannot waive other missing or weak evidence"
            ),
            "recovery_completion_evidence_policy": (
                "completion requires the post-Recovery contract pass, treatment "
                "promotion, and recovery_succeeded events as a conjunction"
            ),
        }
    )
    contract["authorization"]["approved_at"] = (
        "2026-08-30" if live_collection_authorized else None
    )
    contract["runner_sources"].append(
        "scripts/eval_lib/generate_goal_verify_recovery_v4_a14_a13_3.py"
    )
    contract["runner_sources"] = list(dict.fromkeys(contract["runner_sources"]))
    return contract


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate A14-A13-3 completion-safe Recovery smoke"
    )
    parser.add_argument("--code-sha")
    parser.add_argument("--exact-sha-ci-evidence")
    parser.add_argument("--smoke-collection-authorized", action="store_true")
    args = parser.parse_args()
    if bool(args.code_sha) != bool(args.exact_sha_ci_evidence):
        parser.error("--code-sha and --exact-sha-ci-evidence must be paired")
    if args.smoke_collection_authorized and not args.code_sha:
        parser.error("smoke authorization requires exact-SHA inputs")
    _write_json(
        CONTRACT_PATH,
        _build_contract(
            status="frozen" if args.code_sha else "draft",
            code_sha=args.code_sha or "",
            exact_sha_ci_evidence=args.exact_sha_ci_evidence or "",
            live_collection_authorized=args.smoke_collection_authorized,
        ),
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
