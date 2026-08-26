import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_artifacts_v2 import build_registered_baseline_spec
from eval_lib.goal_verify_live import run_campaign


class GoalVerifyLiveV2Test(unittest.TestCase):
    def test_v2_runner_persists_independent_same_snapshot_arms(self):
        corpus_path = ROOT / "eval/goal_verify/v0/corpus.json"
        corpus = json.loads(corpus_path.read_text(encoding="utf-8"))
        contract = json.loads(
            (ROOT / "eval/goal_verify/v0/phase6-preflight-v2-contract.json").read_text(
                encoding="utf-8"
            )
        )
        contract["exact_sha_ci_evidence"] = (
            "eval/goal_verify/v0/exact-sha-ci-b8474aad.json"
        )
        adapters = json.loads(
            (ROOT / "eval/goal_verify/v0/phase6-command-adapters-v2.json").read_text(
                encoding="utf-8"
            )
        )["adapters"]
        source = corpus["cases"][0]
        provider_spec = build_registered_baseline_spec(case=source, adapters=adapters)
        raw = json.dumps(provider_spec, ensure_ascii=False)

        def validate_normalized(**kwargs):
            return {
                "valid": True,
                "spec": json.loads(kwargs["normalized_raw"]),
                "errors": [],
            }

        with tempfile.TemporaryDirectory(dir=ROOT) as temporary:
            temporary_root = Path(temporary)
            contract_path = temporary_root / "contract.json"
            contract_path.write_text(json.dumps(contract), encoding="utf-8")
            run_dir = temporary_root / "run"
            execution_root = temporary_root / "execution"
            with (
                mock.patch("eval_lib.goal_verify_live.preflight"),
                mock.patch(
                    "eval_lib.goal_verify_live.request_ollama",
                    return_value={
                        "status": "completed",
                        "response": {
                            "response": raw,
                            "total_duration": 2_000_000,
                            "prompt_eval_count": 3,
                            "eval_count": 4,
                        },
                    },
                ),
                mock.patch(
                    "eval_lib.goal_verify_live.validate_proposal",
                    side_effect=validate_normalized,
                ),
                mock.patch(
                    "eval_lib.goal_verify_live.run_macos_sandbox",
                    return_value={
                        "exit_code": 0,
                        "stdout": "",
                        "stderr": "",
                        "timed_out": False,
                        "runtime_ms": 1,
                    },
                ),
            ):
                summary = run_campaign(
                    root=ROOT,
                    corpus_path=corpus_path,
                    contract_path=contract_path,
                    schema_path=ROOT
                    / "eval/goal_verify/v0/verification-spec.schema.json",
                    prompt_path=ROOT
                    / "eval/goal_verify/v0/verification-spec-preflight-v2.prompt.txt",
                    validator=ROOT / "target/debug/verification_spec_validate",
                    run_dir=run_dir,
                    execution_root=execution_root,
                    limit=1,
                )
            self.assertEqual(summary["completed_pairs"], 1)
            record = json.loads(
                next((run_dir / "raw").glob("**/pair-*.json")).read_text(
                    encoding="utf-8"
                )
            )
            self.assertTrue(record["baseline_spec"])
            self.assertTrue(record["baseline_oracle_evaluations"])
            self.assertTrue(record["oracle_evaluations"])
            self.assertTrue(
                all(
                    row["arm"] == "baseline"
                    for row in record["baseline_oracle_evaluations"]
                )
            )
            self.assertTrue(
                all(row["arm"] == "candidate" for row in record["oracle_evaluations"])
            )
            baseline_corpus = json.loads(
                (run_dir / "baseline-corpus.json").read_text(encoding="utf-8")
            )
            baseline = baseline_corpus["cases"][0]
            candidate_corpus = json.loads(
                (run_dir / "candidate-corpus.draft.json").read_text(encoding="utf-8")
            )
            candidate = candidate_corpus["cases"][0]
            self.assertEqual(baseline["observation"]["verdict"], "unverified")
            self.assertEqual(candidate["observation"]["verdict"], "unverified")
            self.assertFalse(
                baseline["preflight_only"]["product_task_success_evidence"]
            )
            self.assertFalse(
                candidate["preflight_only"]["product_task_success_evidence"]
            )
            self.assertIn(
                "synthetic snapshot", baseline_corpus["annotation_protocol"]["method"]
            )
            self.assertIn(
                "identical synthetic snapshot",
                candidate_corpus["annotation_protocol"]["method"],
            )


if __name__ == "__main__":
    unittest.main()
