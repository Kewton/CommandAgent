"""Lock the ingest Luna suite to the elev-008 measurement configuration."""

from __future__ import annotations

import unittest
from pathlib import Path

import bench

SUITES_DIR = Path(__file__).resolve().parents[1] / "bench" / "suites"


class IngestLunaSuiteTests(unittest.TestCase):
    def test_only_executor_transport_and_run_names_differ_from_elevated(self) -> None:
        elevated = bench.load_suite(SUITES_DIR / "ingest-create-elevated.toml")
        luna = bench.load_suite(SUITES_DIR / "ingest-create-luna.toml")

        self.assertEqual(luna.provider, "openai")
        self.assertEqual(luna.api, "responses")
        self.assertEqual(luna.tool_protocol, "native")
        self.assertEqual(luna.planner_model, elevated.planner_model)
        self.assertEqual(luna.planner_provider, elevated.planner_provider)
        self.assertEqual(luna.profile, elevated.profile)
        self.assertEqual(luna.intent, elevated.intent)
        self.assertEqual(luna.plan_preset, elevated.plan_preset)
        self.assertEqual(luna.workspace_mode, elevated.workspace_mode)
        self.assertEqual(luna.context_budget, elevated.context_budget)
        self.assertEqual(luna.min_head, elevated.min_head)
        self.assertEqual(luna.goals, elevated.goals)
        self.assertEqual(luna.sources, elevated.sources)
        self.assertEqual(
            [(run.set_id, run.goal_id) for run in luna.runs],
            [(run.set_id, run.goal_id) for run in elevated.runs],
        )
        self.assertEqual(
            [run.name for run in luna.runs],
            [
                "list_luna_001",
                "list_luna_002",
                "list_luna_003",
                "table_luna_001",
                "table_luna_002",
                "table_luna_003",
            ],
        )
        self.assertTrue(all(run.executor == "gpt-5.6-luna" for run in luna.runs))


if __name__ == "__main__":
    unittest.main()
