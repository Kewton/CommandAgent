import json
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_finalize import build_live_matrix


class GoalVerifyFinalizeTest(unittest.TestCase):
    def test_live_matrix_keeps_cells_and_marks_every_lane_available(self):
        template = json.loads(
            (ROOT / "eval/goal_verify/v0/phase6-matrix.json").read_text(encoding="utf-8")
        )
        with tempfile.TemporaryDirectory(dir=ROOT) as temporary:
            temporary_path = Path(temporary)
            matrix = build_live_matrix(
                root=ROOT,
                template=template,
                campaign_dir=temporary_path / "campaign",
                blind_dir=temporary_path / "blind",
                baseline_report=temporary_path / "finalization" / "baseline.json",
                candidate_report=temporary_path / "finalization" / "candidate.json",
            )
        self.assertEqual(matrix["minimum_samples_per_cell"], 30)
        self.assertEqual(matrix["cells"], template["cells"])
        self.assertTrue(matrix["evidence_lanes"]["approved_live"]["authorized"])
        self.assertTrue(
            all(lane["status"] == "available" for lane in matrix["evidence_lanes"].values())
        )
        self.assertNotIn("absence_reason", matrix["candidate"])


if __name__ == "__main__":
    unittest.main()
