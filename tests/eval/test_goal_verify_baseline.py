import copy
import json
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_baseline import (
    build_report,
    load_json,
    validate_corpus,
    write_report,
)

CORPUS_PATH = ROOT / "eval/goal_verify/v0/corpus.json"
CONFIG_PATH = ROOT / "eval/goal_verify/v0/baseline-config.json"


class GoalVerifyBaselineTest(unittest.TestCase):
    def setUp(self):
        self.corpus = load_json(CORPUS_PATH)
        self.config = load_json(CONFIG_PATH)
        self.config["bootstrap_samples"] = 100

    def test_frozen_corpus_is_reviewed_and_covers_each_intent_polarity(self):
        self.assertEqual(validate_corpus(self.corpus), [])
        pairs = {(case["intent"], case["polarity"]) for case in self.corpus["cases"]}
        self.assertEqual(
            pairs,
            {
                ("create", "positive"),
                ("create", "negative"),
                ("fix", "positive"),
                ("fix", "negative"),
                ("investigate", "positive"),
                ("investigate", "negative"),
            },
        )

    def test_baseline_metrics_capture_current_fixture_replay(self):
        report = build_report(self.corpus, self.config)
        metrics = report["metrics"]
        self.assertEqual(metrics["case_count"], 12)
        self.assertEqual(metrics["required_claim_count"], 16)
        self.assertEqual(metrics["false_full_count"], 0)
        self.assertEqual(metrics["required_claim_precision"], 0.705882)
        self.assertEqual(metrics["required_claim_recall"], 0.5)
        self.assertEqual(metrics["strong_binding_coverage"], 0.5)
        self.assertEqual(metrics["schema_compliance_yield"], 0.916667)
        self.assertEqual(report["go_no_go"]["status"], "go")

    def test_bootstrap_and_report_are_deterministic_for_same_seed(self):
        first = build_report(self.corpus, self.config)
        second = build_report(self.corpus, self.config)
        self.assertEqual(first, second)
        self.assertEqual(
            first["confidence_intervals_95"]["overall"]["required_claim_recall"]["status"],
            "estimated",
        )
        cell = next(iter(first["confidence_intervals_95"]["cells"].values()))
        self.assertEqual(cell["required_claim_recall"]["status"], "insufficient_evidence")

    def test_schema_and_adversarial_coverage_fail_closed(self):
        invalid = copy.deepcopy(self.corpus)
        invalid["schema_version"] = "future"
        invalid["cases"][0]["tags"].remove("build_only_insufficient")
        errors = validate_corpus(invalid)
        self.assertTrue(any("schema_version" in error for error in errors), errors)
        self.assertTrue(any("build_only_insufficient" in error for error in errors), errors)

    def test_writer_refuses_to_overwrite_a_run(self):
        with tempfile.TemporaryDirectory() as temporary:
            run_dir = Path(temporary) / "run"
            write_report(corpus_path=CORPUS_PATH, config_path=CONFIG_PATH, run_dir=run_dir)
            baseline = json.loads((run_dir / "baseline.json").read_text(encoding="utf-8"))
            self.assertEqual(baseline["seed"], self.config["seed"])
            with self.assertRaises(FileExistsError):
                write_report(corpus_path=CORPUS_PATH, config_path=CONFIG_PATH, run_dir=run_dir)


if __name__ == "__main__":
    unittest.main()
