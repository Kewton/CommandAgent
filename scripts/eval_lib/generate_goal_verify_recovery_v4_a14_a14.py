from __future__ import annotations

import argparse
from typing import Any

from eval_lib.generate_goal_verify_main_v4 import _write_json
from eval_lib.generate_goal_verify_recovery_v4_a14_a13 import ADAPTERS_PATH
from eval_lib.generate_goal_verify_recovery_v4_a14_a13_1 import (
    ROOT,
    TASKS_PATH,
    _build_adapters,
    _build_tasks,
)
from eval_lib.generate_goal_verify_recovery_v4_a14_a13_3 import (
    _build_contract as _build_a14_a13_3_contract,
)
from eval_lib.goal_verify_recovery_experiment_v4 import (
    classify_case_recovery_eligibility,
)

EVAL = ROOT / "eval/goal_verify/v0"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a14-a14-contract.json"

CONTRACT_ID = "phase6-recovery-v4-20260830-a14-a14-live-01"
RUN_ID = "phase6-recovery-v4-20260830-a14-a14-live-01"
ELIGIBLE_CELL_IDS = ["cell-05", "cell-07"]
ELIGIBLE_CASE_IDS = [
    f"phase6-main-c{cell_number:02d}-task-{task_number:02d}"
    for cell_number in (5, 7)
    for task_number in range(1, 11)
]
SENTINEL_CASE_IDS = [
    f"phase6-main-c{cell_number:02d}-task-{task_number:02d}"
    for cell_number in (6, 8)
    for task_number in range(1, 11)
]


def _eligible_pair_ids() -> list[str]:
    return [
        f"{case_id}--pair-{sample_index:02d}"
        for case_id in ELIGIBLE_CASE_IDS
        for sample_index in range(1, 4)
    ]


def _sentinel_pair_ids() -> list[str]:
    return [f"{case_id}--pair-01" for case_id in SENTINEL_CASE_IDS]


def _build_contract(
    *,
    status: str,
    code_sha: str,
    exact_sha_ci_evidence: str,
    live_collection_authorized: bool,
) -> dict[str, Any]:
    tasks = _build_tasks()
    adapters = _build_adapters()
    contract = _build_a14_a13_3_contract(
        status=status,
        code_sha=code_sha,
        exact_sha_ci_evidence=exact_sha_ci_evidence,
        live_collection_authorized=live_collection_authorized,
    )
    eligible_pair_ids = _eligible_pair_ids()
    sentinel_pair_ids = _sentinel_pair_ids()
    selected_pair_ids = [*eligible_pair_ids, *sentinel_pair_ids]
    task_by_id = {row["case_id"]: row for row in tasks["cases"]}
    all_case_ids = [*ELIGIBLE_CASE_IDS, *SENTINEL_CASE_IDS]
    eligibility = {
        case_id: classify_case_recovery_eligibility(
            task_contract=task_by_id[case_id], adapters=adapters["adapters"]
        )
        for case_id in all_case_ids
    }
    typed_commands = {
        pair_id: task_by_id[pair_id.split("--pair-", 1)[0]][
            "completion_contract"
        ]["fix_reproducer_command"]
        for pair_id in eligible_pair_ids
    }
    contract.update(
        {
            "schema_version": (
                "commandagent.goal_verify.recovery_experiment.v4_a14_a14"
            ),
            "contract_id": CONTRACT_ID,
            "smoke_run_id": RUN_ID,
            "supersedes_contract": (
                "phase6-recovery-v4-20260830-a14-a13-3-live-01"
            ),
            "supersedes_smoke_run": (
                "phase6-recovery-v4-20260830-a14-a13-3-smoke-01"
            ),
        }
    )
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A14-A14",
            "reason": (
                "pre-register the first population Recovery 0-vs-1 estimate after "
                "the corrected A14-A13-3 completion-safe Recovery smoke"
            ),
            "historical_run_policy": (
                "all A14 through A14-A13-3 runs remain immutable instrument "
                "evidence and are excluded from this effect estimate"
            ),
            "inference_role": (
                "fixed 60-pair eligible fix population plus 20 dependency or "
                "profile-contract sentinels"
            ),
        }
    )
    contract["recovery_eligibility"]["preregistered_smoke_cases"] = eligibility
    contract["smoke"].update(
        {
            "selected_pair_ids": selected_pair_ids,
            "expected_pair_count": len(selected_pair_ids),
            "typed_fix_reproducer_commands": typed_commands,
            "minimum_executed_recovery_pairs": 30,
            "includes_dependency_exclusion_sentinel": True,
            "includes_next_manifest_regression": True,
            "inference_role": "full preregistered fix-intent Recovery experiment",
            "effect_claim_allowed": False,
        }
    )
    contract["full_experiment"] = {
        "effect_claim_allowed": True,
        "eligible_cell_ids": ELIGIBLE_CELL_IDS,
        "eligible_case_ids": ELIGIBLE_CASE_IDS,
        "sentinel_case_ids": SENTINEL_CASE_IDS,
        "eligible_pair_ids": eligible_pair_ids,
        "sentinel_pair_ids": sentinel_pair_ids,
        "eligible_pair_count": 60,
        "sentinel_pair_count": 20,
        "cluster_unit": "source_task_id",
        "stratification_unit": "cell_id",
        "minimum_clusters_per_cell": 10,
        "pairs_per_eligible_cluster": 3,
        "minimum_executed_recovery_pairs": 30,
        "bootstrap_samples": 2000,
        "bootstrap_seed": 3991414,
        "confidence_interval": 0.95,
        "primary_estimand": (
            "eligible-pair mean of +1 frozen external fail-to-pass, -1 pass-to-"
            "non-pass, and 0 otherwise, equally weighted by profile after "
            "stratified task-cluster resampling"
        ),
        "stopping_rule": (
            "collect exactly the frozen 60 eligible pairs and 20 sentinels; do not "
            "extend, replace, exclude, or relabel pairs after observing outcomes"
        ),
        "go_rule": (
            "all instrument gates; at least 30 executed Recoveries; 2,000-sample "
            "95% CI lower bound above zero; zero harm, regression, unusable, and "
            "sentinel Recovery; four resource budgets met"
        ),
        "resource_budget_basis": (
            "fixed before full collection from A14-A12 one-rescue diagnostics with "
            "headroom for profile variation; evaluated only on executed Recoveries"
        ),
        "resource_budgets": {
            "wall_time_ms": {"p50": 240000, "p95": 600000},
            "total_tokens": {"p50": 60000, "p95": 120000},
        },
    }
    contract["analysis"].update(
        {
            "primary_population": (
                "preregistered eligible fix tasks in cli and generic; dependency "
                "and explicit profile-contract sentinels are retained but excluded "
                "from the effect denominator by frozen role"
            ),
            "bootstrap_method": (
                "stratified hierarchical paired percentile: resample task clusters "
                "within profile cells, then repetitions within task; 2,000 samples"
            ),
            "default_rollout_policy": (
                "forbidden unless the frozen full-experiment GO rule passes"
            ),
            "recovery_runs_above_one": "out of scope and forbidden",
        }
    )
    contract["authorization"].update(
        {
            "smoke_collection_authorized": live_collection_authorized,
            "full_collection_authorized": live_collection_authorized,
            "approved_at": "2026-08-30" if live_collection_authorized else None,
        }
    )
    contract["runner_sources"].extend(
        [
            "scripts/eval_lib/generate_goal_verify_recovery_v4_a14_a14.py",
            "scripts/eval_lib/goal_verify_recovery_full_report_v4.py",
            "scripts/eval_lib/goal_verify_stats_v2.py",
        ]
    )
    contract["runner_sources"] = list(dict.fromkeys(contract["runner_sources"]))
    return contract


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate A14-A14 full Recovery effect experiment"
    )
    parser.add_argument("--code-sha")
    parser.add_argument("--exact-sha-ci-evidence")
    parser.add_argument("--full-collection-authorized", action="store_true")
    args = parser.parse_args()
    if bool(args.code_sha) != bool(args.exact_sha_ci_evidence):
        parser.error("--code-sha and --exact-sha-ci-evidence must be paired")
    if args.full_collection_authorized and not args.code_sha:
        parser.error("full authorization requires exact-SHA inputs")
    contract = _build_contract(
        status="frozen" if args.code_sha else "draft",
        code_sha=args.code_sha or "",
        exact_sha_ci_evidence=args.exact_sha_ci_evidence or "",
        live_collection_authorized=args.full_collection_authorized,
    )
    _write_json(TASKS_PATH, _build_tasks())
    _write_json(ADAPTERS_PATH, _build_adapters())
    _write_json(CONTRACT_PATH, contract)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
