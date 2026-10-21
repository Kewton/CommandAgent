from __future__ import annotations

import contextlib
import io
import json
import tempfile
import unittest
from pathlib import Path

import score_retrospective

REPOSITORY_ROOT = Path(__file__).resolve().parents[1]


def fixture() -> dict:
    return {
        "schema_version": "1",
        "uat_id": "uat-fixture",
        "campaign_id": "campaign-fixture",
        "revision": "a" * 40,
        "source_hashes": {"live_run_events_sha256": {"run-1": "b" * 64}},
        "runs": [{"name": "run-1", "family": "stats", "executor": "fixture"}],
    }


class ScoreRetrospectiveTests(unittest.TestCase):
    def test_fixed_score_floor_is_pass_absent_violation(self) -> None:
        scores = []
        for state in ("pass", "absent", "violation"):
            result = score_retrospective.score_atoms(
                {"atom": score_retrospective.AtomObservation(state, "fixture.json")}
            )
            scores.append(result["score"])

        self.assertEqual(scores, [100.0, 0.0, -50.0])

    def test_fail_is_half_weight_penalty(self) -> None:
        result = score_retrospective.score_atoms(
            {
                "one": score_retrospective.AtomObservation("pass", "fixture"),
                "two": score_retrospective.AtomObservation("pass", "fixture"),
                "three": score_retrospective.AtomObservation("violation", "fixture"),
                "four": score_retrospective.AtomObservation("pass", "fixture"),
            }
        )

        self.assertEqual(result["weighted_state_sum_twice"], 5)
        self.assertEqual(result["score"], 62.5)

    def test_unobserved_vector_is_not_a_zero_score_run(self) -> None:
        result = score_retrospective.score_atoms(
            {"atom": score_retrospective.AtomObservation("unobserved", "fixture")}
        )

        self.assertFalse(result["reached"])
        self.assertIsNone(result["score"])

    def test_spearman_uses_average_tie_ranks(self) -> None:
        coefficient = score_retrospective.spearman(
            [0.0, 0.0, 1.0, 2.0], [0.0, 0.0, 1.0, 1.0]
        )

        self.assertIsNotNone(coefficient)
        assert coefficient is not None
        self.assertGreater(coefficient, 0.8)

    def test_build_plan_is_inventory_only(self) -> None:
        plan = score_retrospective.build_plan(
            Path("campaign-summary.json"), fixture(), cwd=Path.cwd()
        )

        self.assertEqual(plan["mode"], "dry-run")
        self.assertEqual(plan["inventory"]["run_count"], 1)
        self.assertEqual(
            plan["inventory"]["runs"][0]["expected_events_sha256"], "b" * 64
        )
        self.assertEqual(plan["guards"]["new_judges"], 0)
        self.assertFalse(plan["guards"]["event_scan_performed"])
        self.assertFalse(plan["guards"]["score_computed"])

    def test_inventory_rejects_missing_event_hash(self) -> None:
        document = fixture()
        document["source_hashes"]["live_run_events_sha256"] = {}

        with self.assertRaisesRegex(
            score_retrospective.InputError, "missing an events sha256"
        ):
            score_retrospective.run_inventory(document)

    def test_inventory_rejects_unknown_hash_entry(self) -> None:
        document = fixture()
        document["source_hashes"]["live_run_events_sha256"]["run-2"] = "c" * 64

        with self.assertRaisesRegex(
            score_retrospective.InputError, "unknown runs: run-2"
        ):
            score_retrospective.run_inventory(document)

    def test_cli_requires_explicit_dry_run(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "campaign-summary.json"
            path.write_text(json.dumps(fixture()), encoding="utf-8")
            with (
                contextlib.redirect_stderr(io.StringIO()),
                self.assertRaisesRegex(SystemExit, "2"),
            ):
                score_retrospective.main(["--campaign-summary", str(path)])

    def test_cli_emits_json_without_writing_output(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "campaign-summary.json"
            path.write_text(json.dumps(fixture()), encoding="utf-8")
            stdout = io.StringIO()
            with contextlib.redirect_stdout(stdout):
                rc = score_retrospective.main(
                    ["--campaign-summary", str(path), "--dry-run"]
                )

        self.assertEqual(rc, 0)
        output = json.loads(stdout.getvalue())
        self.assertEqual(output["study_status"], "not_executed_pending_adjudication")
        self.assertFalse(output["guards"]["historical_files_mutated"])

    def test_committed_luna_008_sample_matches_dry_run(self) -> None:
        summary_path = REPOSITORY_ROOT / (
            "workspace/management/runs/uat-test0801-cli-luna-008/"
            "evidence/campaign-summary.json"
        )
        sample_path = (
            REPOSITORY_ROOT / "docs/f1-score-retrospective-luna-008.sample.json"
        )

        document = score_retrospective.load_campaign_summary(summary_path)
        plan = score_retrospective.build_plan(
            summary_path, document, cwd=REPOSITORY_ROOT
        )
        sample = json.loads(sample_path.read_text(encoding="utf-8"))

        self.assertEqual(plan, sample)

    def test_repository_scan_covers_all_run_level_band_rows(self) -> None:
        module = score_retrospective.load_band_module()
        finals, checkpoints = score_retrospective.scan_profiles(module)

        # d39c84a3368d8b59ca0d450b439ad32133e8b78e added six Luna rows
        # after this snapshot was sealed. Preserve every historical row exactly.
        frozen_path = REPOSITORY_ROOT / (
            "workspace/management/runs/f1-retrospective-001/final-vectors.jsonl"
        )
        frozen = [
            json.loads(line)
            for line in frozen_path.read_text(encoding="utf-8").splitlines()
            if line.strip()
        ]
        frozen_by_id = {row["run_id"]: row for row in frozen}
        actual_by_id = {row["run_id"]: row for row in finals}
        self.assertEqual(len(frozen), 287)
        self.assertEqual(len(frozen_by_id), 287)
        self.assertEqual(len(actual_by_id), len(finals))
        historical = [row for row in finals if row["run_id"] in frozen_by_id]
        self.assertEqual(len(historical), 287)
        self.assertEqual({row["run_id"]: row for row in historical}, frozen_by_id)
        frozen_counts = {
            "circle": 33,
            "cli": 98,
            "data": 60,
            "fix": 30,
            "ingest": 54,
            "investigation": 12,
        }
        historical_coverage = score_retrospective.coverage_rows(historical, checkpoints)
        self.assertEqual(
            {row["profile"]: row["scannable_runs"] for row in historical_coverage},
            frozen_counts,
        )
        self.assertEqual(sum(row["final_verdict"] == "full" for row in historical), 10)

        luna_ids = {
            f"uat-test0802-ingest-luna-001/{family}_luna_{index:03d}"
            for family in ("list", "table")
            for index in range(1, 4)
        }
        self.assertEqual(set(actual_by_id) - set(frozen_by_id), luna_ids)
        for run_id in sorted(luna_ids):
            with self.subTest(run_id=run_id):
                row = actual_by_id[run_id]
                self.assertEqual(row["profile"], "ingest")
                self.assertEqual(row["model"], "gpt-5.6-luna")
                self.assertEqual(row["final_verdict"], "full")
                self.assertEqual(row["final_assurance"], "full")
                self.assertIs(row["reached"], True)

        self.assertEqual(len(finals), 293)
        coverage = score_retrospective.coverage_rows(finals, checkpoints)
        self.assertEqual(
            {row["profile"]: row["scannable_runs"] for row in coverage},
            {**frozen_counts, "ingest": 60},
        )
        full = [item for item in finals if item["final_verdict"] == "full"]
        self.assertEqual(len(full), 16)
        self.assertTrue(all(item["score"] == 100.0 for item in full))
        self.assertTrue(
            all(atom["state"] == "pass" for item in full for atom in item["atoms"])
        )


if __name__ == "__main__":
    unittest.main()
