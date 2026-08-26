import copy
import json
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_v2 import (
    build_v2_prompt,
    candidate_case_v2,
    canonicalize_v2_proposal,
    classify_oracle_execution,
    concretize_registered_command,
    evaluate_concretized_command,
    evaluate_existing_evidence,
    evaluate_v2_oracles,
    resolve_evidence_reference,
)


class GoalVerifyV2Test(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.corpus = json.loads(
            (ROOT / "eval/goal_verify/v0/corpus.json").read_text(encoding="utf-8")
        )
        cls.create_fixture = json.loads(
            (ROOT / "tests/fixtures/verification_spec_v0/create.json").read_text(
                encoding="utf-8"
            )
        )

    def _single_claim_proposal(self, case):
        proposal = copy.deepcopy(self.create_fixture)
        claim_id = case["required_claims"][0]["id"]
        proposal["goal"] = case["goal"]
        proposal["intent"] = case["intent"]
        proposal["profile"] = case["profile"]
        proposal["claims"] = [proposal["claims"][0]]
        proposal["claims"][0]["id"] = claim_id
        proposal["claims"][0]["oracle_ids"] = ["provider-invented-id"]
        proposal["oracles"] = [proposal["oracles"][0]]
        proposal["oracles"][0]["id"] = "provider-invented-id"
        proposal["oracles"][0]["claim_id"] = claim_id
        return proposal

    def test_prompt_includes_required_ids_and_closed_vocabulary(self):
        case = self.corpus["cases"][0]
        prompt = build_v2_prompt("base", case, "request-v2", "{}")
        self.assertIn(case["required_claims"][0]["id"], prompt)
        self.assertIn('"claim.kind"', prompt)
        self.assertIn('"oracle.input.kind"', prompt)
        self.assertIn("do not rename, omit, or invent claim IDs", prompt)

    def test_fix_registry_uses_claim_specific_requirement_ids(self):
        case = next(
            case
            for case in self.corpus["cases"]
            if case["case_id"] == "fix-reproduced-after-regression"
        )
        prompt = build_v2_prompt("base", case, "request-v2", "{}")
        request = json.loads(prompt.split("INPUT JSON:\n", 1)[1])
        by_claim = {
            row["claim_id"]: row["requirement_id"]
            for row in request["existing_evidence_registry"]
        }
        self.assertEqual(by_claim["before-after"], "before_fails")
        self.assertEqual(by_claim["regressions"], "no_regression")

    def test_host_computes_utf8_range_ids_and_lineage(self):
        case = copy.deepcopy(self.corpus["cases"][1])
        case["required_claims"] = [case["required_claims"][0]]
        proposal = self._single_claim_proposal(case)
        proposal["oracles"][0]["lineage"] = {
            "proposed_binding_sha256": "a" * 64,
            "concretized_binding_sha256": "b" * 64,
            "semantic_equivalence": False,
            "repair_kind": "provider-value",
        }
        canonical = canonicalize_v2_proposal(
            json.dumps(proposal, ensure_ascii=False), case=case
        )
        claim = canonical["claims"][0]
        oracle = canonical["oracles"][0]
        self.assertEqual(claim["origin"]["start_byte"], 0)
        self.assertEqual(claim["origin"]["end_byte"], len(case["goal"].encode("utf-8")))
        self.assertEqual(claim["oracle_ids"], [oracle["id"]])
        self.assertNotEqual(oracle["lineage"]["proposed_binding_sha256"], "a" * 64)
        self.assertEqual(
            oracle["lineage"]["proposed_binding_sha256"],
            oracle["lineage"]["concretized_binding_sha256"],
        )
        self.assertEqual(oracle["lifecycle"], "proposed")
        self.assertEqual(oracle["result"], "unverified")

    def test_host_rejects_missing_or_invented_claim_ids(self):
        case = self.corpus["cases"][0]
        proposal = self._single_claim_proposal(case)
        proposal["claims"][0]["id"] = "invented"
        proposal["oracles"][0]["claim_id"] = "invented"
        with self.assertRaisesRegex(ValueError, "exactly the registered"):
            canonicalize_v2_proposal(json.dumps(proposal), case=case)

    def test_execution_boundary_never_runs_raw_provider_argv(self):
        command = classify_oracle_execution(
            {"id": "o1", "strategy": "command", "setup": {"argv": ["cargo"]}}
        )
        reference = classify_oracle_execution(
            {"id": "o2", "strategy": "existing_fix_evidence"}
        )
        unavailable = classify_oracle_execution({"id": "o3", "strategy": "dom"})
        self.assertEqual(command["lane"], "executable")
        self.assertEqual(reference["lane"], "reference_validation")
        self.assertEqual(unavailable["lane"], "executor_unavailable")
        self.assertFalse(command["may_execute_raw_provider_argv"])

    def test_registered_command_plan_ignores_provider_argv_and_is_evaluated(self):
        with tempfile.TemporaryDirectory(dir=ROOT) as temporary:
            workspace = Path(temporary)
            oracle = {
                "id": "oracle-cli",
                "strategy": "command",
                "setup": {"argv": ["rm", "-rf", "outside"]},
                "timeout_ms": 999,
            }
            adapter = {
                "oracle_id": "oracle-cli",
                "argv": ["sum-cli", "2", "3"],
                "cwd": ".",
                "timeout_ms": 1000,
                "observation": {"kind": "stdout", "expected": "5\n"},
            }
            plan = concretize_registered_command(
                oracle=oracle, adapter=adapter, workspace_root=workspace
            )
            self.assertEqual(plan["argv"], adapter["argv"])
            self.assertNotIn("rm", plan["argv"])
            seen = []

            def fake_sandbox_runner(received):
                seen.append(received)
                return {
                    "exit_code": 0,
                    "stdout": "5\n",
                    "stderr": "",
                    "timed_out": False,
                    "runtime_ms": 12,
                }

            evaluation = evaluate_concretized_command(plan, runner=fake_sandbox_runner)
            self.assertEqual(evaluation["result"], "pass")
            self.assertEqual(evaluation["observed_strength"], "runtime")
            self.assertEqual(len(seen), 1)
            tampered = copy.deepcopy(plan)
            tampered["argv"] = ["other"]
            with self.assertRaisesRegex(ValueError, "integrity"):
                evaluate_concretized_command(tampered, runner=fake_sandbox_runner)

    def test_existing_evidence_requires_exact_provenance_and_outcome(self):
        with tempfile.TemporaryDirectory(dir=ROOT) as temporary:
            execution_root = Path(temporary)
            evidence_dir = execution_root / "evidence"
            evidence_dir.mkdir()
            artifact = evidence_dir / "fix-evidence.json"
            artifact.write_text(
                json.dumps(
                    {
                        "requirement_id": "after_passes",
                        "stage": "after",
                        "expected_polarity": "success",
                        "lineage": "case-lineage",
                        "epoch": 2,
                        "executed": True,
                        "outcome": "success",
                    }
                ),
                encoding="utf-8",
            )
            claim = {
                "origin": {
                    "source_kind": "fix_requirement",
                    "artifact_path": "evidence/fix-evidence.json",
                    "requirement_id": "after_passes",
                    "stage": "after",
                    "expected_polarity": "success",
                    "lineage": "case-lineage",
                    "epoch": 2,
                }
            }
            oracle = {
                "id": "existing-1",
                "strategy": "existing_fix_evidence",
                "observation": {
                    "kind": "existing_binding",
                    "artifact_path": "evidence/fix-evidence.json",
                },
            }
            evaluation = evaluate_existing_evidence(
                claim=claim, oracle=oracle, execution_root=execution_root
            )
            self.assertEqual(evaluation["result"], "pass")
            self.assertEqual(evaluation["observed_strength"], "runtime")
            claim["origin"]["epoch"] = 3
            evaluation = evaluate_existing_evidence(
                claim=claim, oracle=oracle, execution_root=execution_root
            )
            self.assertEqual(evaluation["result"], "unverified")

    def test_v2_candidate_does_not_copy_baseline_authority(self):
        source = self.corpus["cases"][0]
        claim_id = source["required_claims"][0]["id"]
        record = {
            "record_path": "run/raw.json",
            "response": {
                "status": "completed",
                "response": {
                    "total_duration": 2_000_000,
                    "prompt_eval_count": 3,
                    "eval_count": 4,
                },
            },
            "validation": {
                "valid": True,
                "spec": {
                    "claims": [{"id": claim_id}],
                    "oracles": [{"id": "o1", "claim_id": claim_id}],
                },
            },
            "oracle_evaluations": [],
        }
        candidate = candidate_case_v2(source, "pair-v2", record)
        observation = candidate["observation"]
        self.assertEqual(observation["claimed_claim_ids"], [claim_id])
        self.assertEqual(observation["verified_claims"], [])
        self.assertEqual(observation["verdict"], "unverified")
        self.assertFalse(observation["final_acceptance"])
        self.assertEqual(observation["flake_trials"], [])

    def test_oracle_evaluation_fails_closed_without_registered_adapter(self):
        with tempfile.TemporaryDirectory(dir=ROOT) as temporary:
            evaluations = evaluate_v2_oracles(
                spec={
                    "claims": [{"id": "claim-1"}],
                    "oracles": [
                        {
                            "id": "command-1",
                            "claim_id": "claim-1",
                            "strategy": "command",
                        },
                        {
                            "id": "dom-1",
                            "claim_id": "claim-1",
                            "strategy": "dom",
                        },
                    ],
                },
                adapters={},
                execution_root=Path(temporary),
                sandbox_runner=lambda plan: self.fail(f"unexpected execution: {plan}"),
            )
        self.assertEqual(evaluations[0]["result"], "blocked")
        self.assertEqual(evaluations[0]["reason"], "registered_adapter_missing")
        self.assertEqual(evaluations[1]["result"], "unverified")
        self.assertEqual(evaluations[1]["lane"], "executor_unavailable")

    def test_evidence_reference_must_stay_under_execution_root(self):
        with tempfile.TemporaryDirectory(dir=ROOT) as temporary:
            execution_root = Path(temporary)
            evidence = execution_root / "evidence.json"
            evidence.write_text("{}", encoding="utf-8")
            self.assertEqual(
                resolve_evidence_reference(
                    execution_root=execution_root, artifact_path="evidence.json"
                ),
                evidence.resolve(),
            )
            with self.assertRaises(ValueError):
                resolve_evidence_reference(
                    execution_root=execution_root, artifact_path="../outside.json"
                )

    def test_capability_registry_covers_every_required_claim_once(self):
        registry = json.loads(
            (
                ROOT / "eval/goal_verify/v0/phase6-execution-capabilities-v2.json"
            ).read_text(encoding="utf-8")
        )
        expected = {
            (case["case_id"], claim["id"])
            for case in self.corpus["cases"]
            for claim in case["required_claims"]
        }
        registered = [
            (case["case_id"], claim["claim_id"])
            for case in registry["cases"]
            for claim in case["claims"]
        ]
        self.assertEqual(len(registered), len(set(registered)))
        self.assertEqual(set(registered), expected)

    def test_draft_contract_cannot_authorize_a_live_run(self):
        contract = json.loads(
            (ROOT / "eval/goal_verify/v0/phase6-preflight-v2-contract.json").read_text(
                encoding="utf-8"
            )
        )
        self.assertEqual(contract["code_sha"], "TO_BE_FROZEN")
        self.assertFalse(contract["authorization"]["approved_live"])
        self.assertEqual(
            contract["acceptance"]["schema_compliance"]["minimum_passes"], 38
        )
        self.assertEqual(
            contract["proposal_contract"]["selected_intents"], ["create", "fix"]
        )


if __name__ == "__main__":
    unittest.main()
