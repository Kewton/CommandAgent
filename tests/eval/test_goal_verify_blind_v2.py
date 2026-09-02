import copy
import json
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_blind_v2 import (
    prepare_semantic_items,
    semantic_arms_from_paired_records,
    semantic_proposal_card,
)


class GoalVerifyBlindV2Test(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.case = json.loads(
            (ROOT / "eval/goal_verify/v0/corpus.json").read_text(encoding="utf-8")
        )["cases"][0]
        cls.proposal = json.loads(
            (ROOT / "tests/fixtures/verification_spec_v0/create.json").read_text(
                encoding="utf-8"
            )
        )

    def _record(self, *, model: str, request_id: str):
        proposal = copy.deepcopy(self.proposal)
        proposal["generation"] = {
            "provider": "secret-provider",
            "model": model,
            "request_id": request_id,
            "raw_response_sha256": "a" * 64,
        }
        return {
            "record_path": f"raw/{request_id}.json",
            "response": {
                "status": "completed",
                "response": {
                    "model": model,
                    "prompt_eval_count": 99,
                    "total_duration": 1234,
                    "response": json.dumps(proposal),
                },
            },
            "validation": {"valid": True, "errors": []},
            "oracle_evaluations": [
                {
                    "oracle_id": proposal["oracles"][0]["id"],
                    "executed": True,
                    "result": "pass",
                    "observed_strength": "runtime",
                    "reason": "observation_match",
                }
            ],
        }

    def test_semantic_card_uses_raw_proposal_and_hides_machine_metadata(self):
        card = semantic_proposal_card(self._record(model="model-a", request_id="req-a"))
        encoded = json.dumps(card)
        self.assertEqual(card["parse_status"], "parsed")
        self.assertTrue(card["claims"])
        self.assertTrue(card["oracles"])
        self.assertNotIn("model-a", encoded)
        self.assertNotIn("req-a", encoded)
        self.assertNotIn("raw_response_sha256", encoded)
        self.assertNotIn("lineage", encoded)
        self.assertNotIn("lifecycle", encoded)
        self.assertNotIn("validation", encoded)
        self.assertEqual(card["execution_results"][0]["result"], "pass")

    def test_semantic_items_are_deterministic_and_variant_hidden(self):
        pair_id = "pair-01"
        left = {pair_id: self._record(model="left-model", request_id="left")}
        right = {pair_id: self._record(model="right-model", request_id="right")}
        cases = {pair_id: self.case}
        first, first_mapping = prepare_semantic_items(
            left, right, cases_by_pair_id=cases, seed=42
        )
        second, second_mapping = prepare_semantic_items(
            left, right, cases_by_pair_id=cases, seed=42
        )
        self.assertEqual(first, second)
        self.assertEqual(first_mapping, second_mapping)
        encoded = json.dumps(first)
        self.assertNotIn("left-model", encoded)
        self.assertNotIn("right-model", encoded)
        self.assertNotIn("record_path", encoded)
        self.assertNotIn("validation", encoded)
        self.assertEqual(set(first_mapping[pair_id]), {"A", "B"})

    def test_semantic_items_require_identical_pair_sets(self):
        with self.assertRaises(ValueError):
            prepare_semantic_items(
                {"left": self._record(model="a", request_id="a")},
                {"right": self._record(model="b", request_id="b")},
                cases_by_pair_id={"left": self.case},
                seed=42,
            )

    def test_paired_record_exposes_same_scope_baseline_and_candidate_arms(self):
        candidate = self._record(model="candidate", request_id="candidate")
        baseline_spec = copy.deepcopy(self.proposal)
        record = {
            **candidate,
            "baseline_spec": baseline_spec,
            "baseline_oracle_evaluations": [
                {
                    "oracle_id": baseline_spec["oracles"][0]["id"],
                    "executed": True,
                    "result": "pass",
                    "observed_strength": "deterministic",
                }
            ],
        }
        baseline, candidates = semantic_arms_from_paired_records({"pair": record})
        items, mapping = prepare_semantic_items(
            baseline,
            candidates,
            cases_by_pair_id={"pair": self.case},
            seed=1,
        )
        self.assertEqual(len(items), 1)
        self.assertEqual(set(mapping["pair"].values()), {"left", "right"})
        encoded = json.dumps(items)
        self.assertNotIn("candidate", encoded)
        self.assertIn("execution_results", encoded)


if __name__ == "__main__":
    unittest.main()
