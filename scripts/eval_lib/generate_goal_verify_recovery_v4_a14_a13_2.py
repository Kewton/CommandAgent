from __future__ import annotations

import argparse
from typing import Any

from eval_lib.generate_goal_verify_main_v4 import _write_json
from eval_lib.generate_goal_verify_recovery_v4_a14_a13_1 import (
    ROOT,
    _build_adapters,
    _build_tasks,
)
from eval_lib.generate_goal_verify_recovery_v4_a14_a13_1 import (
    _build_contract as _build_a14_a13_1_contract,
)
from eval_lib.goal_verify_recovery_experiment_v4 import (
    classify_case_recovery_eligibility,
)

EVAL = ROOT / "eval/goal_verify/v0"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a14-a13-2-contract.json"

CONTRACT_ID = "phase6-recovery-v4-20260830-a14-a13-2-live-01"
SMOKE_ID = "phase6-recovery-v4-20260830-a14-a13-2-smoke-01"


def _build_contract(
    *,
    status: str,
    code_sha: str,
    exact_sha_ci_evidence: str,
    live_collection_authorized: bool,
) -> dict[str, Any]:
    tasks = _build_tasks()
    adapters = _build_adapters()
    contract = _build_a14_a13_1_contract(
        status=status,
        code_sha=code_sha,
        exact_sha_ci_evidence=exact_sha_ci_evidence,
        live_collection_authorized=live_collection_authorized,
    )
    task_by_id = {row["case_id"]: row for row in tasks["cases"]}
    selected_case_ids = list(
        dict.fromkeys(
            pair_id.split("--pair-", 1)[0]
            for pair_id in contract["smoke"]["selected_pair_ids"]
        )
    )
    contract.update(
        {
            "schema_version": (
                "commandagent.goal_verify.recovery_experiment.v4_a14_a13_2"
            ),
            "contract_id": CONTRACT_ID,
            "smoke_run_id": SMOKE_ID,
            "supersedes_contract": (
                "phase6-recovery-v4-20260830-a14-a13-1-live-01"
            ),
            "supersedes_smoke_run": (
                "phase6-recovery-v4-20260830-a14-a13-1-smoke-01"
            ),
        }
    )
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A14-A13-2",
            "reason": (
                "A14-A13-1 correctly executed the nextjs-profile lane, but its task "
                "contract explicitly requires a Python project and forbids Next.js "
                "conversion; the product profile therefore demanded unavailable "
                "Next.js scaffolding before and during Recovery"
            ),
            "historical_run_policy": (
                "A14-A13-1 smoke-01 remains immutable 10-pair NO-GO evidence and is "
                "never rescored after preregistered eligibility correction"
            ),
            "inference_role": (
                "profile-contract exclusion and zero-Recovery sentinel diagnostic; "
                "no population effect claim"
            ),
            "instrument_findings": [
                "A14-A13-1 completed all 10 frozen pairs and passed 27 of 30 checks",
                "three nextjs pairs executed one Recovery and remained external fail",
                "the nextjs product profile requested scaffold paths forbidden by the task",
                "the rejected treatments caused zero external harm and zero regression",
                "the dependency sentinel executed zero Recoveries",
            ],
        }
    )
    contract["recovery_eligibility"]["preregistered_smoke_cases"] = {
        case_id: classify_case_recovery_eligibility(
            task_contract=task_by_id[case_id], adapters=adapters["adapters"]
        )
        for case_id in selected_case_ids
    }
    contract["recovery_eligibility"]["pre_run_categories"] = [
        "dependency_or_provisioning",
        "capability_unavailable",
        "profile_or_completion_contract",
    ]
    contract["analysis"].update(
        {
            "profile_contract_exclusion_policy": (
                "if the selected product profile is explicitly named in the task's "
                "do_not_convert_to constraints, configure and execute zero Recoveries"
            ),
            "full_experiment_redesign_required": (
                "A14-A14 must remove cell-08 from the eligible effect population and "
                "retain it as a non-Recovery profile-contract sentinel before freeze"
            ),
        }
    )
    contract["authorization"]["approved_at"] = (
        "2026-08-30" if live_collection_authorized else None
    )
    contract["runner_sources"].append(
        "scripts/eval_lib/generate_goal_verify_recovery_v4_a14_a13_2.py"
    )
    contract["runner_sources"] = list(dict.fromkeys(contract["runner_sources"]))
    return contract


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate A14-A13-2 profile-contract exclusion smoke"
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
