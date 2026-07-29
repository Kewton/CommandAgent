#!/usr/bin/env python3
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from acceptance_sheet import generate


class AcceptanceSheetTests(unittest.TestCase):
    def make(self, text, extra=None):
        d = Path(tempfile.mkdtemp())
        (d / "workflow-events.jsonl").write_text(text)
        if extra:
            (d / "evidence").mkdir()
            (d / "evidence" / "check.json").write_text(extra)
        return d

    def test_full_shape(self):
        s = generate(
            self.make(
                '{"event":"run_stop","status":"full"}\n',
                '{"check_id":"pipeline_probe"}',
            )
        )
        self.assertIn("定義された検証を全て実行し成立", s)
        self.assertIn("パイプラインを実行", s)

    def test_failed_missing_is_recorded(self):
        s = generate(
            self.make(
                '{"event":"workflow_adjudicated","verdict":"circle_failed","reason":"node_failed:fix"}\n'
            )
        )
        self.assertIn("未完了", s)
        self.assertIn("記録なし", s)

    def test_circle_duration_is_derived_from_new_workflow_epochs(self):
        d = self.make(
            '{"event":"workflow_started","entry":"create","epoch":100}\n'
            '{"event":"workflow_adjudicated","verdict":"circle_full","epoch":118}\n'
        )
        (d / "workflow-circle.json").write_text('{"verdict":"circle_full"}')
        s = generate(d)
        self.assertIn("円環全体の所要: 18秒", s)

    def test_unknown_check_passthrough(self):
        s = generate(
            self.make(
                '{"event":"run_stop","status":"full"}\n', '{"check_id":"new_check"}'
            )
        )
        self.assertIn("`new_check`", s)

    def test_evidence_values_are_transcribed(self):
        d = self.make(
            '{"event":"run_start","action":"goal text","profile":"data","model":"m","provider":"ollama","planner_model":"p"}\n{"event":"intent_resolved","value":"create"}\n{"event":"run_stop","status":"full"}\n',
            '{"capability_id":"pipeline_probe","command":"python3 pipeline/main.py","exit_code":1,"outcome":"CommandFailed"}',
        )
        (d / "evidence" / "claims-binding.json").write_text(
            '{"capability_id":"data_claims_binding","claims":[{"raw":"7","matched_result_value":7,"ok":true}]}'
        )
        s = generate(d)
        self.assertIn("goal text", s)
        self.assertIn("exit=1", s)
        self.assertIn("claims=1, matched=1", s)
        self.assertIn("7 × 7 × pass", s)

    def test_circle_acceptance_observations(self):
        p = (
            Path(__file__).parents[3]
            / "workspace/management/runs/uat-test0722-circle-elev-008/run1"
        )
        if not p.is_dir():
            self.skipTest("measurement fixture unavailable")
        d = Path(tempfile.mkdtemp()) / "run1"
        shutil.copytree(p, d)
        s = generate(d)
        for token in (
            "gemma4:31b-cloud",
            "qwen3.6:27b-coding-nvfp4",
            "I2: claims=5, matched=5",
            "CommandFailed",
            "data_results_schema",
            "verify_origin",
            "E-A",
            "E-B",
            "E-C",
            "E-D",
        ):
            self.assertIn(token, s)
        # 検証の精密化: epoch不在は記録なし、人手計測は出自付き別枠。
        self.assertIn("円環全体の所要: 記録なし", s)
        self.assertIn("ノード実行の所要: 6秒", s)
        (d / "manual-timing.md").write_text("18秒（人手計測）")
        s = generate(d)
        self.assertIn("参考: 人手計測による全体所要 18秒", s)
        self.assertIn("stage=before executed=True expected=failure", s)
        self.assertIn("stage=after executed=True expected=success", s)
        self.assertNotIn("/Users/", s)


if __name__ == "__main__":
    unittest.main()
