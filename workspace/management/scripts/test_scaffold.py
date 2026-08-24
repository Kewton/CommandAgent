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
            self.assertIn(
                scaffold.STRUCTURE_LITERAL_GUIDANCE_CHECKLIST_ITEM,
                (out / "ADMISSION.md").read_text(),
            )
            self.assertIn(
                scaffold.MACHINE_PLAN_PRESET_CHECKLIST_ITEM,
                (out / "ADMISSION.md").read_text(),
            )
            self.assertIn(
                scaffold.SOURCE_MATERIAL_INJECTION_CHECKLIST_ITEM,
                (out / "ADMISSION.md").read_text(),
            )
            self.assertIn(
                scaffold.MEASUREMENT_ASSET_DESIGN_CHECKLIST_ITEM,
                (out / "ADMISSION.md").read_text(),
            )
            self.assertIn(
                "## Measurement asset design",
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
            self.assertIn(
                scaffold.STRUCTURE_LITERAL_GUIDANCE_CHECKLIST_ITEM,
                checklist,
            )
            self.assertIn(scaffold.MACHINE_PLAN_PRESET_CHECKLIST_ITEM, checklist)
            self.assertIn(
                scaffold.SOURCE_MATERIAL_INJECTION_CHECKLIST_ITEM,
                checklist,
            )
            self.assertIn(
                scaffold.MEASUREMENT_ASSET_DESIGN_CHECKLIST_ITEM,
                checklist,
            )

    def test_intent_same_shape(self):
        old = scaffold.ROOT
        with tempfile.TemporaryDirectory() as d:
            scaffold.ROOT = Path(d)
            self.assertTrue(
                (scaffold.generate("intent", "inspect") / "corpus").is_dir()
            )
        scaffold.ROOT = old

    def test_pack_scaffold_clones_identity_but_not_reviewed_hash_pin(self):
        old = scaffold.REPOSITORY_ROOT
        with tempfile.TemporaryDirectory() as directory:
            scaffold.REPOSITORY_ROOT = Path(directory)
            source = Path(directory) / "packs" / "cli-assist" / "1.1.0"
            source.mkdir(parents=True)
            (source / "assist.yaml").write_text(
                "schema_version: commandagent.pack.assist/v0\n"
                "pack:\n"
                "  id: cli-assist\n"
                "  version: 1.1.0\n"
                "  profile: python-cli\n"
                "  intent: create\n"
                "inject: []\n",
                encoding="utf-8",
            )
            (source / "pack.sha256").write_text(
                "sha256:" + ("a" * 64) + "\n", encoding="utf-8"
            )

            output = scaffold.generate_pack("cli-assist", "1.1.0", "1.1.1")

            self.assertIn("  version: 1.1.1\n", (output / "assist.yaml").read_text())
            self.assertFalse((output / "pack.sha256").exists())
        scaffold.REPOSITORY_ROOT = old
