from __future__ import annotations

import argparse
from typing import Any

from eval_lib.generate_goal_verify_main_v4 import _write_json
from eval_lib.generate_goal_verify_recovery_v4_a14_a2 import ROOT
from eval_lib.generate_goal_verify_recovery_v4_a14_a10 import (
    _build_contract as _build_a14_a10_contract,
)

EVAL = ROOT / "eval/goal_verify/v0"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a14-a11-contract.json"

CONTRACT_ID = "phase6-recovery-v4-20260830-a14-a11-live-01"
SMOKE_ID = "phase6-recovery-v4-20260830-a14-a11-smoke-01"


def _build_contract(
    *,
    status: str,
    code_sha: str,
    exact_sha_ci_evidence: str,
    live_collection_authorized: bool,
) -> dict[str, Any]:
    contract = _build_a14_a10_contract(
        status=status,
        code_sha=code_sha,
        exact_sha_ci_evidence=exact_sha_ci_evidence,
        live_collection_authorized=live_collection_authorized,
    )
    contract.update(
        {
            "schema_version": (
                "commandagent.goal_verify.recovery_experiment.v4_a14_a11"
            ),
            "contract_id": CONTRACT_ID,
            "smoke_run_id": SMOKE_ID,
            "supersedes_contract": (
                "phase6-recovery-v4-20260830-a14-a10-live-01"
            ),
            "supersedes_smoke_run": (
                "phase6-recovery-v4-20260830-a14-a10-smoke-01"
            ),
        }
    )
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A14-A11",
            "reason": (
                "A14-A10 bound every inner Recovery check and removed the pytest "
                "false NG, but task-05 changed final acceptance from fix_intent_v0 "
                "to the generic CLI profile contract at the Recovery boundary"
            ),
            "historical_run_policy": (
                "A14-A10 smoke-01 remains immutable 28-of-28 instrument evidence; "
                "task-05 is not rescored after contract-continuity repair"
            ),
            "inference_role": (
                "fix-contract continuity and isolated treatment diagnostic; "
                "conditional 0-vs-1 evidence only, with no population effect claim"
            ),
            "instrument_findings": [
                "A14-A10 passed all 28 instrument checks",
                "task-10 became initial success after pytest evidence classification",
                "task-05 removed all unregistered inner verification commands",
                "task-05 then failed generic CLI behavior probing for absent cli/main.py",
                "the initial task used fix_intent_v0 while Recovery final acceptance used contract_origin initial",
            ],
        }
    )
    contract["smoke"].update(
        {
            "require_fix_contract_continuity": True,
            "inference_role": (
                "fix-contract continuity, inner Recovery command binding, "
                "and isolated treatment diagnostics"
            ),
        }
    )
    readiness_checks = contract["smoke"]["required_readiness_checks"]
    if "fix_contract_continuity" not in readiness_checks:
        readiness_checks.append("fix_contract_continuity")
    contract["analysis"].update(
        {
            "recovery_fix_contract_policy": (
                "copy the exact failed fix run identity and before evidence hash into "
                "the isolated treatment, then rerun after and the same frozen profile "
                "regression bindings before registered CompletionContract preflight"
            ),
            "recovery_fix_origin_source": (
                "host-owned read-only .commandagent/recovery-runtime/fix-origin.json"
            ),
            "generic_cli_probe_policy": (
                "unchanged for create/generic CLI acceptance; it is not disabled or "
                "weakened to admit root-level cli.py fix tasks"
            ),
        }
    )
    contract["authorization"]["approved_at"] = (
        "2026-08-30" if live_collection_authorized else None
    )
    contract["runner_sources"].append(
        "scripts/eval_lib/generate_goal_verify_recovery_v4_a14_a11.py"
    )
    return contract


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate A14-A11 Recovery fix-contract continuity contract"
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
