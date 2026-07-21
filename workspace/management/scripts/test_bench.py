#!/usr/bin/env python3
"""Focused tests for the bench v0 UAT measurement harness."""

from __future__ import annotations

import hashlib
import shlex
import tempfile
import textwrap
import unittest
from pathlib import Path
from unittest import mock

import bench


SUITES_DIR = Path(__file__).resolve().parents[1] / "bench" / "suites"


def digest(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def write_test_suite(
    root: Path,
    *,
    expected_sha256: str | None = None,
    precheck_pattern: str = "ValueError",
) -> bench.SuiteDefinition:
    source_dir = root / "fixtures" / "source"
    source_dir.mkdir(parents=True)
    payload = b"qualified input\n"
    (source_dir / "input.txt").write_bytes(payload)
    expected = expected_sha256 or digest(payload)
    suite_path = root / "test-suite.toml"
    suite_path.write_text(
        textwrap.dedent(
            f"""
            [suite]
            id = "test-suite"
            profile = "data"
            intent = "fix"
            plan_preset = "default"
            context_budget = 65536
            planner_model = "planner"
            planner_provider = "ollama"
            provider = "ollama"

            [goals]
            pipe = "repair the pipeline"

            [[sources]]
            set = "pipe-a"
            path = "fixtures/source"
            copy = ["input.txt"]
            input_sha256 = {{ "input.txt" = "{expected}" }}
            precheck_cmd = "python3 -c \\\"raise ValueError('broken')\\\""
            precheck_expect = "nonzero_exit"
            precheck_pattern = "{precheck_pattern}"

            [[runs]]
            name = "pipe_qwen_001"
            set = "pipe-a"
            goal = "pipe"
            executor = "executor"
            """
        ).lstrip(),
        encoding="utf-8",
    )
    return bench.load_suite(suite_path)


class SuiteAndCommandTests(unittest.TestCase):
    def test_parse_bundled_suites(self) -> None:
        dfix = bench.load_suite(SUITES_DIR / "dfix-synthesis.toml")
        investigation = bench.load_suite(SUITES_DIR / "investigation-data.toml")

        self.assertEqual(dfix.suite_id, "dfix-synthesis")
        self.assertEqual(dfix.plan_preset, "default")
        self.assertEqual(len(dfix.sources), 4)
        self.assertEqual(len(dfix.runs), 6)
        self.assertEqual(investigation.intent, "investigate")
        self.assertEqual(len(investigation.runs), 6)

    def test_command_matches_required_argv_without_wrapper(self) -> None:
        suite = bench.load_suite(SUITES_DIR / "dfix-synthesis.toml")
        run = suite.runs[0]
        command = bench.build_command(suite, run)

        self.assertEqual(
            command,
            [
                "commandagent",
                "--yes",
                "--intent",
                "fix",
                "--context-budget",
                "65536",
                "--model",
                "qwen3.6:35b-a3b-coding-nvfp4",
                "--provider",
                "ollama",
                "--planner-model",
                "qwen3.6:27b-coding-nvfp4",
                "--planner-provider",
                "ollama",
                "--ultra-plan-run",
                "--profile",
                "data",
                suite.goals["pipe"],
            ],
        )
        self.assertEqual(shlex.split(bench.format_command(command)), command)
        self.assertFalse(bench.WRAPPER_TOKENS.intersection(command))
        self.assertNotIn("--plan-preset", command)

    def test_wrapper_token_is_rejected_lexically(self) -> None:
        with self.assertRaisesRegex(bench.BenchError, "start directly"):
            bench.verify_unwrapped_command(["timeout", "commandagent"])
        with self.assertRaisesRegex(bench.BenchError, "wrapper token"):
            bench.verify_unwrapped_command(["commandagent", "nice"])


class ProcurementTests(unittest.TestCase):
    def test_sha_mismatch_blocks_run(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            suite = write_test_suite(root, expected_sha256="0" * 64)
            result = bench.procure_run(
                suite, suite.runs[0], root, root / "work" / "run"
            )

            self.assertFalse(result.ok)
            self.assertIn("SHA-256 mismatch", result.reason or "")
            self.assertIsNone(result.precheck)

    def test_precheck_mismatch_blocks_run(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            suite = write_test_suite(root, precheck_pattern="AssertionError")
            result = bench.procure_run(
                suite, suite.runs[0], root, root / "work" / "run"
            )

            self.assertFalse(result.ok)
            self.assertIn("precheck mismatch", result.reason or "")
            self.assertEqual(result.precheck["exit_code"], 1)
            self.assertFalse(result.precheck["pattern_matches"])


class ExecutionPolicyTests(unittest.TestCase):
    def test_resume_skips_completed_run(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            suite = write_test_suite(root)
            campaign = root / "campaign"
            campaign.mkdir()
            metadata = bench.new_metadata(
                suite, "test-suite-campaign", "run", root, {}, []
            )
            metadata["runs"][0]["status"] = "completed"
            bench.write_metadata(campaign / "uat-meta.json", metadata)

            with (
                mock.patch.object(bench, "procure_run") as procure,
                mock.patch.object(bench, "run_product") as product,
            ):
                bench.process_runs(
                    suite,
                    root,
                    campaign,
                    metadata,
                    dry_run=False,
                    resume=True,
                )

            procure.assert_not_called()
            product.assert_not_called()
            self.assertEqual(metadata["runs"][0]["status"], "completed")

    def test_dry_run_procures_but_never_executes_product(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            suite = write_test_suite(root)
            campaign = root / "campaign"
            campaign.mkdir()
            metadata = bench.new_metadata(
                suite, "test-suite-dry-run", "dry-run", root, {}, []
            )
            bench.write_metadata(campaign / "uat-meta.json", metadata)

            with mock.patch.object(bench, "run_product") as product:
                bench.process_runs(
                    suite,
                    root,
                    campaign,
                    metadata,
                    dry_run=True,
                    resume=False,
                )

            product.assert_not_called()
            self.assertEqual(metadata["runs"][0]["status"], "dry-run-ready")
            self.assertTrue(
                (campaign / "artifacts" / "pipe_qwen_001" / "input.txt").is_file()
            )


class ScrubTests(unittest.TestCase):
    def test_name_only_is_allowed_and_value_is_masked(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "issue-analysis.md").write_text(
                "GEMINI_API_KEY is not set\napi_key: Ab1234567890abcdef\n",
                encoding="utf-8",
            )
            result = bench.scrub_path(root)
            self.assertFalse(result.ok)
            self.assertEqual(result.findings[0]["detail"], "Ab…(18 chars)")
            self.assertNotIn("Ab1234567890abcdef", str(result.findings))

    def test_real_value_patterns_and_dangerous_paths_fail(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / ".env").write_text("x=1", encoding="utf-8")
            (root / "token.txt").write_text("sk-abcdefghijklmnopq\n", encoding="utf-8")
            result = bench.scrub_path(root)
            self.assertFalse(result.ok)
            self.assertTrue(any(item["kind"] == "dangerous_file" for item in result.findings))
            self.assertTrue(any(item["kind"] == "secret_value" for item in result.findings))

    def test_scrub_allow_is_transferred_and_suppresses_matching_finding(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "note.txt").write_text("sk-abcdefghijklmnopq\n", encoding="utf-8")
            result = bench.scrub_path(root, ({"pattern": "sk-", "reason": "fixture"},))
            self.assertTrue(result.ok)
            self.assertEqual(result.allows[0]["reason"], "fixture")


if __name__ == "__main__":
    unittest.main()
