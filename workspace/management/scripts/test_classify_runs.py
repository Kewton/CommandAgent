#!/usr/bin/env python3
import tempfile
import unittest
from pathlib import Path

from classify_runs import classify, classify_campaign


class ClassifyTests(unittest.TestCase):
    def make(self, text):
        d = Path(tempfile.mkdtemp())
        (d / "events.jsonl").write_text(text)
        return d

    def test_known(self):
        r = classify(self.make('{"reason":"repair_target_unresolved"}'))
        self.assertEqual(r["classes"], ["repair_target_unresolved"])

    def test_unknown(self):
        r = classify(self.make('{"reason":"new_unseen_stop"}'))
        self.assertEqual(r["attribution"], "UNKNOWN")

    def test_longest_and_ambiguous(self):
        reg = [
            {"id": "short", "attribution": "machine", "match_reason": "loop"},
            {"id": "long", "attribution": "model", "match_reason": "read_only_loop"},
        ]
        self.assertEqual(
            classify(self.make("read_only_loop"), reg)["classes"], ["long"]
        )
        reg.append(
            {"id": "other", "attribution": "mixed", "match_reason": "read_only_loop"}
        )
        self.assertEqual(
            set(classify(self.make("read_only_loop"), reg)["classes"]),
            {"long", "other"},
        )

    def test_campaign_prefers_artifact_copy_of_same_run(self):
        with tempfile.TemporaryDirectory() as tmp:
            campaign = Path(tmp)
            for root in ("artifacts", "workspaces"):
                run = campaign / root / "run-001/.anvil/runs/id"
                run.mkdir(parents=True)
                (run / "events.jsonl").write_text(
                    '{"event":"run_stop","reason":"read_only_loop"}\n'
                )
            registry = [
                {
                    "id": "known",
                    "attribution": "model",
                    "match_reason": "read_only_loop",
                }
            ]

            rows = classify_campaign(campaign, registry)

            self.assertEqual(len(rows), 1)
            self.assertIn("/artifacts/", rows[0]["run"])
