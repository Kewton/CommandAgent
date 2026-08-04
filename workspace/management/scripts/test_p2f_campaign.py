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


if __name__ == "__main__":
    unittest.main()
