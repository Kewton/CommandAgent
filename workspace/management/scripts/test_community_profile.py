from __future__ import annotations

import json
import shutil
import tempfile
import unittest
from pathlib import Path

import community_profile

ROOT = Path(__file__).resolve().parents[3]
FIXTURE = ROOT / "workspace/management/bench/community/synthetic-community"


class CommunityProfileSpecTests(unittest.TestCase):
    def test_synthetic_spec_passes_pinned_schema(self):
        result = community_profile.validate_spec(
            FIXTURE / "app.spec.yaml",
            FIXTURE / "schema/app-spec.schema.yaml",
            FIXTURE / "schema/app-spec.schema.sha256",
        )
        self.assertEqual(result["verdict"], "pass")
        self.assertEqual(result["family"], "S")

    def test_schema_pin_mismatch_is_fail_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            pin = Path(directory) / "pin"
            pin.write_text("0" * 64, encoding="utf-8")
            with self.assertRaises(community_profile.ValidationError):
                community_profile.validate_schema_pin(FIXTURE / "schema/app-spec.schema.yaml", pin)

    def test_unknown_field_is_fail_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            spec = Path(directory) / "app.spec.yaml"
            spec.write_text(
                (FIXTURE / "app.spec.yaml").read_text(encoding="utf-8") + "\nunknown: true\n",
                encoding="utf-8",
            )
            with self.assertRaises(community_profile.ValidationError):
                community_profile.validate_spec(spec, FIXTURE / "schema/app-spec.schema.yaml", FIXTURE / "schema/app-spec.schema.sha256")

    def test_computed_ast_is_bounded_and_typed(self):
        with self.assertRaises(community_profile.ValidationError):
            community_profile.ExpressionParser("eval(1)", {"count": "number"}).parse()
        with self.assertRaises(community_profile.ValidationError):
            community_profile.ExpressionParser("count + 'bad'", {"count": "number"}).parse()

    def test_zone_rejects_core_diff_and_forbidden_api(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "community"
            shutil.copytree(FIXTURE, root)
            with self.assertRaises(community_profile.ValidationError):
                community_profile.validate_zone(root, root / "core.sha256sums", ["core/router.ts"])
            (root / "src/app-zone/attack.ts").write_text("const x = process.env.SECRET;\n", encoding="utf-8")
            with self.assertRaises(community_profile.ValidationError):
                community_profile.validate_zone(root, root / "core.sha256sums", [])

    def test_zone_rejects_initial_allowlist_dependency(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "community"
            shutil.copytree(FIXTURE, root)
            package = json.loads((root / "package.json").read_text(encoding="utf-8"))
            package["dependencies"] = {"unreviewed": "1.0.0"}
            (root / "package.json").write_text(json.dumps(package), encoding="utf-8")
            with self.assertRaises(community_profile.ValidationError):
                community_profile.validate_zone(root, root / "core.sha256sums", [])


if __name__ == "__main__":
    unittest.main()
