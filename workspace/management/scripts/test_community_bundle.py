from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

import community_bundle


class CommunityBundleTests(unittest.TestCase):
    def make_bundle(self, root: Path) -> Path:
        bundle = root / "bundle"
        artifacts = bundle / "artifacts"
        artifacts.mkdir(parents=True)
        (artifacts / "app.spec.yaml").write_text("entities: []\n", encoding="utf-8")
        community_bundle.write_json(
            bundle / "reverification.json",
            {"schema_version": community_bundle.REVERIFY_SCHEMA, "verdict_equal": True},
        )
        community_bundle.write_manifest(
            bundle,
            source_run="fixture-l2",
            level="L2",
            verdict="full",
            binary_sha256="a" * 64,
        )
        return bundle

    def test_manifest_round_trip_and_tamper_are_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            bundle = self.make_bundle(Path(directory))
            manifest = community_bundle.verify_manifest(bundle)
            self.assertEqual(manifest["expected_verdict"], "full")
            (bundle / "artifacts/app.spec.yaml").write_text(
                "entities: [tampered]\n", encoding="utf-8"
            )
            with self.assertRaisesRegex(community_bundle.BundleError, "mismatch"):
                community_bundle.verify_manifest(bundle)

    def test_manifest_rejects_unlisted_file(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            bundle = self.make_bundle(Path(directory))
            (bundle / "unlisted.txt").write_text("unexpected", encoding="utf-8")
            with self.assertRaisesRegex(
                community_bundle.BundleError, "inventory mismatch"
            ):
                community_bundle.verify_manifest(bundle)

    def test_l2_promotion_record_makes_no_evidence_claim(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            record = community_bundle.promotion_record(root, "L2")
            self.assertEqual(record["status"], "not_applicable_l2")
            self.assertFalse(record["evidence_claim"])
            self.assertIsNone(record["promotion_evidence_path"])

    def test_l3_promotion_record_requires_real_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with self.assertRaisesRegex(community_bundle.BundleError, "missing"):
                community_bundle.promotion_record(root, "L3")
            evidence = root / "evidence/promotion-decision.json"
            evidence.parent.mkdir()
            evidence.write_text(json.dumps({"decision": "promote"}), encoding="utf-8")
            record = community_bundle.promotion_record(root, "L3")
            self.assertTrue(record["evidence_claim"])
            self.assertEqual(
                record["evidence_sha256"], community_bundle.sha256_file(evidence)
            )

    def test_level_detection_distinguishes_l2_and_l3(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.assertEqual(community_bundle.artifact_level(root), "L2")
            zone = root / "src/app-zone"
            zone.mkdir(parents=True)
            (zone / "app.ts").write_text("export {};\n", encoding="utf-8")
            self.assertEqual(community_bundle.artifact_level(root), "L3")


if __name__ == "__main__":
    unittest.main()
