import copy
import json
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_phase6 import (
    FINAL_DECISIONS,
    build_phase6_report,
    validate_manifest,
    write_phase6_report,
)

MANIFEST_PATH = ROOT / "eval/goal_verify/v0/phase6-matrix.json"
CONFIG_PATH = ROOT / "eval/goal_verify/v0/baseline-config.json"


class GoalVerifyPhase6Test(unittest.TestCase):
    def setUp(self):
        self.manifest = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
        self.config = json.loads(CONFIG_PATH.read_text(encoding="utf-8"))
        self.config["bootstrap_samples"] = 100

    def test_checked_in_matrix_fails_closed_with_separate_evidence_lanes(self):
        self.assertEqual(validate_manifest(self.manifest, ROOT), [])
        report = build_phase6_report(self.manifest, self.config, ROOT)
        self.assertEqual(report["final_decision"], "INSUFFICIENT-EVIDENCE")
        self.assertEqual(set(report["evidence_lanes"]), {"blind_review", "ci", "offline_local", "approved_live"})
        self.assertFalse(report["evidence_lanes"]["approved_live"]["authorized"])
        self.assertEqual(report["candidate"]["corpus"], None)
        self.assertTrue(report["failure_cases"])

    def test_every_indicator_has_baseline_candidate_delta_ci_threshold_and_verdict(self):
        report = build_phase6_report(self.manifest, self.config, ROOT)
        required = {
            "baseline",
            "candidate",
            "delta",
            "confidence_interval_95",
            "threshold",
            "verdict",
        }
        self.assertGreaterEqual(len(report["indicators"]), 10)
        for indicator in report["indicators"]:
            self.assertTrue(required.issubset(indicator), indicator["name"])
            self.assertEqual(indicator["verdict"], "insufficient_evidence")
        self.assertIn(report["final_decision"], FINAL_DECISIONS)

    def test_measured_safety_regression_takes_no_go_precedence(self):
        manifest = copy.deepcopy(self.manifest)
        corpus = json.loads((ROOT / manifest["baseline"]["corpus"]).read_text(encoding="utf-8"))
        for case in corpus["cases"]:
            case["observation"]["verified_claims"] = []
        with tempfile.TemporaryDirectory() as temporary:
            candidate_path = Path(temporary) / "candidate.json"
            candidate_path.write_text(json.dumps(corpus), encoding="utf-8")
            manifest["candidate"]["corpus"] = str(candidate_path)
            manifest["candidate"]["absence_reason"] = ""
            report = build_phase6_report(manifest, self.config, ROOT)
        self.assertEqual(report["final_decision"], "NO-GO")
        recall = next(item for item in report["indicators"] if item["name"] == "required_claim_recall")
        self.assertEqual(recall["verdict"], "failed")

    def test_missing_raw_reference_and_unsubstantiated_live_authorization_are_rejected(self):
        manifest = copy.deepcopy(self.manifest)
        manifest["evidence_lanes"]["ci"]["references"].append("missing/ci.json")
        live = manifest["evidence_lanes"]["approved_live"]
        live["authorized"] = True
        errors = validate_manifest(manifest, ROOT)
        self.assertTrue(any("missing/ci.json" in error for error in errors), errors)
        self.assertTrue(any("authorized live evidence" in error for error in errors), errors)

    def test_writer_is_deterministic_and_refuses_overwrite(self):
        with tempfile.TemporaryDirectory() as temporary:
            first_dir = Path(temporary) / "first"
            second_dir = Path(temporary) / "second"
            write_phase6_report(
                manifest_path=MANIFEST_PATH,
                config_path=CONFIG_PATH,
                run_dir=first_dir,
                root=ROOT,
            )
            write_phase6_report(
                manifest_path=MANIFEST_PATH,
                config_path=CONFIG_PATH,
                run_dir=second_dir,
                root=ROOT,
            )
            self.assertEqual(
                (first_dir / "phase6-report.json").read_bytes(),
                (second_dir / "phase6-report.json").read_bytes(),
            )
            with self.assertRaises(FileExistsError):
                write_phase6_report(
                    manifest_path=MANIFEST_PATH,
                    config_path=CONFIG_PATH,
                    run_dir=first_dir,
                    root=ROOT,
                )


if __name__ == "__main__":
    unittest.main()
