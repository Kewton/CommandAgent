#!/usr/bin/env python3
"""Focused regression tests for the intent-aware capability-band aggregator."""

from __future__ import annotations

import io
import json
import tempfile
import unittest
from contextlib import redirect_stderr
from dataclasses import replace
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import band_aggregate as band


class FullMeaningLabelTests(unittest.TestCase):
    def test_every_profile_band_has_one_transparent_full_label(self) -> None:
        self.assertEqual(
            set(band.FULL_MEANING_LABELS),
            {"nextjs", "data", "fix", "investigation", "cli", "ingest"},
        )
        for profile, meaning in band.FULL_MEANING_LABELS.items():
            with self.subTest(profile=profile):
                self.assertIn("testimony", meaning.lower())
                self.assertEqual(
                    band.full_meaning_label(profile),
                    f"- Full meaning label: {meaning}",
                )


def data_record(
    *,
    family: str,
    set_id: str,
    assurance: str = "failed",
    final_acceptance: str = "not_checked",
    goal: str = "test goal",
    evidence_dir: Path | None = None,
) -> band.DataRunRecord:
    return band.DataRunRecord(
        set_id=set_id,
        record_dir=set_id,
        run_name=f"{family}-run",
        planner=band.DATA_PLANNER,
        executor="executor",
        preset="profile",
        goal=goal,
        family=family,
        final_acceptance=final_acceptance,
        assurance=assurance,
        failure_class="test_failure",
        duration_seconds=10,
        source="test",
        intent="create",
        evidence_dir=evidence_dir,
    )


class DataFamilyBandTests(unittest.TestCase):
    def test_goal_family_classification(self) -> None:
        self.assertEqual(
            band.classify_data_family("月次の売上合計・前月比（%）を計算する"),
            "timeseries",
        )
        self.assertEqual(
            band.classify_data_family("月次の売上合計と3ヶ月移動平均を計算する"),
            "timeseries",
        )
        self.assertEqual(
            band.classify_data_family("月次 × 地域 の売上集計と全体合計"),
            "aggregation",
        )
        self.assertEqual(band.classify_data_family("CSVを要約する"), "unknown")

    def test_family_rows_keep_unknown_in_denominator(self) -> None:
        records = [
            data_record(
                family="aggregation",
                set_id="uat-test0715-data-007",
                assurance="full",
                final_acceptance="full_success",
            ),
            data_record(
                family="aggregation",
                set_id="uat-test0715-data-007",
                assurance="static",
            ),
            data_record(family="timeseries", set_id="uat-test0716-data-009"),
            data_record(
                family="unknown",
                set_id="uat-test0716-data-010",
                assurance="partial",
            ),
        ]

        self.assertEqual(
            band.data_family_rows(records),
            [
                ["aggregation", "1", "1", "0", "2", "50%"],
                ["timeseries", "0", "0", "1", "1", "0%"],
                ["unknown", "0", "1", "0", "1", "0%"],
            ],
        )

    def test_family_specific_stable_windows(self) -> None:
        self.assertFalse(
            band.data_record_in_stable_window(
                data_record(family="aggregation", set_id="uat-test0715-data-006")
            )
        )
        self.assertTrue(
            band.data_record_in_stable_window(
                data_record(family="aggregation", set_id="uat-test0715-data-007")
            )
        )
        self.assertFalse(
            band.data_record_in_stable_window(
                data_record(family="timeseries", set_id="uat-test0716-data-008")
            )
        )
        self.assertTrue(
            band.data_record_in_stable_window(
                data_record(family="timeseries", set_id="uat-test0716-data-009")
            )
        )
        self.assertFalse(
            band.data_record_in_stable_window(
                data_record(family="unknown", set_id="uat-test9999-data-999")
            )
        )

    def test_full_row_without_evidence_still_aborts(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            record = data_record(
                family="aggregation",
                set_id="uat-test0715-data-007",
                assurance="full",
                final_acceptance="full_success",
                evidence_dir=Path(directory),
            )
            with self.assertRaisesRegex(AssertionError, "false-full evidence gap"):
                band.assert_full_data_evidence([record])

    def test_summary_reports_unknown_without_stable_assignment(self) -> None:
        records = [
            data_record(
                family="aggregation",
                set_id="uat-test0715-data-007",
                assurance="static",
                goal=band.DATA_AGGREGATION_GOAL,
            ),
            data_record(
                family="timeseries",
                set_id="uat-test0716-data-009",
                goal="前月比を計算する",
            ),
            data_record(
                family="unknown",
                set_id="uat-test0716-data-010",
                assurance="partial",
                goal="CSVを要約する",
            ),
        ]

        summary = band.build_data_summary(records, 3, 3, ["test-set"], 0)

        self.assertIn("| unknown | 0 | 1 | 0 | 1 | 0% |", summary)
        self.assertIn("| unknown | 0 | 0 | 0 | 0 | 0% |", summary)
        self.assertIn(
            "| uat-test0716-data-010 | unknown-run | CSVを要約する | test |", summary
        )
        self.assertIn(
            "| uat-test0716-data-010 | uat-test0716-data-010 | unknown-run | "
            "unknown | executor | profile | not_checked | partial | test_failure | "
            "10s | A |",
            summary,
        )


class IntentAxisTests(unittest.TestCase):
    def test_intent_resolution_precedence_and_legacy_default(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            events = Path(directory) / "events.jsonl"
            events.write_text(
                '{"event":"intent_resolved","value":"fix"}\n',
                encoding="utf-8",
            )
            self.assertEqual(
                band.resolve_intent(
                    metadata={"intent_resolved": {"value": "create"}},
                    row={},
                    events_path=events,
                    legacy_create=False,
                ),
                band.IntentResolution("create", "uat-meta"),
            )
            self.assertEqual(
                band.resolve_intent(
                    metadata=None,
                    row={},
                    events_path=events,
                    legacy_create=False,
                ),
                band.IntentResolution("fix", "events"),
            )
            self.assertEqual(
                band.resolve_intent(
                    metadata=None,
                    row={},
                    events_path=None,
                    legacy_create=True,
                ),
                band.IntentResolution("create", "legacy-default"),
            )
            self.assertEqual(
                band.resolve_intent(
                    metadata=None,
                    row={},
                    events_path=None,
                    legacy_create=False,
                ),
                band.IntentResolution("unknown", "unresolved"),
            )

    def test_conflicting_event_intents_stay_unknown(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            events = Path(directory) / "events.jsonl"
            events.write_text(
                '{"event":"intent_resolved","value":"create"}\n'
                '{"event":"intent_resolved","value":"fix"}\n',
                encoding="utf-8",
            )
            self.assertEqual(band.event_intent(events), (True, "unknown"))

    def test_investigation_event_intent_resolves(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            events = Path(directory) / "events.jsonl"
            events.write_text(
                '{"event":"intent_resolved","value":"investigate"}\n',
                encoding="utf-8",
            )
            self.assertEqual(band.event_intent(events), (True, "investigate"))

    def test_historical_data_rows_resolve_as_create(self) -> None:
        records, scanned_rows, _meta_rows, _sets = band.discover_data_records()
        self.assertEqual(len(records), scanned_rows)
        self.assertTrue(records)
        self.assertEqual({record.intent for record in records}, {"create"})


class EmptyAggregationTests(unittest.TestCase):
    def test_zero_rows_abort_with_every_set_filter_reason(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            missing_report = root / "uat-test9000-nextjs-001"
            wrong_report = root / "uat-test9000-nextjs-002"
            empty_aggregate = root / "uat-test9000-nextjs-003"
            missing_report.mkdir()
            wrong_report.mkdir()
            empty_aggregate.mkdir()
            (wrong_report / "uat-report.md").write_text(
                "# UAT report\n\nNo smoke table here.\n",
                encoding="utf-8",
            )
            (empty_aggregate / "aggregate.json").write_text(
                '{"results": []}\n',
                encoding="utf-8",
            )
            frozen_output = root / "band_summary.md"
            frozen_output.write_text("frozen\n", encoding="utf-8")
            stderr = io.StringIO()

            with (
                mock.patch.object(band, "RUNS_DIR", root),
                mock.patch.object(band, "OUTPUT", frozen_output),
                mock.patch.object(
                    band,
                    "parse_args",
                    return_value=SimpleNamespace(profile="nextjs"),
                ),
                redirect_stderr(stderr),
            ):
                self.assertEqual(band.main(), 1)

            error = stderr.getvalue()
            self.assertIn("aggregation result has 0 rows", error)
            self.assertIn("profile 'nextjs' adopted 0 sets", error)
            self.assertIn(
                "uat-test9000-nextjs-001: aggregate.json missing; "
                "uat-report.md missing",
                error,
            )
            self.assertIn(
                "uat-test9000-nextjs-002: aggregate.json missing; "
                "uat-report.md lacks required '## Smoke Result' heading",
                error,
            )
            self.assertIn(
                "uat-test9000-nextjs-003: aggregate.json: no usable result rows",
                error,
            )
            self.assertEqual(frozen_output.read_text(encoding="utf-8"), "frozen\n")


class NextjsProvenanceTests(unittest.TestCase):
    def test_generated_header_records_pre_migration_input_origin(self) -> None:
        summary = band.build_summary([], 0, [])

        self.assertIn(
            "> 出自注記: 本バンドの入力12セットは移行前計測に由来し、"
            "現リポジトリからの再生成は現在未対応"
            "（[analysis.md](band-f821-diff/analysis.md)参照）。",
            summary,
        )


class FixBandTests(unittest.TestCase):
    def test_fix_family_classification(self) -> None:
        self.assertEqual(
            band.classify_fix_family("fix_compile_001", ""),
            "compile_error_fix",
        )
        self.assertEqual(
            band.classify_fix_family(
                "run-001",
                'data-anvil-action="restart" の契約フックを修正しbuildも確認する',
            ),
            "contract_hook_fix",
        )
        self.assertEqual(
            band.classify_fix_family("run-001", "不具合を修正してください"),
            "unknown",
        )

    def test_repository_fix_window_and_full_evidence(self) -> None:
        records, scanned_sets = band.discover_fix_records()
        self.assertEqual(scanned_sets, list(band.FIX_WINDOW_SETS))
        window_a = [
            record for record in records if record.set_id in band.FIX_WINDOW_SETS
        ]
        window_b = [record for record in records if record.set_id == band.FIX_BENCH_SET]
        self.assertEqual(len(records), band.FIX_EXPECTED_RUNS)
        self.assertEqual(len(window_a), 24)
        self.assertEqual(len(window_b), 6)
        self.assertEqual(sum(record.intent == "fix" for record in records), 30)
        self.assertEqual(sum(bool(record.excluded_reason) for record in records), 3)
        self.assertEqual(band.assert_full_fix_evidence(records), 1)
        summary = band.build_fix_summary(records, scanned_sets, 1)
        self.assertIn(
            f"Baseline HEAD: `{band.FIX_WINDOW_B_BASELINE_HEAD}` (FIX-5)", summary
        )

        official = [record for record in records if not record.excluded_reason]
        self.assertEqual(
            band.fix_rate_rows(official),
            [
                [
                    "fix",
                    "compile_error_fix",
                    "gemma4:31b",
                    "1",
                    "4",
                    "5",
                    "20%",
                ],
                [
                    "fix",
                    "compile_error_fix",
                    "qwen3.6:35b-a3b-coding-nvfp4",
                    "0",
                    "7",
                    "7",
                    "0%",
                ],
                [
                    "fix",
                    "contract_hook_fix",
                    "gemma4:31b",
                    "0",
                    "7",
                    "7",
                    "0%",
                ],
                [
                    "fix",
                    "contract_hook_fix",
                    "qwen3.6:35b-a3b-coding-nvfp4",
                    "0",
                    "8",
                    "8",
                    "0%",
                ],
            ],
        )

    def test_full_fix_row_without_f_evidence_aborts(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            record = band.FixRunRecord(
                set_id="uat-test",
                run_name="fix_compile",
                event_run_id="event-run",
                fix_run_id="fix-run",
                intent="fix",
                intent_source="events",
                goal="build failure",
                family="compile_error_fix",
                executor="executor",
                final_acceptance="full_success",
                verdict="full",
                assurance="full",
                failure_class="",
                duration_seconds=1,
                source="test",
                evidence_dir=Path(directory),
            )
            with self.assertRaisesRegex(AssertionError, "missing fix adjudication"):
                band.assert_full_fix_evidence([record])


class InvestigationBandTests(unittest.TestCase):
    def test_repository_investigation_windows_and_evidence(self) -> None:
        records, scanned_sets = band.discover_investigation_records()
        self.assertEqual(scanned_sets, list(band.INVESTIGATION_WINDOW_SETS))
        self.assertEqual(len(records), 12)
        self.assertEqual(sum(record.i1_passed for record in records), 12)
        self.assertEqual(sum(record.i2_executed for record in records), 4)
        self.assertEqual(sum(record.claim_count for record in records), 17)
        self.assertEqual(sum(record.matched_claim_count for record in records), 0)
        self.assertEqual(sum(record.violation_count for record in records), 17)
        self.assertEqual(
            sum(record.claim_kind_counts["code_snippet"] for record in records),
            14,
        )
        self.assertEqual(
            sum(record.claim_kind_counts["error_quote"] for record in records),
            3,
        )
        self.assertTrue(all(record.assurance == "failed" for record in records))
        self.assertEqual(
            band.investigation_rate_rows(records),
            [
                ["pipe", "gemma4:31b", "0", "2", "2", "0%"],
                [
                    "pipe",
                    "qwen3.6:35b-a3b-coding-nvfp4",
                    "0",
                    "4",
                    "4",
                    "0%",
                ],
                ["schema", "gemma4:31b", "0", "2", "2", "0%"],
                [
                    "schema",
                    "qwen3.6:35b-a3b-coding-nvfp4",
                    "0",
                    "4",
                    "4",
                    "0%",
                ],
            ],
        )

        window_b = [
            record
            for record in records
            if record.set_id == band.INVESTIGATION_WINDOW_B_SET
        ]
        self.assertEqual(
            band.investigation_rate_rows(window_b),
            [
                ["pipe", "gemma4:31b", "0", "1", "1", "0%"],
                [
                    "pipe",
                    "qwen3.6:35b-a3b-coding-nvfp4",
                    "0",
                    "2",
                    "2",
                    "0%",
                ],
                ["schema", "gemma4:31b", "0", "1", "1", "0%"],
                [
                    "schema",
                    "qwen3.6:35b-a3b-coding-nvfp4",
                    "0",
                    "2",
                    "2",
                    "0%",
                ],
            ],
        )
        summary = band.build_investigation_summary(records, scanned_sets)
        self.assertIn(
            f"Baseline HEAD `{band.INVESTIGATION_WINDOW_B_BASELINE_HEAD}`",
            summary,
        )
        self.assertIn("all 12 runs were formally consumed", summary)
        self.assertIn("I2 rejected violations: `17`", summary)

    def test_adjudication_without_i2_evidence_aborts(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            evidence_dir = Path(directory)
            (evidence_dir / "investigation-run.json").write_text(
                json.dumps(
                    {
                        "intent": "investigate",
                        "requirement_id": "reproducer_fails",
                        "stage": "diagnosis",
                        "expected": "failure",
                        "epoch": 1,
                        "executed": True,
                        "outcome": "failure",
                    }
                ),
                encoding="utf-8",
            )
            events = [
                {"event": "intent_resolved", "value": "investigate"},
                {
                    "event": "investigation_plan_synthesized",
                    "profile": "data",
                    "phase_count": 3,
                },
                {
                    "event": "investigation_adjudicated",
                    "assurance_level": "full",
                    "assurance_reason": "",
                },
            ]
            with self.assertRaisesRegex(
                AssertionError, "adjudication exists without I2 evidence"
            ):
                band.validate_investigation_evidence(evidence_dir, events, "test/run")


class CliBandTests(unittest.TestCase):
    def test_repository_cli_settlement_windows_and_evidence_invariant(self) -> None:
        records, scanned_sets = band.discover_cli_records()

        self.assertEqual(
            scanned_sets,
            [
                band.CLI_LOCAL_SET,
                *band.CLI_ELEVATED_SETS,
                *band.CLI_PACK_SETS,
                band.CLI_LUNA_SET,
                band.CLI_DIRECTIVE_SET,
            ],
        )
        self.assertEqual(len(records), 56)
        self.assertEqual(band.assert_cli_invariants(records), 6)
        local = [record for record in records if record.set_id == band.CLI_LOCAL_SET]
        self.assertEqual(
            band.cli_rate_rows(local),
            [
                ["filter", "gemma4:31b", "0", "0", "1", "1", "0%"],
                [
                    "filter",
                    "qwen3.6:35b-a3b-coding-nvfp4",
                    "0",
                    "0",
                    "2",
                    "2",
                    "0%",
                ],
                ["stats", "gemma4:31b", "0", "0", "1", "1", "0%"],
                [
                    "stats",
                    "qwen3.6:35b-a3b-coding-nvfp4",
                    "0",
                    "0",
                    "2",
                    "2",
                    "0%",
                ],
            ],
        )
        window_b = [
            record for record in records if record.set_id == band.CLI_WINDOW_B_SET
        ]
        self.assertEqual(
            band.cli_rate_rows(window_b),
            [
                ["filter", "gemma4:31b-cloud", "0", "0", "3", "3", "0%"],
                ["stats", "gemma4:31b-cloud", "0", "0", "3", "3", "0%"],
            ],
        )
        self.assertEqual(sum(record.reached_checks for record in window_b), 2)
        pack_arm = [
            record for record in records if record.set_id in band.CLI_PACK_SETS
        ]
        self.assertEqual(
            band.cli_rate_rows(pack_arm),
            [
                ["filter", "gemma4:31b-cloud", "0", "0", "9", "9", "0%"],
                ["stats", "gemma4:31b-cloud", "0", "0", "9", "9", "0%"],
            ],
        )
        self.assertEqual(sum(record.reached_checks for record in pack_arm), 3)
        self.assertEqual(sum(record.pack_exposed for record in pack_arm), 3)
        self.assertEqual(
            {record.pack_label for record in pack_arm},
            {
                f"{band.CLI_PACK_ID} / {band.CLI_PACK_HASH}",
                f"{band.CLI_PACK_V1_1_ID} / {band.CLI_PACK_V1_1_HASH}",
            },
        )
        luna_arm = [record for record in records if record.set_id == band.CLI_LUNA_SET]
        self.assertEqual(
            band.cli_cost_rows(luna_arm),
            [
                [
                    "filter",
                    "gpt-5.6-luna",
                    "0",
                    "3",
                    "3",
                    "0",
                    "0",
                    "0",
                    "$0.000000",
                ],
                [
                    "stats",
                    "gpt-5.6-luna",
                    "0",
                    "3",
                    "3",
                    "0",
                    "0",
                    "0",
                    "$0.000000",
                ],
            ],
        )
        self.assertTrue(all(record.attribution == "machine" for record in luna_arm))
        summary = band.build_cli_summary(records, scanned_sets, 6)
        self.assertIn("- Window B full: `0/6` (0%)", summary)
        self.assertIn("- Window B runs reaching C checks: `2/6`", summary)
        self.assertIn("- All-history runs reaching C checks: `6/56`", summary)
        self.assertIn("- Reached-run C evidence sets verified: `6/6`", summary)
        self.assertIn("- Pack runs reaching C checks: `3/18`", summary)
        self.assertIn("- Pack renderer exposure: `3/18`", summary)
        self.assertIn("C3は9/9 violationのまま", summary)
        self.assertIn(
            f"{band.CLI_PACK_ID} / {band.CLI_PACK_HASH}",
            summary,
        )
        self.assertIn(
            f"{band.CLI_PACK_V1_1_ID} / {band.CLI_PACK_V1_1_HASH}",
            summary,
        )
        self.assertIn(
            "assist ceiling measured — 援助飽和・効果なし (live 3/3同一署名)",
            summary,
        )
        self.assertIn("testimony target 1/1 reached", summary)
        self.assertIn("static (profile_not_admitted)", summary)
        self.assertIn("completion写像欠落", summary)
        self.assertIn("C1〜C4 runtimeが未配線", summary)
        self.assertIn("calibration predecessor — 配線後・較正前", summary)
        self.assertIn("Directive round / hash", summary)
        self.assertIn("- Directive arm: `0/2` full", summary)
        self.assertIn(
            "round 1 / sha256:e868fada3d47b09d1a9226564e214e752d03a3a2da32b77f7addd08bb5850203",
            summary,
        )
        self.assertIn(
            "round 2 / sha256:55c180bb0fdc86eaa8b219f9aa7c872faae01c974e1d7ccce20ad01c708d2dc4",
            summary,
        )
        self.assertIn("cli_readme_structure:cli_invocation_missing", summary)
        self.assertIn("- Luna arm: `0/6` full; C checks reached `0/6`", summary)
        self.assertIn("calculated cost `$0.000000`", summary)
        self.assertIn("## OpenAI Luna arm with observed cost", summary)
        self.assertIn(
            "Chat Completions rejected function tools with reasoning_effort",
            summary,
        )

    def test_directive_round_is_a_separate_cli_configuration_axis(self) -> None:
        root = Path("/tmp/d3d-band-fixture")
        records = [
            band.CliRunRecord(
                set_id="round-0",
                run_name="run-0",
                family="filter",
                executor="gemma4:31b-cloud",
                harness_status="completed",
                product_exit=1,
                verdict="failed",
                assurance="failed",
                c1="pass",
                c2="pass",
                c3="fail",
                c4="pass",
                failure_class="claims_binding_violation",
                attribution="model",
                duration_seconds=1,
                evidence_dir=root,
            ),
            band.CliRunRecord(
                set_id=band.CLI_DIRECTIVE_SET,
                run_name="run-1",
                family="filter",
                executor="gemma4:31b-cloud",
                harness_status="completed",
                product_exit=0,
                verdict="full",
                assurance="full",
                c1="pass",
                c2="pass",
                c3="pass",
                c4="pass",
                failure_class="none",
                attribution="model",
                duration_seconds=1,
                evidence_dir=root,
                directive_round=1,
                directive_hash=f"sha256:{'a' * 64}",
            ),
        ]
        self.assertEqual(
            band.cli_rate_rows(records),
            [
                ["filter", "gemma4:31b-cloud", "0", "0", "1", "1", "0%"],
                ["filter", "gemma4:31b-cloud", "1", "1", "0", "1", "100%"],
            ],
        )

    def test_reached_run_requires_all_cli_evidence_files(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            evidence_dir = Path(directory)
            records, _ = band.discover_cli_records()
            source = records[0]
            reached = band.CliRunRecord(
                set_id=source.set_id,
                run_name=source.run_name,
                family=source.family,
                executor=source.executor,
                harness_status="completed",
                product_exit=0,
                verdict="full",
                assurance="full",
                c1="pass",
                c2="pass",
                c3="pass",
                c4="pass",
                failure_class="full",
                attribution="model",
                duration_seconds=1,
                evidence_dir=evidence_dir,
            )
            with self.assertRaisesRegex(
                AssertionError, "without all four evidence attestations"
            ):
                band.verify_cli_reached_evidence([reached])

            for name in band.CLI_EVIDENCE_FILES:
                evidence_dir.joinpath(name).write_text("{}\n", encoding="utf-8")
            self.assertEqual(band.verify_cli_reached_evidence([reached]), 1)

    def test_unreached_run_cannot_claim_non_static_assurance(self) -> None:
        records, _ = band.discover_cli_records()
        source = records[0]
        inconsistent = band.CliRunRecord(
            set_id=source.set_id,
            run_name=source.run_name,
            family=source.family,
            executor=source.executor,
            harness_status=source.harness_status,
            product_exit=source.product_exit,
            verdict=source.verdict,
            assurance="full",
            c1=source.c1,
            c2=source.c2,
            c3=source.c3,
            c4=source.c4,
            failure_class=source.failure_class,
            attribution=source.attribution,
            duration_seconds=source.duration_seconds,
            evidence_dir=source.evidence_dir,
        )
        with self.assertRaisesRegex(AssertionError, "no C checks but assurance=full"):
            band.assert_cli_invariants([inconsistent, *records[1:]])

    def test_window_b_markdown_is_mechanically_bound(self) -> None:
        records = band.discover_cli_window_b_records()

        self.assertEqual(len(records), 6)
        reached = [record for record in records if record.reached_checks]
        self.assertEqual(
            [record.run_name for record in reached],
            [
                "filter_cloud_001",
                "filter_cloud_003",
            ],
        )
        self.assertTrue(all(record.c1 == "pass" for record in reached))
        self.assertTrue(all(record.c2 == "pass" for record in reached))
        self.assertTrue(all(record.c3 == "fail" for record in reached))
        self.assertTrue(all(record.c4 == "pass" for record in reached))


class IngestBandTests(unittest.TestCase):
    def test_repository_ingest_settlement_windows_and_evidence_invariant(
        self,
    ) -> None:
        records, scanned_sets = band.discover_ingest_records()

        self.assertEqual(
            scanned_sets,
            [band.INGEST_LOCAL_ALIAS, *band.INGEST_ELEVATED_SETS],
        )
        self.assertEqual(len(records), 54)
        self.assertEqual(band.assert_ingest_invariants(records), 20)
        local = [
            record for record in records if record.set_id == band.INGEST_LOCAL_ALIAS
        ]
        self.assertEqual(
            band.ingest_rate_rows(local),
            [
                ["list", "gemma4:31b", "0", "1", "1", "0%"],
                [
                    "list",
                    "qwen3.6:35b-a3b-coding-nvfp4",
                    "0",
                    "2",
                    "2",
                    "0%",
                ],
                ["table", "gemma4:31b", "0", "1", "1", "0%"],
                [
                    "table",
                    "qwen3.6:35b-a3b-coding-nvfp4",
                    "0",
                    "2",
                    "2",
                    "0%",
                ],
            ],
        )
        window_b = [
            record
            for record in records
            if record.set_id == band.INGEST_WINDOW_B_SET
        ]
        self.assertEqual(
            band.ingest_rate_rows(window_b),
            [
                ["list", "gemma4:31b-cloud", "3", "0", "3", "100%"],
                ["table", "gemma4:31b-cloud", "1", "2", "3", "33.3%"],
            ],
        )
        summary = band.build_ingest_summary(records, scanned_sets, 20)
        self.assertIn("- Window B full-equivalent: `4/6` (66.7%)", summary)
        self.assertIn("- Window B machine-attributed terminals: `0/6`", summary)
        self.assertIn("- Reached-run N1-N5 evidence sets verified: `20/20`", summary)
        self.assertIn("相異なる成功commandをno-diff停滞", summary)
        self.assertIn("複合CSSのengine被覆gap", summary)
        self.assertIn("freeze済み正準candidate ID", summary)
        self.assertIn(
            f"`{band.INGEST_LOCAL_ALIAS}` → `{band.INGEST_LOCAL_SOURCE_SET}`",
            summary,
        )

    def test_reached_run_requires_complete_n_evidence_set(self) -> None:
        records, _ = band.discover_ingest_records()
        reached = next(record for record in records if record.reached_checks)
        incomplete = replace(reached, n5="not_reached")

        with self.assertRaisesRegex(AssertionError, "partial N1-N5 evidence set"):
            band.verify_ingest_reached_evidence([incomplete])

    def test_full_equivalent_requires_all_n_checks_to_pass(self) -> None:
        records, _ = band.discover_ingest_records()
        index = next(
            index
            for index, record in enumerate(records)
            if record.set_id == band.INGEST_WINDOW_B_SET and record.is_full
        )
        inconsistent = replace(records[index], n2="failed")
        mutated = [*records]
        mutated[index] = inconsistent

        with self.assertRaisesRegex(AssertionError, "false ingest full"):
            band.assert_ingest_invariants(mutated)


class CircleBandTests(unittest.TestCase):
    def test_repository_circle_denominator_and_exclusions(self) -> None:
        records, scanned_sets = band.discover_circle_records()

        self.assertEqual(scanned_sets, list(band.CIRCLE_WINDOW_SETS))
        self.assertEqual(len(records), 33)
        self.assertEqual(sum(bool(record.excluded_reason) for record in records), 30)
        official = [record for record in records if not record.excluded_reason]
        self.assertEqual(
            {record.set_id for record in official}, {band.CIRCLE_OFFICIAL_SET}
        )
        self.assertEqual(
            band.circle_rate_rows(records), [["elevated", "1", "2", "3", "33%"]]
        )
        self.assertEqual(
            {record.verdict for record in official}, {"circle_full", "circle_failed"}
        )
        summary = band.build_circle_summary(records, scanned_sets)
        self.assertIn("| elevated | 1 | 2 | 3 | 33% |", summary)
        self.assertIn("profile不伝播により無効（P1-a FAIL）", summary)
        self.assertIn("実行モード欠落により無効", summary)

    def test_missing_workflow_circle_aborts(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for set_id in band.CIRCLE_WINDOW_SETS:
                for run_number in range(1, 4):
                    run_dir = root / set_id / f"run{run_number}"
                    run_dir.mkdir(parents=True)
                    (run_dir / "workflow-events.jsonl").write_text(
                        '{"event":"workflow_adjudicated","verdict":"circle_failed",'
                        '"reason":"node_failed:investigate"}\n',
                        encoding="utf-8",
                    )
                    if not (set_id == band.CIRCLE_OFFICIAL_SET and run_number == 1):
                        (run_dir / "workflow-circle.json").write_text(
                            '{"verdict":"circle_failed",'
                            '"reason":"node_failed:investigate"}\n',
                            encoding="utf-8",
                        )
            with (
                mock.patch.object(band, "RUNS_DIR", root),
                self.assertRaisesRegex(AssertionError, "missing workflow-circle.json"),
            ):
                band.discover_circle_records()

    def test_missing_workflow_adjudication_aborts(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for set_id in band.CIRCLE_WINDOW_SETS:
                for run_number in range(1, 4):
                    run_dir = root / set_id / f"run{run_number}"
                    run_dir.mkdir(parents=True)
                    (run_dir / "workflow-circle.json").write_text(
                        '{"verdict":"circle_failed",'
                        '"reason":"node_failed:investigate"}\n',
                        encoding="utf-8",
                    )
                    events = (
                        ""
                        if (set_id == band.CIRCLE_WINDOW_SETS[0] and run_number == 1)
                        else '{"event":"workflow_adjudicated",'
                        '"verdict":"circle_failed",'
                        '"reason":"node_failed:investigate"}\n'
                    )
                    (run_dir / "workflow-events.jsonl").write_text(
                        events, encoding="utf-8"
                    )
            with (
                mock.patch.object(band, "RUNS_DIR", root),
                self.assertRaisesRegex(AssertionError, "workflow_adjudicated"),
            ):
                band.discover_circle_records()

    def test_zero_official_rows_abort_without_replacing_output(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "band_summary_circle.md"
            output.write_text("frozen\n", encoding="utf-8")
            excluded = band.CircleRunRecord(
                set_id="uat-test0722-circle-001",
                run_name="run1",
                arm="local",
                verdict="circle_failed",
                reason="node_failed:investigate",
                circle_path=Path("workflow-circle.json"),
                events_path=Path("workflow-events.jsonl"),
                excluded_reason="invalid",
            )
            stderr = io.StringIO()
            with (
                mock.patch.object(
                    band,
                    "discover_circle_records",
                    return_value=([excluded], [excluded.set_id]),
                ),
                mock.patch.object(band, "CIRCLE_OUTPUT", output),
                mock.patch.object(
                    band, "parse_args", return_value=SimpleNamespace(profile="circle")
                ),
                redirect_stderr(stderr),
            ):
                self.assertEqual(band.main(), 1)

            self.assertIn("aggregation result has 0 rows", stderr.getvalue())
            self.assertEqual(output.read_text(encoding="utf-8"), "frozen\n")


if __name__ == "__main__":
    unittest.main()
