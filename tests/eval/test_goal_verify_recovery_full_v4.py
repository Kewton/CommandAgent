import copy
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_recovery_full_report_v4 import (
    build_recovery_full_report,
)


def full_contract():
    eligible_pair_ids = [
        f"{cell}-task-{task:02d}--pair-{sample:02d}"
        for cell in ("cli", "generic", "nextjs")
        for task in range(1, 11)
        for sample in range(1, 4)
    ]
    sentinel_pair_ids = [f"dependency-task-{task:02d}--pair-01" for task in range(10)]
    return {
        "full_experiment": {
            "effect_claim_allowed": True,
            "eligible_pair_ids": eligible_pair_ids,
            "sentinel_pair_ids": sentinel_pair_ids,
            "eligible_cell_ids": ["cli", "generic", "nextjs"],
            "minimum_clusters_per_cell": 10,
            "pairs_per_eligible_cluster": 3,
            "minimum_executed_recovery_pairs": 30,
            "bootstrap_samples": 2000,
            "bootstrap_seed": 3991414,
            "primary_estimand": "paired frozen-oracle success delta",
            "resource_budgets": {
                "wall_time_ms": {"p50": 240000, "p95": 600000},
                "total_tokens": {"p50": 60000, "p95": 120000},
            },
        }
    }


def full_records(contract):
    records = []
    for pair_id in contract["full_experiment"]["eligible_pair_ids"]:
        cluster_id, sample = pair_id.rsplit("--pair-", 1)
        cell_id = cluster_id.split("-task-", 1)[0]
        records.append(
            {
                "pair_id": pair_id,
                "cell_id": cell_id,
                "source_task_id": cluster_id,
                "sample_index": int(sample),
                "eligibility": {"preregistered": {"eligible": True}},
                "comparison": {
                    "quality_transition": "improved",
                    "executed_recovery_runs": 1,
                    "regression_introduced": False,
                    "resource_delta": {
                        "wall_time_ms": 120000,
                        "total_tokens": 40000,
                    },
                },
            }
        )
    records.extend(
        {
            "pair_id": pair_id,
            "eligibility": {"preregistered": {"eligible": False}},
            "recovery_one": {
                "result": {"recovery_plan_attempts": {"executed_recovery_runs": 0}}
            },
        }
        for pair_id in contract["full_experiment"]["sentinel_pair_ids"]
    )
    return records


class GoalVerifyRecoveryFullV4Test(unittest.TestCase):
    @patch(
        "eval_lib.goal_verify_recovery_full_report_v4.build_recovery_report",
        return_value={"instrument_ready": True, "effect_attribution_ready": True},
    )
    def test_full_report_requires_cluster_ci_safety_and_budgets(self, _base_report):
        contract = full_contract()
        records = full_records(contract)

        report = build_recovery_full_report(records=records, contract=contract)

        self.assertEqual(report["go_no_go"], "GO")
        self.assertTrue(report["effect_claim_allowed"])
        self.assertEqual(report["primary_effect"]["point"], 1.0)
        self.assertEqual(report["primary_effect"]["bootstrap"]["lower"], 1.0)
        self.assertEqual(report["recovery_execution"]["executed_recovery_pairs"], 90)
        self.assertTrue(all(report["full_experiment_checks"].values()))

        harmed = copy.deepcopy(records)
        harmed[0]["comparison"]["quality_transition"] = "harmed"
        harmed_report = build_recovery_full_report(records=harmed, contract=contract)
        self.assertEqual(harmed_report["go_no_go"], "NO-GO")
        self.assertFalse(
            harmed_report["full_experiment_checks"]["existing_artifact_harm_zero"]
        )


if __name__ == "__main__":
    unittest.main()
