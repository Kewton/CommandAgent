#!/usr/bin/env python3
import json
import tempfile
import unittest
from pathlib import Path

from classify_runs import classes, classify, classify_campaign


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

    def test_category_metadata_does_not_change_classification(self):
        registry = classes()
        without_category = [
            {key: value for key, value in item.items() if key != "category"}
            for item in registry
        ]
        self.assertTrue(
            all(
                item["category"] in {"terminal", "violation_family"}
                for item in registry
            )
        )

        for item in registry:
            terms = [
                item.get("match_stop_class"),
                item.get("match_reason"),
                item.get("match_phase"),
                item.get("match_event"),
            ]
            for term in (term for term in terms if term):
                run = self.make(
                    json.dumps(
                        {
                            "event": "run_stop",
                            "reason": term,
                            "stop_class": term,
                            "failure_kind": term,
                        }
                    )
                )
                self.assertEqual(
                    classify(run, registry),
                    classify(run, without_category),
                    item["id"],
                )
