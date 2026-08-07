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
from unittest import mock

SCRIPTS_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPTS_DIR))

import p2f_campaign as p2f


class P2FCensusTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.declaration = json.loads(p2f.DECLARATION_PATH.read_text(encoding="utf-8"))
        cls.census = [
            p2f.CensusEntry(**entry)
            for entry in cls.declaration["population"]["entries"]
        ]

    def test_live_census_matches_snapshot_when_sources_are_available(self) -> None:
        if not all(source.campaign_path.is_dir() for source in p2f._sources()):
            self.skipTest("campaign source workspaces are intentionally external")
        self.assertEqual(p2f.build_census(), self.census)

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
        declaration = json.loads(p2f.DECLARATION_PATH.read_text(encoding="utf-8"))
        prediction = declaration["prediction"]
        self.assertIsNone(prediction["stratum_point_predictions"])
        self.assertFalse(declaration["measurement_started"])
        self.assertFalse(declaration["scope"]["new_repair_wiring"])
        self.assertFalse(declaration["scope"]["human_directive_injection"])
        self.assertEqual(prediction["predictive_full_count_band_95"], [0, 9])

    def test_census_markdown_matches_committed_declaration(self) -> None:
        declaration = json.loads(p2f.DECLARATION_PATH.read_text(encoding="utf-8"))
        expected_markdown = p2f.render_census(declaration)
        self.assertEqual(p2f.CENSUS_PATH.read_text(encoding="utf-8"), expected_markdown)

    def test_render_can_be_written_outside_the_repository(self) -> None:
        declaration = json.loads(p2f.DECLARATION_PATH.read_text(encoding="utf-8"))
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
        recovery = Path(".anvil/plans/recovery-ultra-plan-phase-final.yaml")
        argv = p2f._continuation_argv_from_source(
            [
                "commandagent",
                "--yes",
                "--intent",
                "create",
                "--context-budget",
                "65536",
                "--model",
                "executor",
                "--provider",
                "openai",
                "--planner-model",
                "planner",
                "--planner-provider",
                "openai",
                "--ultra-plan-run",
                "--profile",
                "cli",
                "build the original app",
            ],
            recovery,
        )
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


class P2FSettlementTests(unittest.TestCase):
    @staticmethod
    def build_recorded_settlement() -> dict[str, object]:
        declaration = json.loads(p2f.DECLARATION_PATH.read_text(encoding="utf-8"))
        recorded_tree = declaration["identity"]["production_path_tree"]
        with mock.patch.object(p2f, "_production_tree_pin", return_value=recorded_tree):
            return p2f.build_settlement("2026-08-05T03:18:57+09:00")

    def test_settlement_recomputes_observation_and_exchange(self) -> None:
        settlement = self.build_recorded_settlement()
        overall = settlement["overall"]
        self.assertEqual((overall["full"], overall["trials"]), (1, 10))
        self.assertEqual(
            overall["predeclared_beta_binomial_full_count_band_95"], [0, 9]
        )
        self.assertTrue(overall["within_predeclared_band"])
        self.assertAlmostEqual(overall["wilson_95"][0], 0.017876213095072896)
        self.assertAlmostEqual(overall["wilson_95"][1], 0.4041500267952385)
        self.assertEqual(
            settlement["score_change"],
            {
                "comparable_pairs": 6,
                "improved": 0,
                "unchanged": 1,
                "worsened": 5,
                "noncomparable_nullable_pairs": 4,
            },
        )
        exchange = settlement["exchange"]
        self.assertAlmostEqual(
            exchange["bon_new_trials"]["cost_usd_per_full"], 0.6508036
        )
        self.assertAlmostEqual(
            exchange["fix_failed_plus_one_continuation"]["cost_usd_per_full"],
            0.7894095,
        )
        self.assertAlmostEqual(
            exchange["single_reference"]["cost_usd_per_full"], 0.6000657
        )

    def test_settlement_outputs_match_canonical_build(self) -> None:
        settlement = self.build_recorded_settlement()
        result = json.loads(p2f.RESULT_PATH.read_text(encoding="utf-8"))
        expected_json = json.dumps(settlement, ensure_ascii=False, indent=2) + "\n"
        self.assertEqual(p2f.SETTLEMENT_PATH.read_text(encoding="utf-8"), expected_json)
        self.assertEqual(
            p2f.REPORT_PATH.read_text(encoding="utf-8"),
            p2f.render_report(settlement, result),
        )

    def test_settlement_keeps_no_go_and_byte_pins(self) -> None:
        settlement = self.build_recorded_settlement()
        integrity = settlement["integrity"]
        self.assertTrue(integrity["production_path_tree_matches_predeclaration"])
        self.assertTrue(integrity["band_byte_pins_match_predeclaration"])
        self.assertEqual(
            settlement["decision_material"]["automatic_bon_repair_connection"],
            "NO-GO remains",
        )
        self.assertEqual(
            settlement["decision_material"]["bon3_score_gate"], "not released"
        )

    def test_settlement_rejects_a_mismatched_production_tree(self) -> None:
        drifted_tree = {"file_count": 0, "sha256": "source-tree-drift"}
        with mock.patch.object(p2f, "_production_tree_pin", return_value=drifted_tree):
            with self.assertRaises(AssertionError):
                p2f.build_settlement("2026-08-05T03:18:57+09:00")


if __name__ == "__main__":
    unittest.main()
