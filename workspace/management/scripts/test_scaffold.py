#!/usr/bin/env python3
import tempfile
import unittest
from pathlib import Path

import scaffold


class ScaffoldTests(unittest.TestCase):
    def test_chapters_and_artifacts(self):
        old = scaffold.ROOT
        with tempfile.TemporaryDirectory() as d:
            scaffold.ROOT = Path(d)
            out = scaffold.generate("profile", "demo")
            text = (out / "contract.md").read_text()
            for chapter in scaffold.CHAPTERS:
                self.assertIn(f"## {chapter}", text)
            for name in ("manifest.toml", "conformance.md", "ADMISSION.md"):
                self.assertTrue((out / name).is_file())
            self.assertIn(
                scaffold.PROJECTION_CHECKLIST_ITEM,
                (out / "ADMISSION.md").read_text(),
            )
            self.assertIn(
                scaffold.PRODUCTION_ACTIVATION_CHECKLIST_ITEM,
                (out / "ADMISSION.md").read_text(),
            )
            self.assertEqual(
                (out / "manifest.toml")
                .read_text()
                .split('admission = "')[1]
                .split('"')[0],
                "off",
            )
        scaffold.ROOT = old

    def test_checked_in_draft_templates_require_completion_projection(self):
        for kind, ident in (("profile", "demo"), ("intent", "investigate")):
            checklist = (
                scaffold.ROOT / "scaffolds" / kind / ident / "ADMISSION.md"
            ).read_text()
            self.assertIn(scaffold.PROJECTION_CHECKLIST_ITEM, checklist)
            self.assertIn(scaffold.PRODUCTION_ACTIVATION_CHECKLIST_ITEM, checklist)

    def test_intent_same_shape(self):
        old = scaffold.ROOT
        with tempfile.TemporaryDirectory() as d:
            scaffold.ROOT = Path(d)
            self.assertTrue(
                (scaffold.generate("intent", "inspect") / "corpus").is_dir()
            )
        scaffold.ROOT = old
