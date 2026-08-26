import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_preflight_v2 import assess_v2_readiness


class GoalVerifyPreflightV2Test(unittest.TestCase):
    def test_checked_in_draft_fails_closed_with_actionable_blockers(self):
        result = assess_v2_readiness(
            root=ROOT,
            contract_path=ROOT
            / "eval/goal_verify/v0/phase6-preflight-v2-contract.json",
        )
        self.assertFalse(result["ready"])
        self.assertEqual(result["selected_case_count"], 8)
        self.assertEqual(result["expected_pair_count"], 40)
        self.assertEqual(result["registered_adapter_count"], 14)
        self.assertEqual(
            result["blockers"],
            [
                "contract_not_frozen",
                "exact_code_sha_missing",
                "exact_sha_ci_evidence_missing",
                "live_preflight_not_authorized",
            ],
        )


if __name__ == "__main__":
    unittest.main()
