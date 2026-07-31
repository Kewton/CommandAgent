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
    pack_id: str | None = None,
    pack_version: str | None = None,
    pack_hash: str | None = None,
) -> bench.SuiteDefinition:
    source_dir = root / "fixtures" / "source"
    source_dir.mkdir(parents=True)
    payload = b"qualified input\n"
    (source_dir / "input.txt").write_bytes(payload)
    expected = expected_sha256 or digest(payload)
    pack_lines = "\n".join(
        line
        for line in (
            f'pack_id = "{pack_id}"' if pack_id is not None else "",
            f'pack_version = "{pack_version}"' if pack_version is not None else "",
            f'pack_hash = "{pack_hash}"' if pack_hash is not None else "",
        )
        if line
    )
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
            {pack_lines}

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


def write_source_less_suite(
    root: Path,
    *,
    workspace_mode: str | None = "empty",
    with_sources: bool = False,
    with_run_set: bool = False,
) -> Path:
    mode_line = (
        f'workspace_mode = "{workspace_mode}"' if workspace_mode is not None else ""
    )
    sources = (
        """
        [[sources]]
        set = "unexpected"
        """
        if with_sources
        else ""
    )
    run_set = 'set = "unexpected"' if with_run_set else ""
    suite_path = root / "source-less-suite.toml"
    suite_path.write_text(
        textwrap.dedent(
            f"""
            [suite]
            id = "source-less-suite"
            profile = "cli"
            intent = "create"
            plan_preset = "default"
            context_budget = 65536
            planner_model = "planner"
            planner_provider = "ollama"
            provider = "ollama"
            {mode_line}

            [goals]
            cli = "create a cli"

            {sources}
            [[runs]]
            name = "cli_qwen_001"
            {run_set}
            goal = "cli"
            executor = "executor"
            """
        ).lstrip(),
        encoding="utf-8",
    )
    return suite_path


class SuiteAndCommandTests(unittest.TestCase):
    def test_parse_bundled_suites(self) -> None:
        dfix = bench.load_suite(SUITES_DIR / "dfix-synthesis.toml")
        investigation = bench.load_suite(SUITES_DIR / "investigation-data.toml")
        cli_create = bench.load_suite(SUITES_DIR / "cli-create.toml")
        cli_pack = bench.load_suite(
            SUITES_DIR / "cli-create-elevated-cli-assist.toml"
        )

        self.assertEqual(dfix.suite_id, "dfix-synthesis")
        self.assertEqual(dfix.workspace_mode, "sourced")
        self.assertEqual(dfix.plan_preset, "default")
        self.assertEqual(len(dfix.sources), 4)
        self.assertEqual(len(dfix.runs), 6)
        self.assertEqual(investigation.intent, "investigate")
        self.assertEqual(len(investigation.runs), 6)
        self.assertEqual(cli_create.workspace_mode, "empty")
        self.assertEqual(cli_create.sources, ())
        self.assertTrue(all(run.set_id is None for run in cli_create.runs))
        self.assertEqual(cli_pack.pack_id, "cli-assist")
        self.assertEqual(cli_pack.pack_version, "1.0.0")
        self.assertEqual(len(cli_pack.runs), 6)

    def test_empty_workspace_rejects_sources(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            suite_path = write_source_less_suite(
                Path(directory), with_sources=True
            )
            with self.assertRaisesRegex(
                bench.BenchError,
                r"workspace_mode empty may not define \[\[sources\]\] tables",
            ):
                bench.load_suite(suite_path)

    def test_empty_workspace_rejects_run_set(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            suite_path = write_source_less_suite(
                Path(directory), with_run_set=True
            )
            with self.assertRaisesRegex(
                bench.BenchError,
                r"runs\[0\]\.set may not be defined for workspace_mode empty",
            ):
                bench.load_suite(suite_path)

    def test_sourced_workspace_still_requires_sources_with_original_error(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            suite_path = write_source_less_suite(
                Path(directory), workspace_mode=None
            )
            with self.assertRaisesRegex(
                bench.BenchError,
                r"^suite must define at least one \[\[sources\]\] table$",
            ):
                bench.load_suite(suite_path)

    def test_sourced_default_keeps_legacy_metadata_shape(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            suite = write_test_suite(root)
            metadata = bench.new_metadata(
                suite, "test-suite-campaign", "dry-run", root, {}, []
            )

            self.assertEqual(suite.workspace_mode, "sourced")
            self.assertNotIn("workspace_mode", metadata["suite"])
            self.assertNotIn("pack", metadata["suite"])
            self.assertEqual(metadata["runs"][0]["set"], "pipe-a")
            self.assertEqual(
                metadata["runs"][0]["input_sha256_expected"],
                suite.sources[0].input_sha256,
            )

    def test_pack_identity_and_hash_are_recorded_in_suite_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pack = root / "packs" / "ingest-default" / "1.0.0"
            pack.mkdir(parents=True)
            (pack / "assist.yaml").write_text(
                "schema_version: commandagent.pack.assist/v0\n",
                encoding="utf-8",
            )
            expected_hash = bench.exact_pack_hash(pack)
            (pack / "pack.sha256").write_text(
                expected_hash + "\n", encoding="utf-8"
            )
            suite = write_test_suite(
                root,
                pack_id="ingest-default",
                pack_version="1.0.0",
                pack_hash=expected_hash,
            )
            metadata = bench.new_metadata(
                suite, "test-suite-campaign", "dry-run", root, {}, []
            )

            self.assertEqual(
                metadata["suite"]["pack"],
                {
                    "id": "ingest-default",
                    "version": "1.0.0",
                    "hash": expected_hash,
                    "assist_present": True,
                    "eval_present": False,
                    "assist_schema_version": "commandagent.pack.assist/v0",
                    "eval_schema_version": None,
                },
            )
            self.assertEqual(
                metadata["runs"][0]["pack"], metadata["suite"]["pack"]
            )
            self.assertEqual(
                bench._pack_product_environment(suite, root),
                {
                    "COMMANDAGENT_PACK_DIRECTORY": str(pack.resolve()),
                    "COMMANDAGENT_PACK_ID": "ingest-default",
                    "COMMANDAGENT_PACK_VERSION": "1.0.0",
                    "COMMANDAGENT_PACK_HASH": expected_hash,
                },
            )

    def test_pack_identity_and_hash_must_be_declared_together(self) -> None:
        with tempfile.TemporaryDirectory() as directory, self.assertRaisesRegex(
            bench.BenchError, "pack_id, suite.pack_version, and suite.pack_hash"
        ):
            write_test_suite(Path(directory), pack_id="ingest-default")

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
    def test_empty_procurement_creates_and_records_virgin_workspace(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            suite = bench.load_suite(write_source_less_suite(root))
            run_dir = root / "work" / "run"
            result = bench.procure_run(suite, suite.runs[0], root, run_dir)
            record: dict[str, object] = {}
            bench._record_procurement(record, result)

            self.assertTrue(result.ok)
            self.assertTrue(run_dir.is_dir())
            self.assertEqual(list(run_dir.iterdir()), [])
            self.assertEqual(
                record["workspace_integrity"],
                {
                    "workspace_mode": "empty",
                    "created": True,
                    "checked": True,
                    "empty": True,
                    "entry_count": 0,
                    "entries": [],
                },
            )

    def test_empty_procurement_rejects_contamination(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            suite = bench.load_suite(write_source_less_suite(root))
            with mock.patch.object(
                bench, "_empty_workspace_entries", return_value=("foreign.txt",)
            ):
                result = bench.procure_run(
                    suite, suite.runs[0], root, root / "work" / "run"
                )

            self.assertFalse(result.ok)
            self.assertIn("empty workspace integrity check failed", result.reason or "")
            self.assertEqual(result.workspace_integrity["entry_count"], 1)
            self.assertFalse(result.workspace_integrity["empty"])

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
    def test_source_less_archive_excludes_reproducible_derivatives(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            workspace = root / "workspace"
            artifact = root / "artifact"
            workspace.mkdir()
            artifact.mkdir()
            (workspace / "README.md").write_text("deliverable")
            for name in bench.DERIVED_ARTIFACT_DIRS:
                (workspace / name).mkdir()
                (workspace / name / "generated.txt").write_text("generated")
                (artifact / name).mkdir()
                (artifact / name / "stale.txt").write_text("stale")

            bench.archive_run(None, workspace, artifact)

            self.assertEqual((artifact / "README.md").read_text(), "deliverable")
            for name in bench.DERIVED_ARTIFACT_DIRS:
                self.assertFalse((artifact / name).exists())

    def test_artifact_archive_is_idempotent_with_dependency_symlinks(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "workspace/node_modules/.bin"
            source.mkdir(parents=True)
            (root / "workspace/node_modules/tool.js").write_text("tool")
            (source / "tool").symlink_to("../tool.js")
            artifact = root / "artifact/node_modules"

            bench._copy_to_artifact(root / "workspace/node_modules", artifact)
            bench._copy_to_artifact(root / "workspace/node_modules", artifact)

            self.assertTrue((artifact / ".bin/tool").is_symlink())
            self.assertEqual((artifact / ".bin/tool").read_text(), "tool")

    def test_resume_recovers_recorded_product_terminal_without_rerun(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            suite = bench.load_suite(write_source_less_suite(root))
            campaign = root / "campaign"
            run_dir = campaign / "workspaces" / "cli_qwen_001"
            run_dir.mkdir(parents=True)
            (run_dir / "uat-console.log").write_text(
                "start_epoch: 100\ncommand: commandagent\nend_epoch: 109\n"
                "product_exit: 0\n--- stdout tail ---\ncomplete\n"
                "--- stderr tail ---\n",
                encoding="utf-8",
            )
            metadata = bench.new_metadata(
                suite, "source-less-campaign", "run", root, {}, []
            )
            metadata["runs"][0]["status"] = "running"

            bench.normalize_interrupted_runs(
                suite, campaign, metadata, campaign / "uat-meta.json"
            )

            record = metadata["runs"][0]
            self.assertEqual(record["status"], "completed")
            self.assertEqual(record["product_exit"], 0)
            self.assertEqual(record["duration_seconds"], 9)
            self.assertTrue(record["resume_recovered_product_terminal"])
            self.assertTrue(
                (campaign / "artifacts/cli_qwen_001/acceptance-sheet.md").is_file()
            )

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
            self.assertTrue(
                (
                    campaign / "artifacts" / "pipe_qwen_001" / "acceptance-sheet.md"
                ).is_file()
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
            self.assertTrue(
                any(item["kind"] == "dangerous_file" for item in result.findings)
            )
            self.assertTrue(
                any(item["kind"] == "secret_value" for item in result.findings)
            )

    def test_scrub_allow_is_transferred_and_suppresses_matching_finding(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "note.txt").write_text("sk-abcdefghijklmnopq\n", encoding="utf-8")
            result = bench.scrub_path(root, ({"pattern": "sk-", "reason": "fixture"},))
            self.assertTrue(result.ok)
            self.assertEqual(result.allows[0]["reason"], "fixture")


if __name__ == "__main__":
    unittest.main()
