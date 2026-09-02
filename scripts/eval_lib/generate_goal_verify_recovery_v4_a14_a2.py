from __future__ import annotations

import argparse
import copy
import hashlib
import json
from pathlib import Path
from typing import Any

from eval_lib.generate_goal_verify_main_v4 import _load, _write_json
from eval_lib.generate_goal_verify_recovery_v4_a14 import SMOKE_CASE_IDS
from eval_lib.generate_goal_verify_recovery_v4_a14_a1 import (
    _build_contract as _build_a14_a1_contract,
)
from eval_lib.goal_verify_recovery_experiment_v4 import (
    classify_case_recovery_eligibility,
)

ROOT = Path(__file__).resolve().parents[2]
EVAL = ROOT / "eval/goal_verify/v0"
SOURCE_TASKS = EVAL / "phase6-task-contracts-v4-a13-main.json"
SOURCE_ADAPTERS = EVAL / "phase6-command-adapters-v4-a13-main.json"
TASKS_PATH = EVAL / "phase6-task-contracts-v4-a14-a2.json"
ADAPTERS_PATH = EVAL / "phase6-command-adapters-v4-a14-a2.json"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a14-a2-contract.json"

CONTRACT_ID = "phase6-recovery-v4-20260829-a14-a2-live-01"
SMOKE_ID = "phase6-recovery-v4-20260829-a14-a2-smoke-01"


def _json_sha256(value: dict[str, Any]) -> str:
    serialized = json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    return hashlib.sha256(serialized.encode()).hexdigest()


def _file_sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _build_tasks() -> dict[str, Any]:
    tasks = copy.deepcopy(_load(SOURCE_TASKS))
    adapters = _load(SOURCE_ADAPTERS)["adapters"]
    for case in tasks["cases"]:
        if not case["case_id"].startswith("phase6-main-c01-"):
            continue
        observations = []
        for adapter in adapters:
            if adapter.get("case_id") != case["case_id"]:
                continue
            executor = adapter.get("executor", {})
            observation = executor.get("observation", {})
            if executor.get("kind") not in {"sandbox_command", "stage_command"}:
                continue
            if observation.get("kind") != "stdout":
                continue
            expected = observation.get("expected")
            if not isinstance(expected, str):
                continue
            observations.append(
                {
                    "argv": executor["argv"],
                    "expected_exit_code": 0,
                    "expected_stdout": expected.rstrip("\n") + "\n",
                }
            )
        contract = case["completion_contract"]
        contract["command_observations"] = observations
        contract["required_capabilities"] = []
        contract["deterministic_oracles"] = [
            "host_owned_exact_cli_command_observations"
        ]
        contract["required_evidence"] = ["implementation_artifact"]
        contract["required_obligations"] = ["implementation"]
        case["decision"] = (
            "A14-A2 uses host-owned exact argv/exit/stdout observations and does not "
            "reintroduce README, help, unknown-option, or synthetic test obligations"
        )
    tasks["schema_version"] = "commandagent.goal_verify.task_contracts.v4_a14_a2"
    return tasks


def _build_adapters() -> dict[str, Any]:
    value = copy.deepcopy(_load(SOURCE_ADAPTERS))
    adapters = value["adapters"]
    fix_preconditions = []
    for adapter in adapters:
        adapter["a14_role"] = "final_success"
        if adapter.get("case_id", "").startswith("phase6-main-c06-"):
            adapter["executor"]["blocked_patterns"] = [
                "ModuleNotFoundError",
                "No module named",
            ]
        if adapter.get("case_id", "").startswith("phase6-main-c07-"):
            adapter["a14_role"] = "precondition"
            fix_preconditions.append(adapter)
    for before in fix_preconditions:
        suffix = before["adapter_id"].removeprefix("exact-reproducer--")
        after = copy.deepcopy(before)
        after["adapter_id"] = f"final-success--{suffix}"
        after["a14_role"] = "final_success"
        after["executor"]["stage"] = "after"
        after["executor"]["observation"] = {
            "kind": "exit_code",
            "expected": 0,
            "rule": (
                "the exact registered fixture and argv must terminate successfully on "
                "the final product snapshot"
            ),
        }
        adapters.append(after)
        adapters.append(
            {
                "adapter_id": f"forbidden-substitution-absent--{suffix}",
                "case_id": before["case_id"],
                "claim_id": before["claim_id"],
                "a14_role": "final_success",
                "executor": {
                    "kind": "file_content",
                    "path": "app.py",
                    "pattern": "fixture/control.json",
                    "polarity": "absent",
                    "observed_strength": "runtime",
                    "stage": "after",
                    "workspace": before["executor"]["workspace"],
                },
                "proposal": {
                    "strategies": ["file"],
                    "polarities": ["absent"],
                    "observation_kinds": ["file_content"],
                    "input_binding": {
                        "kind": "file",
                        "path": "app.py",
                        "strategies": ["file"],
                    },
                },
            },
        )
    value["schema_version"] = "commandagent.goal_verify.adapters.v4_a14_a2"
    return value


def _build_contract(
    *,
    status: str,
    code_sha: str,
    exact_sha_ci_evidence: str,
    live_collection_authorized: bool,
) -> dict[str, Any]:
    contract = _build_a14_a1_contract(
        status=status,
        code_sha=code_sha,
        exact_sha_ci_evidence=exact_sha_ci_evidence,
        live_collection_authorized=live_collection_authorized,
    )
    tasks = _build_tasks()
    adapter_registry = _build_adapters()
    adapters = adapter_registry["adapters"]
    task_by_id = {row["case_id"]: row for row in tasks["cases"]}
    eligibility = {
        case_id: classify_case_recovery_eligibility(
            task_contract=task_by_id[case_id], adapters=adapters
        )
        for case_id in SMOKE_CASE_IDS
    }
    contract.update(
        {
            "schema_version": "commandagent.goal_verify.recovery_experiment.v4_a14_a2",
            "contract_id": CONTRACT_ID,
            "smoke_run_id": SMOKE_ID,
            "supersedes_contract": "phase6-recovery-v4-20260829-a14-a1-live-01",
            "supersedes_smoke_run": "phase6-recovery-v4-20260829-a14-a1-smoke-01",
            "task_contract_registry": str(TASKS_PATH.relative_to(ROOT)),
            "frozen_external_oracles": str(ADAPTERS_PATH.relative_to(ROOT)),
            "pre_live_amendments": [
                {
                    "amendment_id": "v4-A14-A2",
                    "reason": (
                        "A14-A1 used independent stochastic initial attempts, a "
                        "before-failure oracle as final success, and profile/contract "
                        "checks that could trigger unnecessary Recovery"
                    ),
                    "historical_run_policy": (
                        "A14 and A14-A1 runs remain immutable diagnostic evidence and "
                        "are never rescored as Recovery-effect evidence"
                    ),
                    "product_findings": [
                        "Next.js ExistingPlainJavaScript entrypoints were absent from lint",
                        "CLI profile reintroduced unregistered README/help obligations",
                        "fix final-success oracle had reversed exit-code polarity",
                        "independent arms did not share the treatment boundary",
                    ],
                }
            ],
        }
    )
    paired = contract["paired_run_contract"]
    paired.update(
        {
            "pairing_unit": "shared_pre_recovery_snapshot",
            "independent_workspace_copies": False,
            "same_input_snapshot_sha256_required": True,
            "same_physical_initial_attempt_required": True,
            "initial_only": {
                "recovery_plan_auto_runs": 0,
                "execution": "captured pre-Recovery control; no second LLM invocation",
            },
            "recovery_one": {
                "recovery_plan_auto_runs": 1,
                "execution": "same-run post-Recovery treatment",
            },
            "estimand": (
                "within-run change from the captured pre-Recovery snapshot to the "
                "post-Recovery snapshot, with at most one Recovery Plan"
            ),
        }
    )
    paired.pop("stochastic_pairing_limitation", None)
    contract["recovery_eligibility"]["preregistered_smoke_cases"] = eligibility
    contract["external_oracle_policy"].update(
        {
            "timing": "pre-Recovery snapshot and post-Recovery snapshot in one run",
            "semantic_roles_required": ["precondition", "final_success"],
            "fix_before_failure_never_used_as_final_success": True,
            "typed_negative_assertions_required": True,
            "dependency_blocked_is_third_state": True,
        }
    )
    contract["analysis"].update(
        {
            "attribution_requires_shared_initial_history": True,
            "oracle_semantics_validation_required": True,
            "initial_success_attributed_as_improvement": False,
        }
    )
    contract["smoke"].update(
        {
            "minimum_executed_recovery_pairs": 1,
            "effect_claim_allowed": False,
            "required_readiness_checks": [
                "pre_recovery_snapshot_matches_control",
                "pre_recovery_failure_handoff_recorded",
                "final_success_oracle_semantics_validated",
                "fix_before_and_after_polarity_distinct",
                "recovery_attribution_requires_shared_initial_history",
                "internal_external_outcome_matrix_recorded",
                "initial_success_pair_not_attributed",
                "recovery_changed_paths_recorded",
            ],
        }
    )
    contract["runner_sources"].extend(
        [
            "scripts/eval_lib/generate_goal_verify_recovery_v4_a14_a2.py",
            "scripts/eval_lib/goal_verify_executors_v3.py",
            "scripts/eval_lib/goal_verify_task_contracts_v4.py",
        ]
    )
    contract["runner_sources"] = list(dict.fromkeys(contract["runner_sources"]))
    contract["frozen_input_sha256"] = {
        contract["corpus"]: _file_sha256(ROOT / contract["corpus"]),
        contract["task_contract_registry"]: _json_sha256(tasks),
        contract["frozen_external_oracles"]: _json_sha256(adapter_registry),
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
        description="Generate A14-A2 shared-boundary Recovery inputs"
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
