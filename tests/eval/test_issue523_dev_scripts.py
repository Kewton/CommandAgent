"""Regression tests for Issue #523 dev/eval script findings.

FIN-044: eval_lib.process.command_available must not shell out with a
suite-supplied name. FIN-046: eval_lib.run_summary.read_summary must not
silently absorb a summary.eval.tsv header that drifts from SUMMARY_HEADER.

The band_aggregate pin check (FIN-043) and the codex_orchestrate zip length
check (FIN-045) are covered by their focused suites
(workspace/management/scripts/test_band_aggregate.py and
tests/test_codex_orchestrate.py).
"""

import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.process import command_available
from eval_lib.run_summary import read_summary, write_summary


class CommandAvailableTest(unittest.TestCase):
    def test_known_command_is_available(self):
        self.assertTrue(command_available("python3"))

    def test_unknown_command_is_unavailable(self):
        self.assertFalse(command_available("definitely-not-a-real-command-issue523"))

    def test_shell_metacharacters_are_not_interpreted(self):
        self.assertFalse(command_available("python3; echo injected"))


class SummaryHeaderValidationTest(unittest.TestCase):
    def test_unknown_columns_are_rejected(self):
        with tempfile.TemporaryDirectory() as td:
            path = Path(td) / "summary.eval.tsv"
            path.write_text(
                "run_id\tsuite\tunexpected_column\nx\ts\tv\n",
                encoding="utf-8",
            )
            with self.assertRaises(ValueError):
                read_summary(path)

    def test_round_trip_header_is_accepted(self):
        with tempfile.TemporaryDirectory() as td:
            path = Path(td) / "summary.eval.tsv"
            write_summary(path, [{"run_id": "x", "suite": "s"}])
            self.assertEqual(read_summary(path)[0]["run_id"], "x")

    def test_legacy_subset_header_is_still_read(self):
        with tempfile.TemporaryDirectory() as td:
            path = Path(td) / "legacy.summary.eval.tsv"
            path.write_text("run_id\tsuite\nx\ts\n", encoding="utf-8")
            row = read_summary(path)[0]
            self.assertEqual(row["run_id"], "x")
            self.assertEqual(row["eval_schema_version"], "legacy")


if __name__ == "__main__":
    unittest.main()
