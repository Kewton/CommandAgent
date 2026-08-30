from __future__ import annotations

import argparse
import copy
from typing import Any

from eval_lib.generate_goal_verify_main_v4 import _load, _write_json
from eval_lib.generate_goal_verify_recovery_v4_a14_a2 import (
    ROOT,
    _file_sha256,
    _json_sha256,
)
from eval_lib.generate_goal_verify_recovery_v4_a14_a12 import (
    _build_contract as _build_a14_a12_contract,
)
from eval_lib.goal_verify_recovery_experiment_v4 import (
    classify_case_recovery_eligibility,
)

EVAL = ROOT / "eval/goal_verify/v0"
SOURCE_TASKS = EVAL / "phase6-task-contracts-v4-a14-a6-1.json"
SOURCE_ADAPTERS = EVAL / "phase6-command-adapters-v4-a14-a6-1.json"
TASKS_PATH = EVAL / "phase6-task-contracts-v4-a14-a13.json"
ADAPTERS_PATH = EVAL / "phase6-command-adapters-v4-a14-a13.json"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a14-a13-contract.json"

CONTRACT_ID = "phase6-recovery-v4-20260830-a14-a13-live-01"
SMOKE_ID = "phase6-recovery-v4-20260830-a14-a13-smoke-01"
ELIGIBLE_SMOKE_CASES = [
    "phase6-main-c05-task-05",
    "phase6-main-c07-task-01",
    "phase6-main-c08-task-01",
]
SENTINEL_CASE = "phase6-main-c06-task-01"


def _build_tasks() -> dict[str, Any]:
    tasks = copy.deepcopy(_load(SOURCE_TASKS))
    for case in tasks["cases"]:
        case_id = case["case_id"]
        if case_id.startswith(("phase6-main-c07-", "phase6-main-c08-")):
            verify_commands = case["completion_contract"]["verify_commands"]
            case["completion_contract"]["fix_reproducer_command"] = verify_commands[0]
            case["decision"] = (
                "A14-A13 registers the existing exact before-failure command as the "
                "typed fix reproducer; final success and regression requirements are "
                "unchanged"
            )
    tasks["schema_version"] = "commandagent.goal_verify.task_contracts.v4_a14_a13"
    return tasks


def _build_adapters() -> dict[str, Any]:
    registry = copy.deepcopy(_load(SOURCE_ADAPTERS))
    additions = []
    for adapter in registry["adapters"]:
        if not adapter.get("case_id", "").startswith("phase6-main-c08-"):
            continue
        if adapter.get("adapter_id", "").startswith("after-executed--"):
            before = copy.deepcopy(adapter)
            before["adapter_id"] = before["adapter_id"].replace(
                "after-executed--", "before-reproducer--", 1
            )
            before["a14_role"] = "precondition"
            before["executor"]["stage"] = "before"
            before["executor"]["observation"] = {
                "kind": "exit_code",
                "expected": 1,
                "rule": (
                    "the exact registered repro.py input must fail on the frozen "
                    "before snapshot"
                ),
            }
            additions.append(before)
    registry["adapters"].extend(additions)
    registry["schema_version"] = "commandagent.goal_verify.adapters.v4_a14_a13"
    return registry


def _selected_pair_ids() -> list[str]:
    return [
        *[
            f"{case_id}--pair-{sample_index:02d}"
            for case_id in ELIGIBLE_SMOKE_CASES
            for sample_index in range(1, 4)
        ],
        f"{SENTINEL_CASE}--pair-01",
    ]


def _build_contract(
    *,
    status: str,
    code_sha: str,
    exact_sha_ci_evidence: str,
    live_collection_authorized: bool,
) -> dict[str, Any]:
    tasks = _build_tasks()
    adapters = _build_adapters()
    contract = _build_a14_a12_contract(
        status=status,
        code_sha=code_sha,
        exact_sha_ci_evidence=exact_sha_ci_evidence,
        live_collection_authorized=live_collection_authorized,
    )
    selected_pair_ids = _selected_pair_ids()
    task_by_id = {row["case_id"]: row for row in tasks["cases"]}
    selected_cases = [*ELIGIBLE_SMOKE_CASES, SENTINEL_CASE]
    eligibility = {
        case_id: classify_case_recovery_eligibility(
            task_contract=task_by_id[case_id], adapters=adapters["adapters"]
        )
        for case_id in selected_cases
    }
    typed_commands = {
        pair_id: task_by_id[pair_id.split("--pair-", 1)[0]]["completion_contract"][
            "fix_reproducer_command"
        ]
        for pair_id in selected_pair_ids
        if pair_id.split("--pair-", 1)[0] in ELIGIBLE_SMOKE_CASES
    }
    contract.update(
        {
            "schema_version": (
                "commandagent.goal_verify.recovery_experiment.v4_a14_a13"
            ),
            "contract_id": CONTRACT_ID,
            "smoke_run_id": SMOKE_ID,
            "supersedes_contract": ("phase6-recovery-v4-20260830-a14-a12-live-01"),
            "supersedes_smoke_run": ("phase6-recovery-v4-20260830-a14-a12-smoke-01"),
            "task_contract_registry": str(TASKS_PATH.relative_to(ROOT)),
            "frozen_external_oracles": str(ADAPTERS_PATH.relative_to(ROOT)),
        }
    )
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A14-A13",
            "reason": (
                "A14-A12 demonstrated one CLI rescue but did not exercise repeated "
                "pairs or typed fix continuity for generic and nextjs profiles"
            ),
            "historical_run_policy": (
                "A14-A12 smoke-01 remains immutable one-rescue diagnostic evidence; "
                "it is not pooled into the later population effect estimate"
            ),
            "inference_role": (
                "three-profile repeated-pair and dependency-sentinel instrument smoke; "
                "no population effect claim"
            ),
        }
    )
    contract["recovery_eligibility"]["preregistered_smoke_cases"] = eligibility
    contract["smoke"].update(
        {
            "selected_pair_ids": selected_pair_ids,
            "expected_pair_count": len(selected_pair_ids),
            "typed_fix_reproducer_commands": typed_commands,
            "minimum_executed_recovery_pairs": 1,
            "includes_dependency_exclusion_sentinel": True,
            "includes_next_manifest_regression": True,
            "inference_role": (
                "repeat parsing, typed fix continuity across cli/generic/nextjs, "
                "and dependency exclusion diagnostics"
            ),
            "effect_claim_allowed": False,
        }
    )
    contract["analysis"].update(
        {
            "smoke_cluster_design": (
                "one task in each eligible fix profile repeated three times; one "
                "declared-offline dependency sentinel repeated once"
            ),
            "full_experiment_dependency": (
                "do not freeze or collect the full effect experiment until this "
                "stratified instrument smoke passes without amendment"
            ),
        }
    )
    contract["authorization"]["approved_at"] = (
        "2026-08-30" if live_collection_authorized else None
    )
    contract["runner_sources"].append(
        "scripts/eval_lib/generate_goal_verify_recovery_v4_a14_a13.py"
    )
    contract["runner_sources"] = list(dict.fromkeys(contract["runner_sources"]))
    contract["frozen_input_sha256"] = {
        contract["corpus"]: _file_sha256(ROOT / contract["corpus"]),
        contract["task_contract_registry"]: _json_sha256(tasks),
        contract["frozen_external_oracles"]: _json_sha256(adapters),
        contract["workspace_registry"]: _file_sha256(
            ROOT / contract["workspace_registry"]
        ),
        contract["workspace_registry_additions"]: _file_sha256(
            ROOT / contract["workspace_registry_additions"]
        ),
        contract["resource_budget_config"]: _file_sha256(
            ROOT / contract["resource_budget_config"]
        ),
    }
    return contract


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate A14-A13 stratified Recovery instrument smoke"
    )
    parser.add_argument("--code-sha")
    parser.add_argument("--exact-sha-ci-evidence")
    parser.add_argument("--smoke-collection-authorized", action="store_true")
    args = parser.parse_args()
    if bool(args.code_sha) != bool(args.exact_sha_ci_evidence):
        parser.error("--code-sha and --exact-sha-ci-evidence must be paired")
    if args.smoke_collection_authorized and not args.code_sha:
        parser.error("smoke authorization requires exact-SHA inputs")
    tasks = _build_tasks()
    adapters = _build_adapters()
    contract = _build_contract(
        status="frozen" if args.code_sha else "draft",
        code_sha=args.code_sha or "",
        exact_sha_ci_evidence=args.exact_sha_ci_evidence or "",
        live_collection_authorized=args.smoke_collection_authorized,
    )
    _write_json(TASKS_PATH, tasks)
    _write_json(ADAPTERS_PATH, adapters)
    _write_json(CONTRACT_PATH, contract)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
