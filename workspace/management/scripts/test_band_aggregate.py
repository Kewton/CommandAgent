#!/usr/bin/env python3
"""Focused regression tests for the intent-aware capability-band aggregator."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

import band_aggregate as band


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

    def test_historical_data_rows_resolve_as_create(self) -> None:
        records, scanned_rows, _meta_rows, _sets = band.discover_data_records()
        self.assertEqual(len(records), scanned_rows)
        self.assertTrue(records)
        self.assertEqual({record.intent for record in records}, {"create"})


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
        self.assertEqual(len(records), 24)
        self.assertEqual(sum(record.intent == "fix" for record in records), 24)
        self.assertEqual(sum(bool(record.excluded_reason) for record in records), 2)
        self.assertEqual(band.assert_full_fix_evidence(records), 1)

        official = [record for record in records if not record.excluded_reason]
        self.assertEqual(
            band.fix_rate_rows(official),
            [
                [
                    "fix",
                    "compile_error_fix",
                    "gemma4:31b",
                    "1",
                    "3",
                    "4",
                    "25%",
                ],
                [
                    "fix",
                    "compile_error_fix",
                    "qwen3.6:35b-a3b-coding-nvfp4",
                    "0",
                    "5",
                    "5",
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
                    "6",
                    "6",
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


if __name__ == "__main__":
    unittest.main()
