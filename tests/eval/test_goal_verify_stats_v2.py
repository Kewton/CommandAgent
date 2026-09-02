import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_stats_v2 import (
    cluster_paired_bootstrap_interval,
    stratified_cluster_paired_bootstrap_interval,
    validate_cluster_design,
)


class GoalVerifyStatsV2Test(unittest.TestCase):
    def test_cluster_design_requires_multiple_tasks_and_unique_pairs(self):
        rows = [
            {"cell_id": "create", "source_task_id": "task-a", "pair_id": "a-1"},
            {"cell_id": "create", "source_task_id": "task-a", "pair_id": "a-2"},
            {"cell_id": "create", "source_task_id": "task-b", "pair_id": "b-1"},
            {"cell_id": "create", "source_task_id": "task-b", "pair_id": "b-2"},
        ]
        self.assertEqual(
            validate_cluster_design(
                rows, minimum_clusters_per_cell=2, minimum_pairs_per_cluster=2
            ),
            [],
        )
        rows[-1]["pair_id"] = "a-1"
        errors = validate_cluster_design(
            rows, minimum_clusters_per_cell=3, minimum_pairs_per_cluster=2
        )
        self.assertTrue(any("duplicate pair_id" in error for error in errors))
        self.assertTrue(any("requires 3" in error for error in errors))

    def test_cluster_design_rejects_task_id_shared_across_cells(self):
        rows = [
            {"cell_id": "cell-01", "source_task_id": "task-a", "pair_id": "a-1"},
            {"cell_id": "cell-02", "source_task_id": "task-a", "pair_id": "a-2"},
        ]
        errors = validate_cluster_design(
            rows, minimum_clusters_per_cell=1, minimum_pairs_per_cluster=1
        )
        self.assertIn("source_task_id spans cells: task-a:cell-01,cell-02", errors)

    def test_cluster_bootstrap_is_deterministic_and_cluster_weighted(self):
        rows = [
            {"source_task_id": "task-a", "delta": 1.0},
            {"source_task_id": "task-a", "delta": 3.0},
            {"source_task_id": "task-b", "delta": 10.0},
            {"source_task_id": "task-b", "delta": 10.0},
        ]
        first = cluster_paired_bootstrap_interval(
            rows,
            delta=lambda row: row["delta"],
            samples=2000,
            seed=399,
        )
        second = cluster_paired_bootstrap_interval(
            rows,
            delta=lambda row: row["delta"],
            samples=2000,
            seed=399,
        )
        self.assertEqual(first, second)
        self.assertEqual(first["cluster_count"], 2)
        self.assertEqual(first["pair_count"], 4)
        self.assertLessEqual(first["lower"], 2.0)
        self.assertGreaterEqual(first["upper"], 10.0)

    def test_one_cluster_is_insufficient_evidence(self):
        interval = cluster_paired_bootstrap_interval(
            [{"source_task_id": "only", "delta": 1.0}],
            delta=lambda row: row["delta"],
            samples=2000,
            seed=399,
        )
        self.assertEqual(interval["status"], "insufficient_evidence")

    def test_stratified_cluster_bootstrap_preserves_cells_and_is_deterministic(self):
        rows = [
            {
                "cell_id": cell_id,
                "source_task_id": f"{cell_id}-task-{task}",
                "delta": delta,
            }
            for cell_id, delta in (("cli", 1.0), ("generic", 0.0))
            for task in (1, 2)
            for _ in range(3)
        ]
        first = stratified_cluster_paired_bootstrap_interval(
            rows, delta=lambda row: row["delta"], samples=2000, seed=39914
        )
        second = stratified_cluster_paired_bootstrap_interval(
            rows, delta=lambda row: row["delta"], samples=2000, seed=39914
        )

        self.assertEqual(first, second)
        self.assertEqual(first["method"], "stratified_hierarchical_cluster_paired_percentile")
        self.assertEqual(first["stratum_count"], 2)
        self.assertEqual(first["cluster_count"], 4)
        self.assertEqual(first["lower"], 0.5)
        self.assertEqual(first["upper"], 0.5)


if __name__ == "__main__":
    unittest.main()
