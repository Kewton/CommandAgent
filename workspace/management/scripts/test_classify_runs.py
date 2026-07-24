#!/usr/bin/env python3
import tempfile
import unittest
from pathlib import Path

from classify_runs import classify


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
