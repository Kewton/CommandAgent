from __future__ import annotations

import argparse
import shlex
from typing import Any

from eval_lib.generate_goal_verify_main_v4 import _write_json
from eval_lib.generate_goal_verify_recovery_v4_a14_a2 import (
    ROOT,
    _json_sha256,
)
from eval_lib.generate_goal_verify_recovery_v4_a14_a13 import (
    _build_adapters,
)
from eval_lib.generate_goal_verify_recovery_v4_a14_a13 import (
    _build_contract as _build_a14_a13_contract,
)
from eval_lib.generate_goal_verify_recovery_v4_a14_a13 import (
    _build_tasks as _build_a14_a13_tasks,
)
from eval_lib.goal_verify_recovery_experiment_v4 import (
    classify_case_recovery_eligibility,
)

EVAL = ROOT / "eval/goal_verify/v0"
TASKS_PATH = EVAL / "phase6-task-contracts-v4-a14-a13-1.json"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a14-a13-1-contract.json"

CONTRACT_ID = "phase6-recovery-v4-20260830-a14-a13-1-live-01"
SMOKE_ID = "phase6-recovery-v4-20260830-a14-a13-1-smoke-01"


def _build_tasks() -> dict[str, Any]:
    tasks = _build_a14_a13_tasks()
    for case in tasks["cases"]:
        if not case["case_id"].startswith(("phase6-main-c07-", "phase6-main-c08-")):
            continue
        command = case["completion_contract"]["fix_reproducer_command"]
        case["operational_constraints"]["reproducer"] = {
            "argv": shlex.split(command),
            "expected_exit_before": 1,
            "expected_exit_after": 0,
            "stage_before": "before",
        }
        case["decision"] = (
            "A14-A13-1 binds the typed fix reproducer to the same structured "
            "operational constraint consumed by task consistency validation"
        )
    return tasks


def _build_contract(
    *,
    status: str,
    code_sha: str,
    exact_sha_ci_evidence: str,
    live_collection_authorized: bool,
) -> dict[str, Any]:
    tasks = _build_tasks()
    adapters = _build_adapters()
    contract = _build_a14_a13_contract(
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
                "commandagent.goal_verify.recovery_experiment.v4_a14_a13_1"
            ),
            "contract_id": CONTRACT_ID,
            "smoke_run_id": SMOKE_ID,
            "supersedes_contract": (
                "phase6-recovery-v4-20260830-a14-a13-live-01"
            ),
            "supersedes_smoke_run": (
                "phase6-recovery-v4-20260830-a14-a13-smoke-01"
            ),
            "task_contract_registry": str(TASKS_PATH.relative_to(ROOT)),
        }
    )
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A14-A13-1",
            "reason": (
                "A14-A13 added completion_contract.fix_reproducer_command for "
                "generic and nextjs but omitted the matching structured "
                "operational_constraints.reproducer binding"
            ),
            "historical_run_policy": (
                "A14-A13 smoke-01 remains immutable early-terminated evidence at "
                "3 of 10 pairs and is never resumed or pooled"
            ),
            "inference_role": (
                "corrected three-profile repeated-pair instrument smoke; no "
                "population effect claim"
            ),
            "instrument_findings": [
                "all three CLI repetitions completed before the binding failure",
                "the first generic binding failed before its product invocation",
                "selected-case binding was previously lazy and allowed partial collection",
            ],
        }
    )
    contract["recovery_eligibility"]["preregistered_smoke_cases"] = {
        case_id: classify_case_recovery_eligibility(
            task_contract=task_by_id[case_id], adapters=adapters["adapters"]
        )
        for case_id in selected_case_ids
    }
    contract["analysis"]["selected_task_binding_preflight"] = (
        "bind and validate every selected case before manifest creation and before "
        "the first product invocation"
    )
    contract["authorization"]["approved_at"] = (
        "2026-08-30" if live_collection_authorized else None
    )
    old_task_path = (
        "eval/goal_verify/v0/phase6-task-contracts-v4-a14-a13.json"
    )
    contract["frozen_input_sha256"].pop(old_task_path, None)
    contract["frozen_input_sha256"][contract["task_contract_registry"]] = (
        _json_sha256(tasks)
    )
    contract["runner_sources"].append(
        "scripts/eval_lib/generate_goal_verify_recovery_v4_a14_a13_1.py"
    )
    contract["runner_sources"] = list(dict.fromkeys(contract["runner_sources"]))
    return contract


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate A14-A13-1 prebound Recovery instrument smoke"
    )
    parser.add_argument("--code-sha")
    parser.add_argument("--exact-sha-ci-evidence")
    parser.add_argument("--smoke-collection-authorized", action="store_true")
    args = parser.parse_args()
    if bool(args.code_sha) != bool(args.exact_sha_ci_evidence):
        parser.error("--code-sha and --exact-sha-ci-evidence must be paired")
    if args.smoke_collection_authorized and not args.code_sha:
        parser.error("smoke authorization requires exact-SHA inputs")
    _write_json(TASKS_PATH, _build_tasks())
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
