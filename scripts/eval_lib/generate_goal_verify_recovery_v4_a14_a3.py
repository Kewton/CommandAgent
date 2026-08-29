from __future__ import annotations

import argparse
from typing import Any

from eval_lib.generate_goal_verify_main_v4 import _write_json
from eval_lib.generate_goal_verify_recovery_v4_a14_a2 import ROOT
from eval_lib.generate_goal_verify_recovery_v4_a14_a2 import (
    _build_contract as _build_a14_a2_contract,
)

EVAL = ROOT / "eval/goal_verify/v0"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a14-a3-contract.json"

CONTRACT_ID = "phase6-recovery-v4-20260829-a14-a3-live-01"
SMOKE_ID = "phase6-recovery-v4-20260829-a14-a3-smoke-01"


def _build_contract(
    *,
    status: str,
    code_sha: str,
    exact_sha_ci_evidence: str,
    live_collection_authorized: bool,
) -> dict[str, Any]:
    contract = _build_a14_a2_contract(
        status=status,
        code_sha=code_sha,
        exact_sha_ci_evidence=exact_sha_ci_evidence,
        live_collection_authorized=live_collection_authorized,
    )
    contract.update(
        {
            "schema_version": (
                "commandagent.goal_verify.recovery_experiment.v4_a14_a3"
            ),
            "contract_id": CONTRACT_ID,
            "smoke_run_id": SMOKE_ID,
            "supersedes_contract": (
                "phase6-recovery-v4-20260829-a14-a2-live-01"
            ),
            "supersedes_smoke_run": (
                "phase6-recovery-v4-20260829-a14-a2-smoke-01"
            ),
        }
    )
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A14-A3",
            "reason": (
                "A14-A2 applied before/after fix-oracle polarity validation to "
                "the preregistered dependency-unavailable exclusion sentinel, "
                "which has no Recovery treatment by design"
            ),
            "historical_run_policy": (
                "A14-A2 smoke-01 remains immutable NO-GO evidence and is not "
                "rescored with the corrected report"
            ),
            "scope_correction": (
                "Recovery-effect oracle semantics apply only to pairs eligible "
                "for a Recovery treatment; excluded pairs remain governed by "
                "ineligible_recovery_not_executed"
            ),
        }
    )
    contract["analysis"]["oracle_semantics_validation_population"] = (
        "runtime_recovery_eligible_pairs_only"
    )
    contract["analysis"]["preregistered_exclusion_gate"] = (
        "ineligible_recovery_not_executed"
    )
    contract["runner_sources"].append(
        "scripts/eval_lib/generate_goal_verify_recovery_v4_a14_a3.py"
    )
    return contract


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate A14-A3 Recovery semantic-scope contract"
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
