import json
import shutil
import tempfile
import unittest
from pathlib import Path

import calibration_corpus

MEASURED_ELEV_003 = (
    Path(__file__).resolve().parents[3]
    / "tests/corpus/apps/test0725_cli_elev_003/fixtures"
)


class CalibrationCorpusTests(unittest.TestCase):
    def test_tool_parse_failure_envelope_becomes_calibration_material(self):
        with tempfile.TemporaryDirectory() as tmp:
            campaign = Path(tmp) / "campaign"
            evidence = campaign / "artifacts/stats_luna_001/evidence"
            evidence.mkdir(parents=True)
            (evidence / "tool-parse-failure-001.json").write_text(
                json.dumps(
                    {
                        "model": "gpt-5.6-luna",
                        "protocol": "text",
                        "failure_kind": "json_trailing",
                        "parse_error": "trailing characters at line 1 column 121",
                        "raw_excerpt": {"text": "{...} trailing", "max_bytes": 512},
                        "phase": "create-sample-data",
                        "claims": [
                            {
                                "claim": "json_trailing",
                                "observation": {
                                    "model": "gpt-5.6-luna",
                                    "protocol": "text",
                                    "failure_kind": "json_trailing",
                                    "parse_error": "trailing characters at line 1 column 121",
                                    "raw_excerpt": {
                                        "text": "{...} trailing",
                                        "max_bytes": 512,
                                    },
                                    "phase": "create-sample-data",
                                },
                            }
                        ],
                        "evidence_envelope": {
                            "envelope_version": 1,
                            "family": "tool_parse",
                            "kind": "tool_parse_failure",
                            "epoch": 123,
                            "claims": [
                                {
                                    "index": 0,
                                    "label": "json_trailing",
                                    "judgement": "observed",
                                    "observation": {
                                        "model": "gpt-5.6-luna",
                                        "protocol": "text",
                                        "failure_kind": "json_trailing",
                                        "parse_error": "trailing characters at line 1 column 121",
                                        "raw_excerpt": {
                                            "text": "{...} trailing",
                                            "max_bytes": 512,
                                        },
                                        "phase": "create-sample-data",
                                    },
                                    "source_ref": None,
                                    "direction": None,
                                }
                            ],
                            "nearest_miss": [],
                            "source_refs": [],
                        },
                    }
                )
            )

            rows = list(calibration_corpus.records(campaign))

            self.assertEqual(len(rows), 1)
            self.assertEqual(rows[0]["kind"], "tool_parse")
            self.assertEqual(rows[0]["judgement"], "observed")
            self.assertEqual(rows[0]["claim"], "json_trailing")
            self.assertEqual(
                rows[0]["observation"]["raw_excerpt"]["text"],
                "{...} trailing",
            )

    def test_ingest_envelope_nearest_miss_uses_the_common_reader(self):
        with tempfile.TemporaryDirectory() as tmp:
            campaign = Path(tmp) / "campaign"
            evidence = campaign / "artifacts/ingest_001/evidence"
            evidence.mkdir(parents=True)
            (evidence / "source-binding.json").write_text(
                json.dumps(
                    {
                        "capability_id": "ingest_source_binding",
                        "evidence_envelope": {
                            "envelope_version": 1,
                            "family": "N",
                            "kind": "source_binding",
                            "epoch": 123,
                            "claims": [
                                {
                                    "index": 0,
                                    "label": "date",
                                    "judgement": "violation",
                                    "observation": "2026-08-04",
                                    "source_ref": "data/snapshots/events.html",
                                    "direction": None,
                                }
                            ],
                            "nearest_miss": [
                                {
                                    "claim_index": 0,
                                    "value": {
                                        "raw_source": "2026-08-03",
                                        "distance": 1,
                                    },
                                }
                            ],
                            "source_refs": ["data/snapshots/events.html"],
                        },
                    }
                )
            )

            rows = list(calibration_corpus.records(campaign))

            self.assertEqual(len(rows), 1)
            self.assertEqual(rows[0]["kind"], "n2")
            self.assertEqual(rows[0]["claim"], "date")
            self.assertEqual(rows[0]["judgement"], "violation")
            self.assertEqual(rows[0]["nearest_miss"]["distance"], 1)
            self.assertEqual(
                rows[0]["source"], "data/snapshots/events.html"
            )

    def test_measured_c2_nearest_miss_is_retroactively_collected_once(self):
        rows = list(calibration_corpus.records(MEASURED_ELEV_003))

        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["kind"], "c2")
        self.assertEqual(rows[0]["judgement"], "violation")
        self.assertEqual(rows[0]["claim"], "--anvil-invalid-probe")
        self.assertEqual(
            rows[0]["nearest_miss"],
            {"candidate": "--pattern", "edit_distance": 17},
        )

        with tempfile.TemporaryDirectory() as tmp:
            store = Path(tmp)
            self.assertEqual(
                calibration_corpus.append([MEASURED_ELEV_003], store=store), 1
            )
            self.assertEqual(
                calibration_corpus.append([MEASURED_ELEV_003], store=store), 0
            )
            saved = [
                json.loads(line)
                for line in (store / "c2/records.jsonl").read_text().splitlines()
            ]
            self.assertEqual(saved, rows)

    def test_workspace_and_artifact_copies_are_one_logical_record(self):
        with tempfile.TemporaryDirectory() as tmp:
            campaign = Path(tmp) / "measured-campaign"
            source = MEASURED_ELEV_003 / "evidence/help-binding.json"
            for root in ("artifacts", "workspaces"):
                evidence = campaign / root / "filter_cloud_001/evidence"
                evidence.mkdir(parents=True)
                shutil.copyfile(source, evidence / "help-binding.json")

            rows = list(calibration_corpus.records(campaign))
            self.assertEqual(len(rows), 2)
            self.assertEqual(rows[0]["record_id"], rows[1]["record_id"])

            store = Path(tmp) / "store"
            self.assertEqual(calibration_corpus.append([campaign], store=store), 1)
            saved = (store / "c2/records.jsonl").read_text().splitlines()
            self.assertEqual(len(saved), 1)


if __name__ == "__main__":
    unittest.main()
