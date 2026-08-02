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
    def test_build_plan_is_inventory_only(self) -> None:
        plan = score_retrospective.build_plan(
            Path("campaign-summary.json"), fixture(), cwd=Path.cwd()
        )

        self.assertEqual(plan["mode"], "dry-run")
        self.assertEqual(plan["inventory"]["run_count"], 1)
        self.assertEqual(plan["inventory"]["runs"][0]["expected_events_sha256"], "b" * 64)
        self.assertEqual(plan["guards"]["new_judges"], 0)
        self.assertFalse(plan["guards"]["event_scan_performed"])
        self.assertFalse(plan["guards"]["score_computed"])

    def test_inventory_rejects_missing_event_hash(self) -> None:
        document = fixture()
        document["source_hashes"]["live_run_events_sha256"] = {}

        with self.assertRaisesRegex(score_retrospective.InputError, "missing an events sha256"):
            score_retrospective.run_inventory(document)

    def test_inventory_rejects_unknown_hash_entry(self) -> None:
        document = fixture()
        document["source_hashes"]["live_run_events_sha256"]["run-2"] = "c" * 64

        with self.assertRaisesRegex(score_retrospective.InputError, "unknown runs: run-2"):
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


if __name__ == "__main__":
    unittest.main()
