import copy
import hashlib
import json
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_blind import prepare_items, validate_blind_evidence
from eval_lib.goal_verify_live import _atomic_json, sha256_file


class GoalVerifyBlindTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.corpus = json.loads(
            (ROOT / "eval/goal_verify/v0/corpus.json").read_text(encoding="utf-8")
        )

    def test_items_are_deterministic_and_hide_variant_metadata(self):
        baseline = {**self.corpus, "cases": [self.corpus["cases"][0]]}
        candidate = json.loads(json.dumps(baseline))
        candidate["cases"][0]["observation"]["wall_time_ms"] = 1
        first_items, first_mapping = prepare_items(baseline, candidate, seed=42)
        second_items, second_mapping = prepare_items(baseline, candidate, seed=42)
        self.assertEqual(first_items, second_items)
        self.assertEqual(first_mapping, second_mapping)
        encoded = json.dumps(first_items)
        self.assertNotIn("wall_time_ms", encoded)
        self.assertNotIn("source_reference", encoded)
        self.assertNotIn("candidate", encoded)
        self.assertEqual(set(first_mapping[baseline["cases"][0]["case_id"]]), {"A", "B"})

    def test_pair_ids_must_match_exactly(self):
        baseline = {**self.corpus, "cases": [self.corpus["cases"][0]]}
        candidate = {**self.corpus, "cases": [self.corpus["cases"][1]]}
        with self.assertRaises(ValueError):
            prepare_items(baseline, candidate, seed=42)

    def test_validation_reconstructs_raw_reviews_and_binds_candidate(self):
        contract_path = ROOT / "eval/goal_verify/v0/phase6-blind-review-contract.json"
        contract = json.loads(contract_path.read_text(encoding="utf-8"))
        baseline = {**self.corpus, "cases": [self.corpus["cases"][0]]}
        draft = json.loads(json.dumps(baseline))
        draft["annotation_protocol"]["status"] = "pending"
        with tempfile.TemporaryDirectory(dir=ROOT) as temporary:
            run_dir = Path(temporary)
            baseline_path = run_dir / "baseline.json"
            draft_path = run_dir / "candidate-draft.json"
            _atomic_json(baseline_path, baseline)
            _atomic_json(draft_path, draft)
            items, mapping = prepare_items(
                baseline, draft, seed=int(contract["randomization_seed"])
            )
            items_path = run_dir / "blind-items.json"
            mapping_path = run_dir / "variant-mapping.json"
            _atomic_json(items_path, items)
            _atomic_json(mapping_path, mapping)
            pair_id = items[0]["pair_id"]
            reviews = [{"pair_id": pair_id, "preferred_variant": "tie", "reason_codes": ["equal"]}]
            raw = json.dumps({"reviews": reviews})
            batch_path = run_dir / "raw" / "batch-01.json"
            _atomic_json(
                batch_path,
                {
                    "batch_id": 1,
                    "attempt": 0,
                    "response": {"status": "completed", "response": {"response": raw}},
                    "reviews": reviews,
                    "error": None,
                },
            )
            final_candidate = copy.deepcopy(draft)
            final_candidate["annotation_protocol"]["status"] = "reviewed"
            candidate_path = run_dir / "candidate-corpus.json"
            _atomic_json(candidate_path, final_candidate)
            canonical_items = hashlib.sha256(
                json.dumps(items, ensure_ascii=False, separators=(",", ":"), sort_keys=True).encode()
            ).hexdigest()
            canonical_mapping = hashlib.sha256(
                json.dumps(mapping, ensure_ascii=False, separators=(",", ":"), sort_keys=True).encode()
            ).hexdigest()
            _atomic_json(
                run_dir / "blind-review-manifest.json",
                {
                    "contract_sha256": sha256_file(contract_path),
                    "baseline_sha256": sha256_file(baseline_path),
                    "candidate_draft_sha256": sha256_file(draft_path),
                    "item_count": 1,
                    "blind_items_sha256": canonical_items,
                    "variant_mapping_sha256": canonical_mapping,
                    "reviewer_source_sha256": {},
                },
            )
            _atomic_json(
                run_dir / "blind-review-report.json",
                {
                    "complete": True,
                    "reviewed_pairs": 1,
                    "blind_items_sha256": canonical_items,
                    "variant_mapping_sha256": canonical_mapping,
                    "blind_items_file_sha256": sha256_file(items_path),
                    "variant_mapping_file_sha256": sha256_file(mapping_path),
                    "batch_record_sha256": {"batch-01": sha256_file(batch_path)},
                    "candidate_corpus_sha256": sha256_file(candidate_path),
                    "preference_counts": {"tie": 1},
                    "reviews": [{**reviews[0], "decoded_preference": "tie"}],
                },
            )
            validate_blind_evidence(
                root=ROOT,
                baseline_path=baseline_path,
                candidate_draft_path=draft_path,
                contract_path=contract_path,
                run_dir=run_dir,
            )
            final_candidate["cases"][0]["goal"] = "tampered"
            _atomic_json(candidate_path, final_candidate)
            with self.assertRaises(ValueError):
                validate_blind_evidence(
                    root=ROOT,
                    baseline_path=baseline_path,
                    candidate_draft_path=draft_path,
                    contract_path=contract_path,
                    run_dir=run_dir,
                )


if __name__ == "__main__":
    unittest.main()
