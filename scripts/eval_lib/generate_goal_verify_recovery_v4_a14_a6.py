from __future__ import annotations

import argparse
import copy
import hashlib
import json
import shlex
from typing import Any

from eval_lib.generate_goal_verify_main_v4 import _load, _write_json
from eval_lib.generate_goal_verify_recovery_v4_a14_a2 import ROOT
from eval_lib.generate_goal_verify_recovery_v4_a14_a5 import (
    _build_contract as _build_a14_a5_contract,
)

EVAL = ROOT / "eval/goal_verify/v0"
SOURCE_TASKS = EVAL / "phase6-task-contracts-v4-a14-a2.json"
SOURCE_ADAPTERS = EVAL / "phase6-command-adapters-v4-a14-a5.json"
TASKS_PATH = EVAL / "phase6-task-contracts-v4-a14-a6.json"
ADAPTERS_PATH = EVAL / "phase6-command-adapters-v4-a14-a6.json"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a14-a6-contract.json"

CONTRACT_ID = "phase6-recovery-v4-20260830-a14-a6-live-01"
SMOKE_ID = "phase6-recovery-v4-20260830-a14-a6-smoke-01"
SELECTED_CASE_IDS = [
    "phase6-main-c05-task-01",
    "phase6-main-c05-task-05",
    "phase6-main-c05-task-10",
]
REPRODUCER_FIXTURE = (
    ROOT
    / "tests/fixtures/goal_verify_v3/fix-reproduced-after-regression"
    / "before/tests/test_cli.py"
)


def _serialized_sha256(value: dict[str, Any]) -> str:
    text = json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    return hashlib.sha256(text.encode()).hexdigest()


def _build_tasks() -> dict[str, Any]:
    value = copy.deepcopy(_load(SOURCE_TASKS))
    for task in value["cases"]:
        reproducer = task.get("operational_constraints", {}).get("reproducer", {})
        argv = reproducer.get("argv")
        if not (
            task.get("case_id", "").startswith("phase6-main-c05-")
            and isinstance(argv, list)
            and argv
            and all(isinstance(token, str) and token for token in argv)
        ):
            continue
        task["completion_contract"]["fix_reproducer_command"] = shlex.join(argv)
    value["schema_version"] = "commandagent.goal_verify.task_contracts.v4_a14_a6"
    value["decision"] = (
        "A14-A6 types the already candidate-visible c05 reproducer argv as one "
        "CompletionContract fix command; goals and acceptance requirements are unchanged"
    )
    return value


def _build_adapters() -> dict[str, Any]:
    value = copy.deepcopy(_load(SOURCE_ADAPTERS))
    fixture = {
        "path": "tests/test_cli.py",
        "sha256": hashlib.sha256(REPRODUCER_FIXTURE.read_bytes()).hexdigest(),
    }
    for adapter in value["adapters"]:
        if not adapter.get("case_id", "").startswith("phase6-main-c05-"):
            continue
        executor = adapter["executor"]
        adapter_id = adapter["adapter_id"]
        if adapter_id.startswith("before-after-before--"):
            adapter["a14_role"] = "precondition"
            executor["kind"] = "fixture_hash_command"
            executor["registered_fixture"] = fixture
        elif adapter_id.startswith("before-after-after--"):
            adapter["a14_role"] = "final_success"
            executor["kind"] = "fixture_hash_command"
            executor["registered_fixture"] = fixture
        else:
            adapter["a14_role"] = "final_success"
    value["schema_version"] = "commandagent.goal_verify.adapters.v4_a14_a6"
    return value


def _build_contract(
    *,
    status: str,
    code_sha: str,
    exact_sha_ci_evidence: str,
    live_collection_authorized: bool,
) -> dict[str, Any]:
    tasks = _build_tasks()
    adapters = _build_adapters()
    contract = _build_a14_a5_contract(
        status=status,
        code_sha=code_sha,
        exact_sha_ci_evidence=exact_sha_ci_evidence,
        live_collection_authorized=live_collection_authorized,
    )
    contract.update(
        {
            "schema_version": (
                "commandagent.goal_verify.recovery_experiment.v4_a14_a6"
            ),
            "contract_id": CONTRACT_ID,
            "smoke_run_id": SMOKE_ID,
            "supersedes_contract": (
                "phase6-recovery-v4-20260830-a14-a5-live-01"
            ),
            "supersedes_smoke_run": (
                "phase6-recovery-v4-20260830-a14-a5-smoke-01"
            ),
            "task_contract_registry": str(TASKS_PATH.relative_to(ROOT)),
            "frozen_external_oracles": str(ADAPTERS_PATH.relative_to(ROOT)),
        }
    )
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A14-A6",
            "reason": (
                "a candidate-visible known fix reproducer expanded into seven model "
                "verify commands and prevented the valid F1 expected-failure boundary"
            ),
            "historical_run_policy": (
                "A14-A5 smoke-01 and both A14-A6 design diagnostics remain immutable; "
                "none are rescored into A14-A6"
            ),
            "inference_role": (
                "typed reproducer and initial-resolution readiness; Recovery effect is "
                "conditional on a naturally executed Recovery run"
            ),
            "instrument_findings": [
                "known candidate-visible reproducer argv lacked a typed completion field",
                "model planning and sanitizer expansion changed one F1 command into seven verifies",
                "typed F1 made design diagnostic task-01 pass the frozen final oracle without Recovery",
            ],
        }
    )
    commands = {
        f"{case_id}--pair-01": next(
            row["completion_contract"]["fix_reproducer_command"]
            for row in tasks["cases"]
            if row["case_id"] == case_id
        )
        for case_id in SELECTED_CASE_IDS
    }
    contract["smoke"].update(
        {
            "selected_pair_ids": list(commands),
            "expected_pair_count": len(commands),
            "typed_fix_reproducer_commands": commands,
            "minimum_executed_recovery_pairs": 0,
            "minimum_current_success_suppressions": 0,
            "require_executed_recovery_for_attribution": False,
            "require_browser_oracle_executability": False,
            "require_separate_browser_oracle_preflight": False,
            "browser_oracle_gate_source": "not_applicable_cli_only_smoke",
            "includes_dependency_exclusion_sentinel": False,
            "includes_next_manifest_regression": False,
            "inference_role": (
                "typed reproducer and initial-resolution diagnostic; conditional "
                "Recovery 0-vs-1 evidence only when executed"
            ),
            "required_readiness_checks": [
                "typed_fix_reproducer_binding",
                "fix_before_and_after_polarity_distinct",
                "frozen_external_oracle_post_execution",
                "internal_external_outcome_matrix_recorded",
                "maximum_one_recovery_executed",
            ],
        }
    )
    contract["analysis"].update(
        {
            "smoke_readiness_requires_live_recovery_execution": False,
            "typed_reproducer_candidate_visible": True,
            "typed_reproducer_is_external_oracle": False,
            "initial_resolution_and_recovery_effect_reported_separately": True,
        }
    )
    contract["recovery_eligibility"]["preregistered_smoke_cases"] = {
        case_id: {
            "eligible": True,
            "category": "recoverable_candidate",
            "reason": "task_inputs_and_frozen_external_oracles_available",
            "policy_id": "commandagent.goal_verify.recovery_eligibility.v4_a14",
        }
        for case_id in SELECTED_CASE_IDS
    }
    contract["authorization"]["approved_at"] = (
        "2026-08-30" if live_collection_authorized else None
    )
    contract.pop("oracle_executability_preflight", None)
    frozen = contract["frozen_input_sha256"]
    frozen.pop(str(SOURCE_TASKS.relative_to(ROOT)), None)
    frozen.pop(str(SOURCE_ADAPTERS.relative_to(ROOT)), None)
    for path in list(frozen):
        if path.startswith("tests/fixtures/goal_verify_v4_a14_a5/"):
            del frozen[path]
    frozen[str(TASKS_PATH.relative_to(ROOT))] = _serialized_sha256(tasks)
    frozen[str(ADAPTERS_PATH.relative_to(ROOT))] = _serialized_sha256(adapters)
    frozen[str(REPRODUCER_FIXTURE.relative_to(ROOT))] = hashlib.sha256(
        REPRODUCER_FIXTURE.read_bytes()
    ).hexdigest()
    contract["runner_sources"].append(
        "scripts/eval_lib/generate_goal_verify_recovery_v4_a14_a6.py"
    )
    return contract


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate A14-A6 typed fix reproducer contract"
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
