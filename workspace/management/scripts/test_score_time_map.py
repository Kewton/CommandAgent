#!/usr/bin/env python3
"""Focused tests for the generated score/time projection."""

from __future__ import annotations

import tempfile
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path

import band_aggregate as band
import score_time_map as score_map


def observation(
    run_id: str,
    *,
    score: float | None,
    seconds: float | None = 10.0,
    full: bool = False,
    instance_id: str | None = None,
    instance_seconds: float | None = None,
    instance_success: bool | None = None,
) -> score_map.Observation:
    return score_map.Observation(
        profile="cli",
        model="model",
        family="filter",
        configuration="bon:3" if instance_id else "single",
        marker="verdict_mapping",
        run_id=run_id,
        score=score,
        full=full,
        duration_seconds=seconds,
        cost_usd=0.01,
        instance_id=instance_id or run_id,
        instance_seconds=instance_seconds if instance_id else seconds,
        instance_cost_usd=0.03 if instance_id else 0.01,
        instance_success=full if instance_success is None else instance_success,
        source="fixture",
    )


class ProjectionDisciplineTests(unittest.TestCase):
    def test_all_formal_runs_contribute_without_rewriting_null_scores(self) -> None:
        observations = [
            observation("r1", score=100.0, full=True),
            observation("r2", score=None),
            observation("r3", score=50.0),
        ]

        cell = score_map.aggregate_cells(observations, band.score_quantile)[0]

        self.assertEqual(cell.n, 3)
        self.assertEqual(cell.reached, 2)
        self.assertEqual(cell.mean_score, 50.0)
        self.assertEqual(cell.five_number, (0.0, 25.0, 50.0, 75.0, 100.0))
        self.assertIsNone(observations[1].score)

    def test_n_and_time_gates_fail_closed_but_keep_rows(self) -> None:
        too_small = [
            observation("r1", score=100.0),
            observation("r2", score=100.0),
        ]
        missing_time = [
            observation("r1", score=100.0, seconds=None),
            observation("r2", score=100.0),
            observation("r3", score=100.0),
        ]

        small_cell = score_map.aggregate_cells(too_small, band.score_quantile)[0]
        missing_cell = score_map.aggregate_cells(missing_time, band.score_quantile)[0]

        self.assertFalse(small_cell.plotted)
        self.assertEqual(small_cell.plot_reason, "n不足")
        self.assertFalse(missing_cell.plotted)
        self.assertEqual(missing_cell.plot_reason, "時間欠落")

    def test_bon_uses_campaign_total_and_campaign_success_denominator(self) -> None:
        observations = [
            observation(
                f"r{index}",
                score=100.0 if index == 1 else None,
                full=index == 1,
                instance_id="campaign-1",
                instance_seconds=90.0,
                instance_success=True,
            )
            for index in range(1, 4)
        ]

        cell = score_map.aggregate_cells(observations, band.score_quantile)[0]

        self.assertEqual(cell.n, 3)
        self.assertEqual(cell.instance_count, 1)
        self.assertEqual(cell.mean_seconds, 90.0)
        self.assertEqual(cell.successful_instances, 1)
        self.assertEqual(cell.expected_seconds_per_success, 90.0)
        self.assertAlmostEqual(cell.expected_cost_per_success or 0.0, 0.03)


class RepositoryProjectionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.markdown, cls.svg, cls.cells = score_map.build_score_time_map(band)
        cls.observations, _sources = score_map.collect_observations(band)

    def test_frozen_and_post_seal_denominators_are_complete(self) -> None:
        self.assertEqual(len(self.observations), 335)
        historical = self.observations[: score_map.HISTORICAL_VECTOR_COUNT]
        self.assertEqual(
            sum(item.marker == "verdict_mapping" for item in historical), 251
        )
        self.assertEqual(sum(item.marker == "checkpoint" for item in historical), 36)
        self.assertEqual(
            len(self.observations[score_map.HISTORICAL_VECTOR_COUNT :]), 48
        )

    def test_required_reference_points_exist(self) -> None:
        keys = {
            (cell.profile, cell.model, cell.family, cell.configuration): cell
            for cell in self.cells
        }
        self.assertEqual(keys[("cli", "gpt-5.6-luna", "filter", "single")].n, 24)
        self.assertEqual(keys[("cli", "gpt-5.6-luna", "filter", "bon:6")].n, 30)
        self.assertEqual(keys[("ingest", "gpt-5.6-luna", "list", "single")].n, 3)
        self.assertEqual(keys[("ingest", "gpt-5.6-luna", "table", "single")].n, 3)
        local = keys[
            (
                "nextjs",
                "qwen3.6:35b-a3b-coding-nvfp4",
                "Breakout",
                "bon:6",
            )
        ]
        self.assertEqual((local.n, local.instance_count, local.full), (6, 1, 1))

    def test_output_is_deterministic_and_self_describing(self) -> None:
        second_markdown, second_svg, _cells = score_map.build_score_time_map(band)
        self.assertEqual(self.markdown, second_markdown)
        self.assertEqual(self.svg, second_svg)
        self.assertTrue(
            self.markdown.startswith("<!-- GENERATED FILE: DO NOT EDIT. -->")
        )
        self.assertIn(score_map.MAP_COMMAND, self.markdown)
        self.assertIn("## 成功1件あたり期待時間・費用", self.markdown)
        reading = self.markdown.split("## 読み", 1)[1].split("## 正準数値表", 1)[0]
        self.assertEqual(reading.count("\n- "), 3)
        self.assertIn("<circle", self.svg)
        self.assertIn("<polygon", self.svg)
        ET.fromstring(self.svg)

    def test_writing_map_leaves_existing_band_bytes_unchanged(self) -> None:
        band_paths = [
            band.OUTPUT,
            band.DATA_OUTPUT,
            band.FIX_OUTPUT,
            band.INVESTIGATION_OUTPUT,
            band.CIRCLE_OUTPUT,
            band.CLI_OUTPUT,
            band.INGEST_OUTPUT,
        ]
        before = {path: path.read_bytes() for path in band_paths}
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            score_map.write_score_time_map(
                band,
                markdown_path=root / "score_time_map.md",
                svg_path=root / "score_time_map.svg",
            )
        self.assertEqual(before, {path: path.read_bytes() for path in band_paths})


if __name__ == "__main__":
    unittest.main()
