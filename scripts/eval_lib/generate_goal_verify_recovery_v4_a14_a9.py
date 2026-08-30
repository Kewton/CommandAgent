from __future__ import annotations

import argparse
from typing import Any

from eval_lib.generate_goal_verify_main_v4 import _write_json
from eval_lib.generate_goal_verify_recovery_v4_a14_a2 import ROOT
from eval_lib.generate_goal_verify_recovery_v4_a14_a8 import (
    _build_contract as _build_a14_a8_contract,
)

EVAL = ROOT / "eval/goal_verify/v0"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a14-a9-contract.json"

CONTRACT_ID = "phase6-recovery-v4-20260830-a14-a9-live-01"
SMOKE_ID = "phase6-recovery-v4-20260830-a14-a9-smoke-01"


def _build_contract(
    *,
    status: str,
    code_sha: str,
    exact_sha_ci_evidence: str,
    live_collection_authorized: bool,
) -> dict[str, Any]:
    contract = _build_a14_a8_contract(
        status=status,
        code_sha=code_sha,
        exact_sha_ci_evidence=exact_sha_ci_evidence,
        live_collection_authorized=live_collection_authorized,
    )
    contract.update(
        {
            "schema_version": (
                "commandagent.goal_verify.recovery_experiment.v4_a14_a9"
            ),
            "contract_id": CONTRACT_ID,
            "smoke_run_id": SMOKE_ID,
            "supersedes_contract": (
                "phase6-recovery-v4-20260830-a14-a8-live-01"
            ),
            "supersedes_smoke_run": (
                "phase6-recovery-v4-20260830-a14-a8-smoke-01"
            ),
        }
    )
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A14-A9",
            "reason": (
                "A14-A8 proved treatment-owned CompletionContract binding, but "
                "the executed step-level Recovery candidate inherited three "
                "LLM-authored shell fragments instead of the registered final-"
                "success commands"
            ),
            "historical_run_policy": (
                "A14-A8 smoke-01 remains immutable contract-binding evidence; "
                "its unchanged-fail treatment is not rescored as A14-A9"
            ),
            "inference_role": (
                "registered Recovery verification-command provenance and "
                "conditional 0-vs-1 effect diagnostic; no population effect claim"
            ),
            "instrument_findings": [
                "A14-A8 loaded the treatment-owned contract and executed one Recovery",
                "the selected handoff commands were python3 cli.py 16, exit_code=$?, and [ $exit_code -eq 2 ]",
                "independent command execution left exit_code undefined and stopped Recovery",
                "A14-A9 rebinds the automatic Recovery plan to exact CompletionContract commands",
                "binding failure remains a fail-closed stop before treatment execution",
            ],
        }
    )
    contract["smoke"].update(
        {
            "require_registered_recovery_verify_commands": True,
            "inference_role": (
                "registered Recovery command provenance plus isolated treatment "
                "binding; conditional evidence only for naturally executed Recovery pairs"
            ),
        }
    )
    readiness_checks = contract["smoke"]["required_readiness_checks"]
    if "registered_recovery_verify_commands" not in readiness_checks:
        readiness_checks.append("registered_recovery_verify_commands")
    contract["analysis"].update(
        {
            "recovery_verify_command_authority": "CompletionContract.verify_commands",
            "unregistered_handoff_command_policy": (
                "replace plan before execution using registered commands and "
                "read-only preflight evidence; fail closed if rebinding fails"
            ),
            "stateful_shell_fragment_policy": (
                "never carry unregistered fragments into automatic Recovery"
            ),
        }
    )
    contract["authorization"]["approved_at"] = (
        "2026-08-30" if live_collection_authorized else None
    )
    contract["runner_sources"].append(
        "scripts/eval_lib/generate_goal_verify_recovery_v4_a14_a9.py"
    )
    return contract


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate A14-A9 registered Recovery command contract"
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
