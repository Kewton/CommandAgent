import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_baseline_product_v3 import (
    extract_product_observations,
    score_baseline_observations,
)
from eval_lib.goal_verify_blind_v3 import (
    build_blind_review_report,
    cohen_kappa,
    human_sample,
    prepare_blind_items,
    records_to_blind_inputs,
)
from eval_lib.goal_verify_executors_v3 import execute_registered
from eval_lib.goal_verify_live_v3 import run_campaign_v3, verify_live_inputs_v3
from eval_lib.goal_verify_observation_match_v3 import (
    evaluate_candidate_spec,
    proposal_matches_adapter,
    score_claim_coverage,
)
from eval_lib.goal_verify_preflight_v3 import (
    cross_source_errors,
    exact_sha_ci_evidence_errors,
    readiness_report,
)
from eval_lib.goal_verify_v3 import (
    build_conformance_prompt,
    build_held_out_prompt,
    canonicalize_held_out_proposal,
    load_prompt_from_contract,
    regeneration_seed,
    should_regenerate,
)
from eval_lib.goal_verify_workspaces_v3 import (
    load_workspace_registry,
    prepare_workspace_stage,
    validate_workspace_registry,
    workspace_by_case,
)


def load(relative):
    return json.loads((ROOT / relative).read_text(encoding="utf-8"))


class PromptAndCanonicalizationTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.corpus = load("eval/goal_verify/v0/corpus.json")
        cls.adapters = load("eval/goal_verify/v0/phase6-command-adapters-v3.json")
        cls.capabilities = load(
            "eval/goal_verify/v0/phase6-execution-capabilities-v3.json"
        )
        cls.base = (
            ROOT / "eval/goal_verify/v0/verification-spec-preflight-v3.prompt.txt"
        ).read_text(encoding="utf-8")
        cls.shape = (
            ROOT / "tests/fixtures/verification_spec_v0/create.json"
        ).read_text(encoding="utf-8")

    def case(self, case_id):
        return next(row for row in self.corpus["cases"] if row["case_id"] == case_id)

    def test_conformance_input_contains_registered_ids_and_adapter_units(self):
        case = self.case("create-cli-known-multiple-inputs")
        prompt = build_conformance_prompt(
            self.base,
            case,
            "request-1",
            self.shape,
            adapters=self.adapters["adapters"],
        )
        payload = json.loads(prompt.rsplit("INPUT JSON:\n", 1)[1])
        self.assertEqual(payload["required_claims"][0]["id"], "cli-known-values")
        self.assertEqual(len(payload["required_claims"][0]["expected_observations"]), 2)

    def test_held_out_input_omits_claim_ids_adapter_values_and_snapshot(self):
        case = self.case("fix-reproduced-after-regression")
        prompt = build_held_out_prompt(
            self.base,
            case,
            "request-2",
            self.shape,
            capabilities=self.capabilities,
        )
        payload_text = prompt.rsplit("INPUT JSON:\n", 1)[1]
        payload = json.loads(payload_text)
        self.assertNotIn("required_claims", payload)
        self.assertNotIn("expected_observations", payload_text)
        self.assertNotIn("adapter_id", payload_text)
        self.assertNotIn("snapshot", payload_text)
        self.assertTrue(payload["executor_capabilities"])
        self.assertNotIn("claim_id", payload["existing_evidence_registry"][0])

    def test_held_out_host_replaces_provider_ids_deterministically(self):
        case = self.case("create-build-only-functional")
        proposal = load("tests/fixtures/verification_spec_v0/create.json")
        proposal["goal"] = "rewritten"
        normalized_a = canonicalize_held_out_proposal(
            json.dumps(proposal), case=case, model="m", request_id="r"
        )
        normalized_b = canonicalize_held_out_proposal(
            json.dumps(proposal), case=case, model="m", request_id="r"
        )
        self.assertEqual(normalized_a, normalized_b)
        value = json.loads(normalized_a)
        self.assertTrue(value["claims"][0]["id"].startswith("held-"))
        self.assertEqual(value["goal"], case["goal"])
        self.assertEqual(value["claims"][0]["origin"]["start_byte"], 0)
        self.assertEqual(
            value["claims"][0]["origin"]["end_byte"],
            len(case["goal"].encode("utf-8")),
        )

    def test_held_out_fix_origin_is_intent_compatible_and_deterministic(self):
        case = self.case("fix-reproduced-after-regression")
        proposal = load("tests/fixtures/verification_spec_v0/fix.json")
        proposal["claims"][0]["id"] = "temporary-provider-id"
        proposal["claims"][0]["kind"] = "regression"
        proposal["oracles"][0]["claim_id"] = "temporary-provider-id"
        normalized_a = canonicalize_held_out_proposal(
            json.dumps(proposal), case=case, model="m", request_id="r"
        )
        normalized_b = canonicalize_held_out_proposal(
            json.dumps(proposal), case=case, model="m", request_id="r"
        )
        self.assertEqual(normalized_a, normalized_b)
        origin = json.loads(normalized_a)["claims"][0]["origin"]
        self.assertEqual(origin["source_kind"], "fix_requirement")
        self.assertEqual(origin["requirement_id"], "before_fails")
        self.assertEqual(origin["stage"], "before")
        self.assertEqual(origin["expected_polarity"], "failure")

    def test_preflight_prompt_fixes_kind_fields_and_allow_tables(self):
        for required in (
            "Claim-kind allow table by intent",
            "Input-kind field table",
            'http -> {"kind":"http","method":"GET or HEAD","port":4173',
            'dom -> {"kind":"dom","route":"/absolute-route-path"',
            "Observation-kind field table",
            "Strategy/input/observation allow table",
            "require setup.argv with",
        ):
            self.assertIn(required, self.base)

    def test_prompt_path_mismatch_aborts(self):
        contract = load("eval/goal_verify/v0/phase6-preflight-v3-contract.json")
        with self.assertRaisesRegex(ValueError, "CLI prompt differs"):
            load_prompt_from_contract(
                root=ROOT,
                contract=contract,
                cli_prompt=ROOT / "eval/goal_verify/v0/verification-spec.prompt.txt",
            )

    def test_regeneration_is_one_validator_triggered_attempt(self):
        invalid = {"valid": False, "errors": ["expected u16"]}
        self.assertTrue(should_regenerate(invalid, 1))
        self.assertFalse(should_regenerate(invalid, 2))
        self.assertFalse(
            should_regenerate({"valid": False, "errors": ["provider_error"]}, 1)
        )
        self.assertEqual(regeneration_seed(100, 4, "contract_conformance", 1), 104)
        self.assertEqual(regeneration_seed(100, 4, "held_out_synthesis", 2), 1105)


class ExecutorAndScoringTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.corpus = load("eval/goal_verify/v0/corpus.json")
        cls.adapters = load("eval/goal_verify/v0/phase6-command-adapters-v3.json")[
            "adapters"
        ]

    def test_registered_command_uses_injected_runner(self):
        executor = next(
            row["executor"]
            for row in self.adapters
            if row["adapter_id"] == "cli-known-values-2-3"
        )
        calls = []

        def runner(argv, cwd, timeout):
            calls.append((argv, cwd, timeout))
            return {"exit_code": 0, "stdout": "5\n", "stderr": "", "runtime_ms": 1}

        result = execute_registered(executor, workspace=ROOT, runner=runner)
        self.assertEqual(result["result"], "pass")
        self.assertEqual(calls[0][0], ["python3", "sum_cli.py", "2", "3"])

    def test_regression_set_requires_every_registered_id(self):
        executor = next(
            row["executor"]
            for row in self.adapters
            if row["adapter_id"] == "regressions"
        )
        outcomes = iter((0, 1))

        def runner(argv, cwd, timeout):
            return {
                "exit_code": next(outcomes),
                "stdout": "",
                "stderr": "",
                "runtime_ms": 1,
            }

        result = execute_registered(executor, workspace=ROOT, runner=runner)
        self.assertEqual(result["result"], "fail")
        self.assertEqual(len(result["registered_results"]), 2)

    def test_registered_command_runner_error_is_fail_closed(self):
        executor = next(
            row["executor"]
            for row in self.adapters
            if row["adapter_id"] == "cli-known-values-2-3"
        )

        def runner(argv, cwd, timeout):
            return {
                "runner_error": "sandbox_backend_unavailable",
                "exit_code": None,
                "runtime_ms": 0,
            }

        result = execute_registered(executor, workspace=ROOT, runner=runner)
        self.assertFalse(result["executed"])
        self.assertEqual(result["result"], "oracle_error")
        self.assertEqual(result["reason"], "sandbox_backend_unavailable")

    def test_regression_set_runner_error_is_not_a_test_failure(self):
        executor = next(
            row["executor"]
            for row in self.adapters
            if row["adapter_id"] == "regressions"
        )

        def runner(argv, cwd, timeout):
            return {
                "runner_error": "sandbox_backend_unavailable",
                "exit_code": None,
                "runtime_ms": 0,
            }

        result = execute_registered(executor, workspace=ROOT, runner=runner)
        self.assertFalse(result["executed"])
        self.assertEqual(result["result"], "oracle_error")
        self.assertEqual(result["reason"], "registered_executor_error")

    def test_observation_match_can_ignore_claim_id_only_in_held_out(self):
        adapter = next(
            row for row in self.adapters if row["adapter_id"] == "cli-known-values-2-3"
        )
        oracle = {
            "claim_id": "provider-name",
            "strategy": "stdout",
            "expected_polarity": "success",
            "observation": {"kind": "stdout", "expected": "5"},
        }
        self.assertFalse(
            proposal_matches_adapter(oracle, adapter, compare_claim_id=True)
        )
        self.assertTrue(
            proposal_matches_adapter(oracle, adapter, compare_claim_id=False)
        )

    def test_candidate_execution_never_uses_provider_argv(self):
        adapter = next(
            row for row in self.adapters if row["adapter_id"] == "cli-known-values-2-3"
        )
        spec = {
            "oracles": [
                {
                    "id": "o1",
                    "claim_id": "arbitrary",
                    "strategy": "stdout",
                    "expected_polarity": "success",
                    "setup": {"argv": ["bash", "-c", "touch /tmp/bad"]},
                    "input": {"kind": "none"},
                    "observation": {"kind": "stdout", "expected": "5"},
                }
            ]
        }
        seen = []

        def execute(executor, workspace):
            seen.append(executor["argv"])
            return {"executed": True, "result": "pass", "actual": "5\n"}

        result = evaluate_candidate_spec(
            case_id=adapter["case_id"],
            spec=spec,
            adapters=self.adapters,
            workspaces={(adapter["case_id"], "reference"): ROOT},
            lane="held_out_synthesis",
            executor=execute,
        )
        self.assertEqual(seen, [adapter["executor"]["argv"]])
        self.assertTrue(result["evaluations"][0]["observation_match"])

    def test_claim_coverage_requires_all_registered_entries(self):
        case = next(
            row
            for row in self.corpus["cases"]
            if row["case_id"] == "create-cli-known-multiple-inputs"
        )
        one = [
            {
                "adapter_id": "cli-known-values-2-3",
                "observation_match": True,
                "observed_strength": "runtime",
            }
        ]
        score = score_claim_coverage(case=case, adapters=self.adapters, evaluations=one)
        self.assertEqual(score["claims"][0]["status"], "weak")


class WorkspaceBaselineBlindAndReadinessTest(unittest.TestCase):
    def test_live_input_verification_rejects_wrong_binary_commit(self):
        contract = load("eval/goal_verify/v0/phase6-preflight-v3-contract.json")
        completed = subprocess.CompletedProcess(
            args=[], returncode=0, stdout="commandagent 0.1.0 deadbeef now\n", stderr=""
        )
        with (
            mock.patch("subprocess.run", return_value=completed),
            self.assertRaisesRegex(ValueError, "clean frozen code SHA"),
        ):
            verify_live_inputs_v3(
                root=ROOT,
                contract=contract,
                commandagent_bin=ROOT / "target/release/commandagent",
                validator=ROOT / "target/release/verification_spec_validate",
            )

    def test_exact_sha_ci_evidence_requires_matching_successful_workflows(self):
        code_sha = "a" * 40
        contract = {
            "code_sha": code_sha,
            "exact_sha_ci_evidence": "evidence.json",
        }
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            evidence = {
                "head_sha": code_sha,
                "workflows": [
                    {
                        "name": "CI",
                        "head_sha": code_sha,
                        "status": "completed",
                        "conclusion": "success",
                    },
                    {
                        "name": "acceptance",
                        "head_sha": code_sha,
                        "status": "completed",
                        "conclusion": "success",
                    },
                ],
            }
            (root / "evidence.json").write_text(
                json.dumps(evidence), encoding="utf-8"
            )
            self.assertEqual(
                exact_sha_ci_evidence_errors(root=root, contract=contract), []
            )
            evidence["head_sha"] = "b" * 40
            evidence["workflows"][1]["conclusion"] = "failure"
            (root / "evidence.json").write_text(
                json.dumps(evidence), encoding="utf-8"
            )
            self.assertEqual(
                exact_sha_ci_evidence_errors(root=root, contract=contract),
                [
                    "exact_sha_ci_evidence_sha_mismatch",
                    "exact_sha_ci_workflow_not_successful:acceptance",
                ],
            )

    def test_workspace_registry_and_copy(self):
        registry = load_workspace_registry(
            ROOT / "eval/goal_verify/v0/phase6-real-workspaces-v3.json"
        )
        self.assertEqual(validate_workspace_registry(root=ROOT, registry=registry), [])
        case = workspace_by_case(registry)["create-cli-known-multiple-inputs"]
        with tempfile.TemporaryDirectory() as temporary:
            copied = prepare_workspace_stage(
                root=ROOT,
                workspace=case,
                stage="reference",
                destination=Path(temporary) / "copy",
            )
            self.assertTrue((copied / "sum_cli.py").is_file())

    def test_baseline_parser_is_deterministic_and_fails_closed_on_replay_drift(self):
        with tempfile.TemporaryDirectory() as temporary:
            run = Path(temporary)
            (run / "events.jsonl").write_text(
                json.dumps(
                    {
                        "event": "runtime_bash_verify_command",
                        "argv": ["python3", "sum_cli.py", "2", "3"],
                        "exit_code": 0,
                    }
                )
                + "\n",
                encoding="utf-8",
            )

            def replay(argv, cwd):
                return {"exit_code": 1, "stdout": "5\n", "stderr": ""}

            first = extract_product_observations(run, replay=replay)
            second = extract_product_observations(run, replay=replay)
            self.assertEqual(first, second)
            self.assertFalse(first[0]["passed"])
            self.assertEqual(first[0]["reason"], "baseline_observation_inconsistent")

    def test_baseline_parser_does_not_treat_normalization_as_execution(self):
        with tempfile.TemporaryDirectory() as temporary:
            run = Path(temporary)
            (run / "events.jsonl").write_text(
                json.dumps(
                    {
                        "event": "verify_command_normalized_at_runtime",
                        "original": "python sum_cli.py 2 3",
                        "repaired": "python3 sum_cli.py 2 3",
                    }
                )
                + "\n",
                encoding="utf-8",
            )
            self.assertEqual(extract_product_observations(run), [])

    def test_baseline_scoring_uses_observation_not_claim_id(self):
        adapters = load("eval/goal_verify/v0/phase6-command-adapters-v3.json")[
            "adapters"
        ]
        observations = [
            {
                "strategy": "stdout",
                "kind": "stdout",
                "actual": "5",
                "stdout": "5\n",
                "executed": True,
                "passed": True,
                "strength": "runtime",
            }
        ]
        rows = score_baseline_observations(
            observations, adapters, case_id="create-cli-known-multiple-inputs"
        )
        self.assertTrue(rows[0]["observation_match"])
        self.assertNotIn("baseline_claim_id", rows[0])

    def test_blind_hidden_lane_removes_execution_and_mapping_is_separate(self):
        records = []
        primary = ["a", "b", "c", "d", "e", "f", "g"]
        for case in primary:
            for index in (1, 2):
                records.append(
                    {
                        "pair_id": f"{case}--pair-{index:02d}",
                        "goal": "g",
                        "intent": "create",
                        "profile": "generic",
                        "required_claims": [{"id": "c", "min_strength": "runtime"}],
                        "baseline_card": {"arm": "baseline", "execution_results": [1]},
                        "candidate_card": {
                            "arm": "candidate",
                            "execution_results": [2],
                        },
                    }
                )
        items, mapping = prepare_blind_items(
            records=records, contract_sha256="a" * 64, lane="semantic_hidden"
        )
        self.assertNotIn("execution_results", items[0]["variant_a"])
        self.assertNotIn("arm", items[0]["variant_a"])
        self.assertTrue(mapping)
        self.assertEqual(len(human_sample(items, primary)), 10)

    def test_blind_candidate_uses_raw_not_host_canonicalized_proposal(self):
        raw = {
            "claims": [{"id": "provider-claim"}],
            "oracles": [{"id": "provider-oracle", "claim_id": "provider-claim"}],
        }
        record = {
            "pair_id": "case--pair-01",
            "goal": "g",
            "intent": "create",
            "profile": "generic",
            "required_claims": [{"id": "gold", "min_strength": "runtime"}],
            "baseline": {"coverage": {}, "observations": [], "evaluations": []},
            "lanes": {
                "held_out_synthesis": {
                    "attempts": [
                        {
                            "response": {
                                "status": "completed",
                                "response": {"response": json.dumps(raw)},
                            }
                        }
                    ],
                    "validation": {
                        "valid": True,
                        "spec": {"claims": [{"id": "host-rewritten"}], "oracles": []},
                    },
                    "execution": {"evaluations": []},
                }
            },
        }
        candidate = records_to_blind_inputs([record])[0]["candidate_card"]
        self.assertEqual(candidate["claims"][0]["id"], "provider-claim")
        self.assertEqual(candidate["parse_status"], "parsed")

    def test_blind_report_requires_provenance_and_passes_agreement_gates(self):
        items = [
            {"item_id": f"semantic_hidden:case--pair-{index:02d}"}
            for index in range(1, 11)
        ]
        mapping = {
            f"{item['item_id']}:variant_a": "candidate" for item in items
        }
        mapping.update(
            {f"{item['item_id']}:variant_b": "baseline" for item in items}
        )
        reviews = [
            {
                "item_id": item["item_id"],
                "preferred": "variant_a",
                "reviewer_id": "human-1",
                "reviewed_at": "2026-08-26T00:00:00Z",
                "reason_codes": ["better_coverage"],
                "rationale": "candidate covers the required claim",
            }
            for item in items
        ]
        items_sha256 = hashlib.sha256(
            json.dumps(
                items,
                ensure_ascii=False,
                separators=(",", ":"),
                sort_keys=True,
            ).encode()
        ).hexdigest()
        common = {
            "provider": "ollama",
            "invoked_at": "2026-08-26T00:00:00Z",
            "items_sha256": items_sha256,
            "raw_response": "raw",
            "parsed_reviews": reviews,
            "invocation_script_sha256": "b" * 64,
            "independent": True,
        }
        report = build_blind_review_report(
            items=items,
            mapping=mapping,
            model_reviews=[
                {**common, "model_id_or_version": "m1", "model_family": "family-a"},
                {**common, "model_id_or_version": "m2", "model_family": "family-b"},
            ],
            human_review={"reviews": reviews},
            required_human_ids=[item["item_id"] for item in items],
        )
        self.assertTrue(report["semantic_blind_review_complete"])
        self.assertEqual(cohen_kappa(["a", "b"], ["a", "b"])["kappa"], 1.0)

    def test_frozen_contract_is_blocked_without_provisioned_execution_root(self):
        contract = load("eval/goal_verify/v0/phase6-preflight-v3-contract.json")
        self.assertEqual(cross_source_errors(root=ROOT, contract=contract), [])
        report = readiness_report(
            root=ROOT,
            contract_path=ROOT
            / "eval/goal_verify/v0/phase6-preflight-v3-contract.json",
        )
        self.assertFalse(report["ready"])
        self.assertNotIn("contract_not_frozen", report["blockers"])
        self.assertNotIn("exact_code_sha_missing", report["blockers"])
        self.assertNotIn("exact_sha_ci_evidence_missing", report["blockers"])
        self.assertNotIn("live_preflight_not_authorized", report["blockers"])
        self.assertNotIn("independent_human_reviewer_missing", report["blockers"])
        self.assertEqual(report["pending_executor_adapters"], [])
        self.assertIn(
            "provisioning_root_missing:create-build-only-functional",
            report["blockers"],
        )

    def test_v3_live_runner_records_two_lanes_and_one_shared_baseline(self):
        corpus_path = ROOT / "eval/goal_verify/v0/corpus.json"
        contract = load("eval/goal_verify/v0/phase6-preflight-v3-contract.json")
        contract["generation"]["seed_base"] = 399_000
        contract["selected_cells"] = [
            row
            for row in contract["selected_cells"]
            if row["case_id"] == "create-cli-known-multiple-inputs"
        ]
        source = next(
            row
            for row in load("eval/goal_verify/v0/corpus.json")["cases"]
            if row["case_id"] == "create-cli-known-multiple-inputs"
        )
        fixture = load("tests/fixtures/verification_spec_v0/create.json")
        fixture["goal"] = source["goal"]
        fixture["profile"] = source["profile"]
        fixture["claims"][0]["id"] = source["required_claims"][0]["id"]
        fixture["oracles"][0]["claim_id"] = source["required_claims"][0]["id"]
        raw = json.dumps(fixture)
        baseline_calls = []

        def provider(**kwargs):
            return {
                "status": "completed",
                "response": {
                    "response": raw,
                    "prompt_eval_count": 1,
                    "eval_count": 2,
                },
            }

        def baseline_runner(**kwargs):
            baseline_calls.append(kwargs)
            return {"status": "completed", "product_run_dir": None, "wall_time_ms": 1}

        def validation(**kwargs):
            return {
                "valid": True,
                "spec": json.loads(kwargs["normalized_raw"]),
                "errors": [],
            }

        with tempfile.TemporaryDirectory(dir=ROOT) as temporary:
            temporary_root = Path(temporary)
            contract_path = temporary_root / "contract.json"
            contract_path.write_text(json.dumps(contract), encoding="utf-8")
            run_dir = temporary_root / contract["contract_id"]
            with (
                mock.patch(
                    "eval_lib.goal_verify_live_v3.readiness_report",
                    return_value={"ready": True, "blockers": []},
                ),
                mock.patch(
                    "eval_lib.goal_verify_live_v3.validate_proposal",
                    side_effect=validation,
                ),
                mock.patch(
                    "eval_lib.goal_verify_live_v3.evaluate_candidate_spec",
                    return_value={"evaluations": [], "scoring_coverage": True},
                ),
                mock.patch(
                    "eval_lib.goal_verify_live_v3.verify_live_inputs_v3",
                    return_value={"commandagent_binary_sha256": "a" * 64},
                ),
            ):
                summary = run_campaign_v3(
                    root=ROOT,
                    corpus_path=corpus_path,
                    contract_path=contract_path,
                    schema_path=ROOT
                    / contract["generation"]["structured_output_schema"],
                    prompt_path=None,
                    validator=ROOT / "target/release/verification_spec_validate",
                    run_dir=run_dir,
                    execution_root=temporary_root / "execution",
                    limit=1,
                    provider=provider,
                    baseline_runner=baseline_runner,
                )
            record = json.loads(
                (
                    run_dir
                    / "raw/create-cli-known-multiple-inputs/pair-01.json"
                ).read_text(encoding="utf-8")
            )
            self.assertEqual(
                sorted(record["lanes"]), ["contract_conformance", "held_out_synthesis"]
            )
            self.assertEqual(len(baseline_calls), 1)
            self.assertEqual(summary["completed_pairs"], 1)
            self.assertEqual(summary["completed_proposals"], 2)


if __name__ == "__main__":
    unittest.main()
