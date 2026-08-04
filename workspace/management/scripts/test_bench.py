#!/usr/bin/env python3
"""Focused tests for the bench v0 UAT measurement harness."""

from __future__ import annotations

import hashlib
import json
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
    bon_series: str | None = None,
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
    bon_series_line = (
        f'bon_series = "{bon_series}"' if bon_series is not None else ""
    )
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
            {bon_series_line}

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


def bon_methodology(
    suite: bench.SuiteDefinition, full_count: int = 4, trial_count: int = 42
) -> dict[str, object]:
    estimate = full_count / trial_count
    lower, upper = bench.wilson_score_interval(full_count, trial_count, 0.95)
    posterior_alpha = full_count + 0.5
    posterior_beta = trial_count - full_count + 0.5
    probabilities = bench.beta_binomial_probabilities(
        len(suite.runs), posterior_alpha, posterior_beta
    )
    band_lower, band_upper = bench.shortest_contiguous_band(probabilities, 0.95)
    return {
        "baseline_rate": {
            "full_count": full_count,
            "trial_count": trial_count,
            "estimate": estimate,
            "sources": ["fixture:4/42"],
            "confidence_interval": {
                "method": "wilson_score",
                "confidence": 0.95,
                "lower": lower,
                "upper": upper,
            },
        },
        "predictive_distribution": {
            "model": "beta_binomial",
            "prior": {"name": "jeffreys", "alpha": 0.5, "beta": 0.5},
            "trials": len(suite.runs),
            "posterior": {
                "alpha": posterior_alpha,
                "beta": posterior_beta,
            },
            "probability_at_least_one_full": 1.0 - probabilities[0],
            "expected_full_count": len(suite.runs)
            * posterior_alpha
            / (posterior_alpha + posterior_beta),
            "full_count_band": {
                "method": "shortest_contiguous",
                "mass": 0.95,
                "lower": band_lower,
                "upper": band_upper,
            },
        },
    }


def bon_predeclaration_document(
    suite: bench.SuiteDefinition,
) -> dict[str, object]:
    return {
        "schema_version": bench.BON_PREDECLARATION_SCHEMA_VERSION,
        "recorded_at": "2026-08-04T00:00:00+09:00",
        "series_id": "test-bon-series",
        "execution_revision": "a" * 40,
        "suite_sha256": bench.sha256_file(suite.path),
        "binary_sha256": "b" * 64,
        **bon_methodology(suite),
    }


class SuiteAndCommandTests(unittest.TestCase):
    def test_parse_bundled_suites(self) -> None:
        dfix = bench.load_suite(SUITES_DIR / "dfix-synthesis.toml")
        investigation = bench.load_suite(SUITES_DIR / "investigation-data.toml")
        cli_create = bench.load_suite(SUITES_DIR / "cli-create.toml")
        cli_pack = bench.load_suite(
            SUITES_DIR / "cli-create-elevated-cli-assist.toml"
        )
        cli_luna = bench.load_suite(SUITES_DIR / "cli-create-luna.toml")
        cli_bon = bench.load_suite(SUITES_DIR / "cli-filter-bon0.toml")
        cli_elevated = bench.load_suite(SUITES_DIR / "cli-create-elevated.toml")
        gemma_negative = bench.load_suite(
            SUITES_DIR / "cli-gemma-negative-bon.toml"
        )
        nextjs_t1 = bench.load_suite(SUITES_DIR / "nextjs-t1.toml")
        breakout_local = bench.load_suite(
            SUITES_DIR / "nextjs-breakout-local-bon.toml"
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
        self.assertEqual(cli_luna.api, "responses")
        self.assertEqual(cli_luna.tool_protocol, "native")
        self.assertEqual(cli_bon.bon_series, "f-bon-v-cli-luna")
        self.assertEqual(
            gemma_negative.bon_series, "f-bon-v-cli-gemma-negative"
        )
        self.assertEqual(
            {run.executor for run in gemma_negative.runs}, {"gemma4:31b-cloud"}
        )
        self.assertEqual(len(gemma_negative.runs), 6)
        self.assertEqual(gemma_negative.goals, cli_elevated.goals)
        self.assertEqual(
            [run.goal_id for run in gemma_negative.runs],
            [run.goal_id for run in cli_elevated.runs],
        )
        for field in (
            "profile",
            "intent",
            "plan_preset",
            "workspace_mode",
            "context_budget",
            "planner_model",
            "planner_provider",
            "provider",
            "api",
            "tool_protocol",
        ):
            self.assertEqual(
                getattr(gemma_negative, field), getattr(cli_elevated, field)
            )
        self.assertEqual(
            breakout_local.bon_series, "f-bon-v-nextjs-breakout-local"
        )
        self.assertEqual(set(breakout_local.goals), {"breakout"})
        self.assertEqual(len(breakout_local.runs), 6)
        self.assertEqual(
            {run.executor for run in breakout_local.runs},
            {"qwen3.6:35b-a3b-coding-nvfp4"},
        )
        for field in (
            "profile",
            "intent",
            "plan_preset",
            "workspace_mode",
            "context_budget",
            "planner_model",
            "planner_provider",
            "provider",
            "api",
            "tool_protocol",
        ):
            self.assertEqual(
                getattr(breakout_local, field), getattr(nextjs_t1, field)
            )
        self.assertIn("--api", bench.build_command(cli_luna, cli_luna.runs[0]))
        self.assertIn("responses", bench.build_command(cli_luna, cli_luna.runs[0]))
        self.assertIn(
            "--tool-protocol",
            bench.build_command(cli_luna, cli_luna.runs[0]),
        )

    def test_tool_protocol_is_optional_and_strict(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            suite_path = write_source_less_suite(Path(directory))
            suite = bench.load_suite(suite_path)
            metadata = bench.new_metadata(
                suite, "source-less-campaign", "dry-run", Path(directory), {}, []
            )
            self.assertIsNone(suite.tool_protocol)
            self.assertNotIn("tool_protocol", metadata["suite"])
            self.assertNotIn("--tool-protocol", bench.build_command(suite, suite.runs[0]))

            contents = suite_path.read_text(encoding="utf-8").replace(
                'provider = "ollama"',
                'provider = "ollama"\ntool_protocol = "automatic"',
                1,
            )
            suite_path.write_text(contents, encoding="utf-8")
            with self.assertRaisesRegex(
                bench.BenchError,
                "suite.tool_protocol must be native or text when present",
            ):
                bench.load_suite(suite_path)

    def test_openai_api_is_optional_and_strict(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            suite_path = write_source_less_suite(Path(directory))
            suite = bench.load_suite(suite_path)
            metadata = bench.new_metadata(
                suite, "source-less-campaign", "dry-run", Path(directory), {}, []
            )
            self.assertIsNone(suite.api)
            self.assertNotIn("api", metadata["suite"])
            self.assertNotIn("--api", bench.build_command(suite, suite.runs[0]))

            contents = suite_path.read_text(encoding="utf-8").replace(
                'provider = "ollama"',
                'provider = "openai"\napi = "automatic"',
                1,
            )
            suite_path.write_text(contents, encoding="utf-8")
            with self.assertRaisesRegex(
                bench.BenchError,
                "suite.api must be chat_completions or responses when present",
            ):
                bench.load_suite(suite_path)

    def test_bon_predeclaration_requires_binary_sha256(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            suite = bench.load_suite(
                write_source_less_suite(root, bon_series="test-bon-series")
            )
            predeclaration = root / "predeclaration.json"
            document = bon_predeclaration_document(suite)
            del document["binary_sha256"]
            predeclaration.write_text(json.dumps(document), encoding="utf-8")

            with self.assertRaisesRegex(
                bench.BenchError,
                "binary_sha256 must be a non-empty string",
            ):
                bench.load_bon_predeclaration(predeclaration, suite)

    def test_bon_predeclaration_records_valid_series_pin(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            suite = bench.load_suite(
                write_source_less_suite(root, bon_series="test-bon-series")
            )
            predeclaration = root / "predeclaration.json"
            document = bon_predeclaration_document(suite)
            predeclaration.write_text(json.dumps(document), encoding="utf-8")

            pin = bench.load_bon_predeclaration(predeclaration, suite)

            self.assertEqual(pin.series_id, "test-bon-series")
            self.assertEqual(pin.binary_sha256, "b" * 64)
            self.assertEqual(pin.baseline_rate["trial_count"], 42)
            self.assertEqual(
                pin.predictive_distribution["model"], "beta_binomial"
            )

    def test_bon_predeclaration_rejects_point_binomial_declaration(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            suite = bench.load_suite(
                write_source_less_suite(root, bon_series="test-bon-series")
            )
            document = bon_predeclaration_document(suite)
            document["expected_full_probability"] = 0.17
            predeclaration = root / "predeclaration.json"
            predeclaration.write_text(json.dumps(document), encoding="utf-8")

            with self.assertRaisesRegex(
                bench.BenchError, "point-binomial declaration is forbidden"
            ):
                bench.load_bon_predeclaration(predeclaration, suite)

    def test_bon_predeclaration_requires_wilson_confidence_interval(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            suite = bench.load_suite(
                write_source_less_suite(root, bon_series="test-bon-series")
            )
            document = bon_predeclaration_document(suite)
            del document["baseline_rate"]["confidence_interval"]
            predeclaration = root / "predeclaration.json"
            predeclaration.write_text(json.dumps(document), encoding="utf-8")

            with self.assertRaisesRegex(
                bench.BenchError, "confidence_interval must be an object"
            ):
                bench.load_bon_predeclaration(predeclaration, suite)

    def test_bon_predeclaration_recomputes_beta_binomial_band(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            suite = bench.load_suite(
                write_source_less_suite(root, bon_series="test-bon-series")
            )
            document = bon_predeclaration_document(suite)
            document["predictive_distribution"]["full_count_band"]["upper"] = 99
            predeclaration = root / "predeclaration.json"
            predeclaration.write_text(json.dumps(document), encoding="utf-8")

            with self.assertRaisesRegex(
                bench.BenchError, "full_count_band does not match Beta-binomial"
            ):
                bench.load_bon_predeclaration(predeclaration, suite)

    def test_bon_predeclaration_requires_ninety_five_percent_band(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            suite = bench.load_suite(
                write_source_less_suite(root, bon_series="test-bon-series")
            )
            document = bon_predeclaration_document(suite)
            document["predictive_distribution"]["full_count_band"]["mass"] = 0.5
            predeclaration = root / "predeclaration.json"
            predeclaration.write_text(json.dumps(document), encoding="utf-8")

            with self.assertRaisesRegex(
                bench.BenchError, "full_count_band.mass must be 0.95"
            ):
                bench.load_bon_predeclaration(predeclaration, suite)

    def test_bon_suite_requires_predeclaration_before_workspace_allocation(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory) / "workspace"

            exit_code = bench.main(
                [
                    "run",
                    "--suite",
                    str(SUITES_DIR / "cli-filter-bon0.toml"),
                    "--workspace-root",
                    str(workspace),
                ]
            )

            self.assertEqual(exit_code, 2)
            self.assertFalse(workspace.exists())

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


class PreflightPinTests(unittest.TestCase):
    def test_binary_pin_mismatch_fails_before_install_or_product(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / "target/release/commandagent"
            binary.parent.mkdir(parents=True)
            binary.write_bytes(b"deterministic fixture binary")
            predeclaration = root / "predeclaration.json"
            predeclaration.write_text("{}", encoding="utf-8")
            pin = bench.BonSeriesPredeclaration(
                path=predeclaration,
                series_id="test-bon-series",
                execution_revision="a" * 40,
                suite_sha256="b" * 64,
                binary_sha256="0" * 64,
                baseline_rate={"full_count": 0, "trial_count": 1},
                predictive_distribution={"model": "beta_binomial"},
            )
            commands: list[tuple[str, ...]] = []

            def capture(argv: list[str], _cwd: Path) -> dict[str, object]:
                commands.append(tuple(argv))
                stdout = ""
                if argv[:2] == ["git", "rev-parse"]:
                    stdout = "a" * 40 + "\n"
                elif argv[:2] == ["git", "log"]:
                    stdout = "aaaaaaaa fixture\n"
                return {
                    "command_argv": argv,
                    "command": bench.format_command(argv),
                    "start_epoch": 1,
                    "end_epoch": 1,
                    "exit_code": 0,
                    "stdout_tail": stdout,
                    "stderr_tail": "",
                }

            with (
                mock.patch.object(bench, "_run_capture", side_effect=capture),
                self.assertRaisesRegex(
                    bench.BenchError,
                    "BoN series binary SHA-256 pin mismatch",
                ),
            ):
                bench.perform_preflight(
                    root,
                    min_head=None,
                    skip_suite_tests=True,
                    bon_predeclaration=pin,
                )

            self.assertIn(("cargo", "build", "--release"), commands)
            self.assertFalse(any(command[0] == "install" for command in commands))
            self.assertFalse(any(command[0] == "commandagent" for command in commands))


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

    def test_modern_openai_key_shape_is_detected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "event.jsonl").write_text(
                "sk-proj_example-key-material_123456789\n", encoding="utf-8"
            )

            result = bench.scrub_path(root)

            self.assertFalse(result.ok)
            self.assertTrue(
                any(item["kind"] == "secret_value" for item in result.findings)
            )
            self.assertNotIn("sk-proj_example-key-material_123456789", str(result.findings))

    def test_scrub_allow_is_transferred_and_suppresses_matching_finding(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "note.txt").write_text("sk-abcdefghijklmnopq\n", encoding="utf-8")
            result = bench.scrub_path(root, ({"pattern": "sk-", "reason": "fixture"},))
            self.assertTrue(result.ok)
            self.assertEqual(result.allows[0]["reason"], "fixture")


if __name__ == "__main__":
    unittest.main()
