import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_live import (
    _acquire_run_lock,
    _append_record_ledger,
    _atomic_json,
    _candidate_case,
    _load_record_ledger,
    _verify_exact_sha_ci,
    _verify_frozen_git_inputs,
    build_prompt,
)


class GoalVerifyLiveTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.corpus = json.loads(
            (ROOT / "eval/goal_verify/v0/corpus.json").read_text(encoding="utf-8")
        )

    def test_prompt_exposes_frozen_evidence_only_for_evidence_intents(self):
        create = self.corpus["cases"][0]
        fix = next(case for case in self.corpus["cases"] if case["intent"] == "fix")
        create_prompt = build_prompt("base", create, "request-create", "{}")
        fix_prompt = build_prompt("base", fix, "request-fix", "{}")
        self.assertNotIn("existing_evidence_registry", create_prompt)
        self.assertIn("existing_evidence_registry", fix_prompt)
        self.assertIn(fix["required_claims"][0]["id"], fix_prompt)

    def test_candidate_projection_ignores_provider_execution_claims(self):
        source = self.corpus["cases"][1]
        claim_id = source["required_claims"][0]["id"]
        record = {
            "record_path": "evidence/raw.json",
            "response": {
                "status": "completed",
                "response": {
                    "total_duration": 12_000_000,
                    "prompt_eval_count": 10,
                    "eval_count": 20,
                },
            },
            "validation": {
                "valid": True,
                "spec": {
                    "claims": [{"id": claim_id}],
                    "oracles": [
                        {
                            "claim_id": claim_id,
                            "lifecycle": "executed",
                            "result": "pass",
                        }
                    ],
                },
            },
        }
        candidate = _candidate_case(source, "pair-id", record)
        observation = candidate["observation"]
        self.assertEqual(observation["verified_claims"], [source["observation"]["verified_claims"][0]])
        self.assertEqual(observation["wall_time_ms"], 12)
        self.assertEqual(observation["input_tokens"], 10)
        self.assertEqual(observation["output_tokens"], 20)
        self.assertEqual(observation["source_reference"], "evidence/raw.json")

    def test_record_ledger_detects_raw_mutation(self):
        with tempfile.TemporaryDirectory(dir=ROOT) as temporary:
            run_dir = Path(temporary)
            record_path = run_dir / "raw" / "case" / "pair-01.json"
            _atomic_json(record_path, {"pair_id": "case--pair-01"})
            record_reference = str(record_path.relative_to(ROOT))
            ledger_path = run_dir / "record-ledger.jsonl"
            entries = {}
            head = _append_record_ledger(
                ledger_path=ledger_path,
                entries=entries,
                previous="0" * 64,
                pair_id="case--pair-01",
                source_case_id="case",
                record_reference=record_reference,
                record_path=record_path,
            )
            loaded, loaded_head = _load_record_ledger(
                root=ROOT, run_dir=run_dir, ledger_path=ledger_path
            )
            self.assertEqual(set(loaded), {record_reference})
            self.assertEqual(loaded_head, head)
            _atomic_json(record_path, {"pair_id": "changed"})
            with self.assertRaises(ValueError):
                _load_record_ledger(root=ROOT, run_dir=run_dir, ledger_path=ledger_path)

    def test_run_lock_rejects_a_second_process_for_the_same_directory(self):
        with tempfile.TemporaryDirectory(dir=ROOT) as temporary:
            run_dir = Path(temporary)
            first = _acquire_run_lock(run_dir, ".campaign.lock")
            try:
                with self.assertRaises(RuntimeError):
                    _acquire_run_lock(run_dir, ".campaign.lock")
            finally:
                first.close()

    def test_frozen_code_sha_may_be_an_ancestor_when_inputs_are_unchanged(self):
        contract = {
            "code_sha": "a" * 40,
            "frozen_inputs": ["scripts/runner.py", "eval/prompt.txt"],
        }
        completed = subprocess.CompletedProcess([], 0, stdout="", stderr="")
        with mock.patch(
            "eval_lib.goal_verify_live.subprocess.run",
            side_effect=[completed, completed],
        ) as run:
            self.assertEqual(_verify_frozen_git_inputs(ROOT, contract), "a" * 40)
        self.assertEqual(run.call_args_list[0].args[0][-2:], ["a" * 40, "HEAD"])
        self.assertEqual(
            run.call_args_list[1].args[0][-2:],
            ["scripts/runner.py", "eval/prompt.txt"],
        )

    def test_frozen_input_change_is_rejected(self):
        contract = {"code_sha": "a" * 40, "frozen_inputs": ["eval/prompt.txt"]}
        ancestor = subprocess.CompletedProcess([], 0, stdout="", stderr="")
        changed = subprocess.CompletedProcess([], 1, stdout="", stderr="")
        with mock.patch(
            "eval_lib.goal_verify_live.subprocess.run",
            side_effect=[ancestor, changed],
        ), self.assertRaisesRegex(ValueError, "frozen runner or experiment"):
            _verify_frozen_git_inputs(ROOT, contract)

    def test_exact_sha_ci_requires_every_registered_workflow(self):
        with tempfile.TemporaryDirectory(dir=ROOT) as temporary:
            evidence = Path(temporary) / "ci.json"
            evidence.write_text(
                json.dumps(
                    {
                        "head_sha": "a" * 40,
                        "workflows": [
                            {
                                "name": "CI",
                                "status": "completed",
                                "conclusion": "success",
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )
            contract = {
                "exact_sha_ci_evidence": str(evidence.relative_to(ROOT)),
                "required_ci_workflows": ["CI", "acceptance"],
            }
            with self.assertRaisesRegex(ValueError, "absent or non-green"):
                _verify_exact_sha_ci(ROOT, contract, "a" * 40)


if __name__ == "__main__":
    unittest.main()
