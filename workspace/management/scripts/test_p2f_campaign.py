#!/usr/bin/env python3
"""Focused tests for the P2F-0 census and statistical declaration."""

from __future__ import annotations

import json
import math
import sys
import tempfile
import unittest
from collections import Counter
from pathlib import Path

SCRIPTS_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPTS_DIR))

import p2f_campaign as p2f


class P2FCensusTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.census = p2f.build_census()

    def test_census_is_complete_and_routes_exist(self) -> None:
        self.assertEqual(len(self.census), 44)
        self.assertEqual(
            Counter(entry.source_label for entry in self.census),
            {
                "bon-local-001": 5,
                "bon0-001": 5,
                "bon0-002r": 6,
                "bon0-003r": 6,
                "bon0-004r": 6,
                "luna-006": 6,
                "luna-007": 5,
                "luna-008": 5,
            },
        )
        self.assertNotIn("bon0-001r2", {entry.source_label for entry in self.census})
        self.assertTrue(all(entry.workspace_exists for entry in self.census))
        self.assertTrue(all(entry.fix_continuation_applicable for entry in self.census))
        self.assertTrue(all(entry.recovery_plan_exists for entry in self.census))
        self.assertFalse(any(entry.recovery_circle_applicable for entry in self.census))

    def test_score_denominators_are_not_invented(self) -> None:
        self.assertEqual(
            Counter(entry.score_band for entry in self.census),
            {
                "unreached": 19,
                "mid:37.5-<75": 14,
                "low:<37.5": 10,
                "high:75-<100": 1,
            },
        )
        local = [entry for entry in self.census if entry.profile == "nextjs"]
        self.assertEqual(len(local), 5)
        self.assertTrue(all(entry.score is None for entry in local))

    def test_cross_strata_and_sample_follow_fixed_rule(self) -> None:
        strata = Counter(entry.stratum for entry in self.census)
        self.assertEqual(len(strata), 9)
        self.assertEqual(strata[("cli_polarity", "mid:37.5-<75")], 9)
        first = p2f.select_sample(self.census)
        second = p2f.select_sample(reversed(self.census))
        self.assertEqual(first, second)
        self.assertEqual(len(first), 10)
        self.assertEqual(len({entry.census_id for entry in first}), 10)
        sample_strata = Counter(entry.stratum for entry in first)
        self.assertEqual(set(sample_strata), set(strata))
        self.assertEqual(sample_strata[("cli_polarity", "mid:37.5-<75")], 2)
        self.assertTrue(
            all(
                count == 1
                for cell, count in sample_strata.items()
                if cell != ("cli_polarity", "mid:37.5-<75")
            )
        )


class P2FStatisticsTests(unittest.TestCase):
    def test_wilson_interval_for_one_of_three(self) -> None:
        lower, upper = p2f.wilson_interval(1, 3)
        self.assertAlmostEqual(lower, 0.06149194472039621)
        self.assertAlmostEqual(upper, 0.7923403991979522)

    def test_beta_binomial_band_is_broad_and_normalized(self) -> None:
        probabilities = p2f.beta_binomial_probabilities(10, 1.5, 2.5)
        self.assertEqual(len(probabilities), 11)
        self.assertTrue(math.isclose(sum(probabilities), 1.0))
        self.assertEqual(p2f.equal_tail_count_band(probabilities), (0, 9))
        mean = sum(index * value for index, value in enumerate(probabilities))
        self.assertAlmostEqual(mean, 3.75)

    def test_declaration_has_no_stratum_point_prediction(self) -> None:
        declaration = p2f.build_declaration("2026-08-05T00:30:41+09:00")
        prediction = declaration["prediction"]
        self.assertIsNone(prediction["stratum_point_predictions"])
        self.assertFalse(declaration["measurement_started"])
        self.assertFalse(declaration["scope"]["new_repair_wiring"])
        self.assertFalse(declaration["scope"]["human_directive_injection"])
        self.assertEqual(prediction["predictive_full_count_band_95"], [0, 9])

    def test_generated_files_match_canonical_build(self) -> None:
        declaration = p2f.build_declaration("2026-08-05T00:30:41+09:00")
        expected_json = json.dumps(declaration, ensure_ascii=False, indent=2) + "\n"
        expected_markdown = p2f.render_census(declaration)
        self.assertEqual(
            p2f.DECLARATION_PATH.read_text(encoding="utf-8"), expected_json
        )
        self.assertEqual(p2f.CENSUS_PATH.read_text(encoding="utf-8"), expected_markdown)

    def test_render_can_be_written_outside_the_repository(self) -> None:
        declaration = p2f.build_declaration("2026-08-05T00:30:41+09:00")
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "census.md"
            path.write_text(p2f.render_census(declaration), encoding="utf-8")
            self.assertIn("measurement had started", path.read_text(encoding="utf-8"))


class P2FExecutionHarnessTests(unittest.TestCase):
    def test_recorded_measurement_preserves_predeclared_order_and_scope(self) -> None:
        if not p2f.RESULT_PATH.is_file():
            self.skipTest("measurement has not been recorded yet")
        declaration = json.loads(p2f.DECLARATION_PATH.read_text(encoding="utf-8"))
        result = json.loads(p2f.RESULT_PATH.read_text(encoding="utf-8"))
        runs = result["runs"]
        self.assertEqual(result["status"], "complete")
        self.assertEqual(result["sample_size"], 10)
        self.assertEqual(len(runs), 10)
        self.assertEqual(
            [run["census_id"] for run in runs],
            declaration["sampling"]["selected_ids"],
        )
        self.assertEqual(result["full_count"], sum(run["full"] for run in runs))
        self.assertAlmostEqual(
            result["duration_seconds_total"],
            sum(run["duration_seconds"] for run in runs),
        )
        self.assertAlmostEqual(
            result["cost_usd_total"], sum(run["cost_usd"] for run in runs)
        )
        self.assertTrue(all(run["repair_cycles"] == 1 for run in runs))
        self.assertTrue(all(run["directive"] is None for run in runs))
        self.assertTrue(
            all(
                run["source_workspace_tree_sha256_before"]
                == run["source_workspace_tree_sha256_after"]
                for run in runs
            )
        )

    def test_continuation_argv_replaces_only_the_action_and_goal(self) -> None:
        declaration = json.loads(p2f.DECLARATION_PATH.read_text(encoding="utf-8"))
        entry = declaration["sampling"]["selected_entries"][1]
        source_workspace = Path(entry["workspace"])
        with tempfile.TemporaryDirectory() as temporary:
            copied = Path(temporary)
            recovery = Path(entry["recovery_plan"]).relative_to(source_workspace)
            copied_recovery = copied / recovery
            copied_recovery.parent.mkdir(parents=True)
            copied_recovery.write_bytes(Path(entry["recovery_plan"]).read_bytes())
            argv = p2f.continuation_argv(entry, copied)
        self.assertEqual(argv[0], str(p2f.EXECUTION_BINARY))
        self.assertNotIn("--intent", argv)
        self.assertNotIn("--ultra-plan-run", argv)
        self.assertEqual(argv.count("--run-ultra-plan"), 1)
        self.assertEqual(argv[-1], recovery.as_posix())
        self.assertIn("--profile", argv)
        self.assertIn("cli", argv)

    def test_usage_cost_counts_all_openai_turns_only(self) -> None:
        events = [
            {
                "event": "provider_turn_duration",
                "provider": "openai",
                "prompt_eval_count": 1000,
                "provider_cached_input_tokens": 600,
                "eval_count": 100,
            },
            {
                "event": "provider_turn_duration",
                "provider": "ollama",
                "prompt_eval_count": 9000,
                "provider_cached_input_tokens": 0,
                "eval_count": 900,
            },
        ]
        usage, cost = p2f._usage_and_cost(events)
        self.assertEqual(
            usage,
            {
                "input_tokens": 1000,
                "cached_input_tokens": 600,
                "output_tokens": 100,
            },
        )
        self.assertAlmostEqual(cost, 0.00106)

    def test_cli_vector_requires_a_probe_in_the_new_event_file(self) -> None:
        checks = {
            "cli_probe": "failed",
            "help_binding": "pass",
            "cli_output_claims": "pass",
            "cli_rerun_consistency": "pass",
        }
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary)
            evidence = workspace / "evidence/cli-assurance.json"
            evidence.parent.mkdir()
            evidence.write_text(
                json.dumps({"evidence": {"checks": checks}}), encoding="utf-8"
            )
            unreached = p2f._cli_score_vector(workspace, [])
            reached = p2f._cli_score_vector(
                workspace, [{"event": "profile_behavior_probe"}]
            )
        self.assertFalse(unreached["reached"])
        self.assertIsNone(unreached["score"])
        self.assertTrue(reached["reached"])
        self.assertEqual(reached["score"], 62.5)


if __name__ == "__main__":
    unittest.main()
