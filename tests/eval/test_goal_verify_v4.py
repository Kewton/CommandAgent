import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_additive_v4 import (
    candidate_visible_manifest,
    combine_evaluations,
    concretize_candidate_oracle,
    evaluate_candidate_spec_v4,
    execute_candidate_plan,
    score_candidate_outcomes,
    workspace_manifest,
)
from eval_lib.goal_verify_live_v4 import (
    _build_prompt_v4,
    _validate_proposal_v4,
    run_campaign_v4,
)
from eval_lib.goal_verify_preflight_report_v4 import build_report
from eval_lib.goal_verify_preflight_v4 import design_errors, readiness_report
from eval_lib.goal_verify_sandbox import _sandbox_profile, sandbox_backend_status


def load(relative):
    return json.loads((ROOT / relative).read_text(encoding="utf-8"))


class WorkspaceAndConcretizationTest(unittest.TestCase):
    def test_candidate_sandbox_read_scope_does_not_allow_global_reads(self):
        with tempfile.TemporaryDirectory() as temporary:
            profile = _sandbox_profile(
                Path(temporary), restricted_reads=True, argv0="python3"
            )
            self.assertIn(f'(deny file-read-data (subpath "{Path.home()}"))', profile)
            self.assertIn(
                f'(allow file-read-data (subpath "{Path(temporary).resolve()}"))',
                profile,
            )

    def test_manifest_is_deterministic_and_hides_runtime_state(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "app.py").write_text("print('ok')\n", encoding="utf-8")
            (root / ".anvil").mkdir()
            (root / ".anvil/events.jsonl").write_text("secret", encoding="utf-8")
            first = workspace_manifest(root)
            second = workspace_manifest(root)
            self.assertEqual(first, second)
            self.assertEqual([row["path"] for row in first["entries"]], ["app.py"])
            visible = candidate_visible_manifest(first)
            self.assertNotIn("secret", json.dumps(visible))

    def test_concretizer_uses_candidate_argv_without_gold(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "app.py").write_text("print('ok')\n", encoding="utf-8")
            concrete = concretize_candidate_oracle(
                oracle={
                    "strategy": "stdout",
                    "setup": {"argv": ["python3", "app.py"], "cwd": "."},
                    "input": {"kind": "none"},
                    "observation": {"kind": "stdout", "expected": "ok\n"},
                },
                claim={"origin": {"source_kind": "goal"}},
                manifest=workspace_manifest(root),
            )
            self.assertEqual(concrete["classification"], "executable")
            self.assertEqual(concrete["plan"]["argv"], ["python3", "app.py"])
            self.assertFalse(concrete["gold_used_for_concretization"])

    def test_concretizer_rejects_shell_and_fixture_hash_drift(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            fixture = root / "fixture.json"
            fixture.write_text("{}\n", encoding="utf-8")
            manifest = workspace_manifest(root)
            shell = concretize_candidate_oracle(
                oracle={
                    "strategy": "command",
                    "setup": {"argv": ["sh", "-c", "echo bad"], "cwd": "."},
                    "input": {"kind": "none"},
                    "observation": {"kind": "exit_code", "expected": 0},
                },
                claim={"origin": {}},
                manifest=manifest,
            )
            self.assertEqual(shell["classification"], "policy_rejected")
            inline = concretize_candidate_oracle(
                oracle={
                    "strategy": "command",
                    "setup": {"argv": ["python3", "-c", "print('bad')"], "cwd": "."},
                    "input": {"kind": "none"},
                    "observation": {"kind": "exit_code", "expected": 0},
                },
                claim={"origin": {}},
                manifest=manifest,
            )
            self.assertEqual(inline["reason"], "argv_inline_code_unsafe")
            drift = concretize_candidate_oracle(
                oracle={
                    "strategy": "command",
                    "setup": {"argv": ["python3", "app.py"], "cwd": "."},
                    "input": {
                        "kind": "fixture",
                        "path": "fixture.json",
                        "sha256": "0" * 64,
                    },
                    "observation": {"kind": "exit_code", "expected": 0},
                },
                claim={"origin": {}},
                manifest=manifest,
            )
            self.assertEqual(drift["reason"], "fixture_hash_mismatch")

    def test_interaction_concretizer_requires_self_contained_browser_plan(self):
        concrete = concretize_candidate_oracle(
            oracle={
                "strategy": "interaction",
                "setup": {
                    "argv": ["npm", "run", "dev", "--", "-p", "4174"],
                    "cwd": ".",
                },
                "input": {
                    "kind": "dom",
                    "port": 4174,
                    "route": "/",
                    "selector": "#count",
                    "actions": [
                        {"kind": "click", "selector": "#increment", "repeat": 2}
                    ],
                },
                "observation": {"kind": "interaction", "expected": "2"},
                "timeout_ms": 30000,
            },
            claim={"origin": {"source_kind": "goal"}},
            manifest={"entries": []},
        )
        self.assertEqual(concrete["classification"], "executable")
        self.assertEqual(concrete["plan"]["actions"][0]["repeat"], 2)
        missing_actions = concretize_candidate_oracle(
            oracle={
                "strategy": "interaction",
                "setup": {
                    "argv": ["npm", "run", "dev", "--", "-p", "4174"],
                    "cwd": ".",
                },
                "input": {
                    "kind": "dom",
                    "port": 4174,
                    "route": "/",
                    "selector": "#count",
                },
                "observation": {"kind": "interaction", "expected": "2"},
            },
            claim={"origin": {"source_kind": "goal"}},
            manifest={"entries": []},
        )
        self.assertEqual(missing_actions["reason"], "interaction_actions_required")

    def test_http_candidate_executes_host_validated_web_plan(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            captured = []

            def web_runner(plan):
                captured.append(plan)
                return {
                    "executed": True,
                    "result": "pass",
                    "reason": "observation_match",
                    "actual": 200,
                    "observed_strength": "runtime",
                }

            result = execute_candidate_plan(
                {
                    "kind": "http_probe",
                    "server_argv": ["python3", "-m", "http.server", "4175"],
                    "cwd": ".",
                    "port": 4175,
                    "ready_path": "/",
                    "path": "/",
                    "method": "GET",
                    "timeout_ms": 5000,
                    "expected": 200,
                },
                workspace=root,
                web_runner=web_runner,
                browser_toolchain=root,
            )
            self.assertEqual(result["result"], "pass")
            self.assertEqual(captured[0]["source"], "host_validated_candidate_web_v4")
            self.assertTrue(Path(captured[0]["server_argv"][0]).is_absolute())
            self.assertFalse(captured[0]["raw_provider_argv_used"])

    def test_candidate_sandbox_denies_sibling_file_read(self):
        if not sandbox_backend_status()["available"]:
            self.skipTest(
                "macOS sandbox backend unavailable in this execution boundary"
            )
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            workspace = root / "workspace"
            workspace.mkdir()
            allowed = workspace / "allowed.txt"
            allowed.write_text("workspace-readable\n", encoding="utf-8")
            secret = root / "outside-secret.txt"
            secret.write_text("must-not-be-readable", encoding="utf-8")
            allowed_result = execute_candidate_plan(
                {
                    "kind": "command",
                    "argv": ["cat", "allowed.txt"],
                    "cwd": ".",
                    "timeout_ms": 5000,
                    "observation": {
                        "kind": "stdout",
                        "expected": "workspace-readable\n",
                    },
                },
                workspace=workspace,
            )
            self.assertEqual(allowed_result["result"], "pass", allowed_result)
            result = execute_candidate_plan(
                {
                    "kind": "command",
                    "argv": ["cat", str(secret)],
                    "cwd": ".",
                    "timeout_ms": 5000,
                    "observation": {"kind": "exit_code", "expected": 0},
                },
                workspace=workspace,
            )
            self.assertEqual(result["result"], "fail", result)
            self.assertNotEqual(result.get("actual"), 0)
            self.assertNotIn("must-not-be-readable", result.get("stdout", ""))

    def test_candidate_execution_requires_frozen_product_snapshot(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "app.py").write_text("print('ok')\n", encoding="utf-8")
            manifest = workspace_manifest(root)
            spec = {
                "claims": [{"id": "c1", "origin": {"source_kind": "goal"}}],
                "oracles": [
                    {
                        "id": "o1",
                        "claim_id": "c1",
                        "strategy": "stdout",
                        "setup": {"argv": ["python3", "app.py"], "cwd": "."},
                        "input": {"kind": "none"},
                        "observation": {"kind": "stdout", "expected": "ok\n"},
                    }
                ],
            }

            def runner(plan):
                self.assertEqual(plan["source"], "host_validated_candidate_v4")
                self.assertTrue(Path(plan["argv"][0]).is_absolute())
                self.assertEqual(plan["argv"][1:], ["app.py"])
                return {
                    "exit_code": 0,
                    "stdout": "ok\n",
                    "stderr": "",
                    "timed_out": False,
                }

            result = evaluate_candidate_spec_v4(
                spec=spec,
                workspaces={"product": root},
                frozen_snapshot_sha256={"product": manifest["snapshot_sha256"]},
                runner=runner,
            )
            self.assertTrue(result["same_snapshot"])
            self.assertEqual(result["reference_fallback_count"], 0)
            self.assertEqual(result["gold_used_for_execution_count"], 0)
            self.assertEqual(result["evaluations"][0]["result"], "pass")
            self.assertTrue(result["evaluations"][0]["execution_attempt_recorded"])

            (root / "app.py").write_text("print('changed')\n", encoding="utf-8")
            drift = evaluate_candidate_spec_v4(
                spec=spec,
                workspaces={"product": root},
                frozen_snapshot_sha256={"product": manifest["snapshot_sha256"]},
                runner=runner,
            )
            self.assertEqual(
                drift["evaluations"][0]["reason"], "snapshot_hash_mismatch:product"
            )


class UnionAndVerdictTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        corpus = load("eval/goal_verify/v0/corpus.json")
        cls.case = next(
            row
            for row in corpus["cases"]
            if row["case_id"] == "create-cli-known-multiple-inputs"
        )
        cls.adapters = load("eval/goal_verify/v0/phase6-command-adapters-v3.json")[
            "adapters"
        ]

    def test_union_adds_candidate_without_replacing_baseline(self):
        baseline = []
        candidate = [
            {
                "adapter_id": "cli-known-values-2-3",
                "classification": "executable",
                "executed": True,
                "result": "pass",
                "observation_match": True,
                "observed_strength": "runtime",
            },
            {
                "adapter_id": "cli-known-values-neg1-1",
                "classification": "executable",
                "executed": True,
                "result": "pass",
                "observation_match": True,
                "observed_strength": "runtime",
            },
        ]
        result = combine_evaluations(
            case=self.case,
            adapters=self.adapters,
            baseline_evaluations=baseline,
            candidate_evaluations=candidate,
            baseline_status="completed",
        )
        self.assertEqual(result["baseline_score"]["claims"][0]["status"], "unverified")
        self.assertEqual(result["combined_score"]["claims"][0]["status"], "strong")
        self.assertEqual(result["paired_delta"]["recovered_claim_count"], 1)
        self.assertEqual(result["shadow_verdict"], "pass")

    def test_candidate_never_overrides_baseline_failure(self):
        candidate = [
            {
                "adapter_id": "cli-known-values-2-3",
                "classification": "executable",
                "executed": True,
                "result": "pass",
                "observation_match": True,
                "observed_strength": "runtime",
            },
            {
                "adapter_id": "cli-known-values-neg1-1",
                "classification": "executable",
                "executed": True,
                "result": "pass",
                "observation_match": True,
                "observed_strength": "runtime",
            },
        ]
        result = combine_evaluations(
            case=self.case,
            adapters=self.adapters,
            baseline_evaluations=[],
            candidate_evaluations=candidate,
            baseline_status="failed",
        )
        self.assertEqual(result["shadow_verdict"], "failure")
        self.assertFalse(result["baseline_failure_overridden"])

    def test_required_candidate_failure_tightens_shadow_verdict(self):
        oracle = {
            "id": "o1",
            "claim_id": self.case["required_claims"][0]["id"],
            "strategy": "stdout",
            "expected_polarity": "success",
            "observation": {"kind": "stdout", "expected": "5\n"},
        }
        candidate = score_candidate_outcomes(
            case_id=self.case["case_id"],
            lane="contract_conformance",
            oracles=[oracle],
            outcomes=[
                {
                    "classification": "executable",
                    "executed": True,
                    "result": "fail",
                    "actual": "4\n",
                }
            ],
            adapters=self.adapters,
        )
        self.assertEqual(candidate[0]["adapter_id"], "cli-known-values-2-3")
        result = combine_evaluations(
            case=self.case,
            adapters=self.adapters,
            baseline_evaluations=[],
            candidate_evaluations=candidate,
            baseline_status="completed",
        )
        self.assertEqual(result["shadow_verdict"], "failure")

    def test_gold_is_used_only_after_candidate_execution(self):
        oracle = {
            "id": "o1",
            "claim_id": "generated-id",
            "strategy": "stdout",
            "expected_polarity": "success",
            "observation": {"kind": "stdout", "expected": "5\n"},
        }
        rows = score_candidate_outcomes(
            case_id=self.case["case_id"],
            lane="held_out_synthesis",
            oracles=[oracle],
            outcomes=[
                {
                    "executed": True,
                    "result": "pass",
                    "actual": "5\n",
                    "observed_strength": "runtime",
                }
            ],
            adapters=self.adapters,
        )
        self.assertFalse(rows[0]["gold_used_for_execution"])
        self.assertTrue(rows[0]["gold_used_for_scoring"])
        self.assertTrue(rows[0]["observation_match"])


class ContractReadinessTest(unittest.TestCase):
    def test_v4_browser_schema_is_additive_and_separate_from_production_v0(self):
        base = load("eval/goal_verify/v0/verification-spec.schema.json")
        extended = load(
            "eval/goal_verify/v0/verification-spec-preflight-v4.schema.json"
        )
        self.assertNotEqual(base["$id"], extended["$id"])
        base_dom = base["$defs"]["input"]["oneOf"][-1]
        extended_dom = extended["$defs"]["input"]["oneOf"][-1]
        self.assertNotIn("port", base_dom["properties"])
        self.assertIn("port", extended_dom["required"])
        self.assertIn("actions", extended_dom["properties"])

    def test_v4_validator_preserves_browser_extension_outside_production_type(self):
        proposal = load("tests/fixtures/verification_spec_v0/create.json")
        proposal["oracles"][0]["strategy"] = "interaction"
        proposal["oracles"][0]["setup"]["argv"] = [
            "npm",
            "run",
            "dev",
            "--",
            "-p",
            "4174",
        ]
        proposal["oracles"][0]["input"] = {
            "kind": "dom",
            "route": "/",
            "selector": "#count",
            "port": 4174,
            "actions": [{"kind": "click", "selector": "#increment", "repeat": 2}],
        }

        def validator(**kwargs):
            stripped = json.loads(kwargs["normalized_raw"])
            self.assertNotIn("port", stripped["oracles"][0]["input"])
            return {"valid": True, "spec": stripped, "errors": []}

        with mock.patch(
            "eval_lib.goal_verify_live_v4.validate_proposal", side_effect=validator
        ):
            result = _validate_proposal_v4(
                validator=Path("validator"),
                goal=proposal["goal"],
                intent=proposal["intent"],
                normalized_raw=json.dumps(proposal),
            )
        self.assertEqual(result["spec"]["oracles"][0]["input"]["port"], 4174)
        self.assertEqual(
            result["spec"]["oracles"][0]["input"]["actions"][0]["repeat"], 2
        )

    def test_v4_validator_rejects_browser_extension_before_execution(self):
        proposal = load("tests/fixtures/verification_spec_v0/create.json")
        proposal["oracles"][0]["strategy"] = "interaction"
        proposal["oracles"][0]["setup"]["argv"] = ["npm", "run", "dev"]
        proposal["oracles"][0]["input"] = {
            "kind": "dom",
            "route": "/",
            "selector": "#count",
            "port": 4174,
            "actions": [],
        }

        with mock.patch(
            "eval_lib.goal_verify_live_v4.validate_proposal",
            return_value={"valid": True, "spec": proposal, "errors": []},
        ):
            result = _validate_proposal_v4(
                validator=Path("validator"),
                goal=proposal["goal"],
                intent=proposal["intent"],
                normalized_raw=json.dumps(proposal),
            )
        self.assertFalse(result["valid"])
        self.assertIn(
            f"v4_dom_port_unbound:{proposal['oracles'][0]['id']}", result["errors"]
        )
        self.assertIn(
            f"v4_interaction_actions_missing:{proposal['oracles'][0]['id']}",
            result["errors"],
        )

    def test_v4_pair_root_keeps_provisioning_at_execution_root(self):
        execution_root = Path("/execution")
        pair_root = execution_root / "run-id" / "pair-id"
        self.assertEqual(
            pair_root.parents[1] / "provisioned", execution_root / "provisioned"
        )

    def test_v4_contract_is_frozen_with_exact_ci_evidence(self):
        path = ROOT / "eval/goal_verify/v0/phase6-preflight-v4-contract.json"
        contract = load("eval/goal_verify/v0/phase6-preflight-v4-contract.json")
        self.assertEqual(design_errors(root=ROOT, contract=contract), [])
        report = readiness_report(root=ROOT, contract_path=path)
        self.assertFalse(report["ready"])
        self.assertNotIn("contract_not_frozen", report["blockers"])
        self.assertNotIn("exact_code_sha_missing", report["blockers"])
        self.assertNotIn("exact_sha_ci_evidence_missing", report["blockers"])
        self.assertNotIn("live_collection_not_authorized", report["blockers"])

    def test_v4_prompt_has_manifest_but_no_gold_adapter_id(self):
        corpus = load("eval/goal_verify/v0/corpus.json")
        case = next(
            row
            for row in corpus["cases"]
            if row["case_id"] == "create-cli-known-multiple-inputs"
        )
        adapters = load("eval/goal_verify/v0/phase6-command-adapters-v3.json")[
            "adapters"
        ]
        capabilities = load("eval/goal_verify/v0/phase6-execution-capabilities-v3.json")
        base = (
            ROOT / "eval/goal_verify/v0/verification-spec-preflight-v4.prompt.txt"
        ).read_text(encoding="utf-8")
        shape = (ROOT / "tests/fixtures/verification_spec_v0/create.json").read_text(
            encoding="utf-8"
        )
        prompt = _build_prompt_v4(
            lane="contract_conformance",
            base_prompt=base,
            case=case,
            request_id="r",
            shape=shape,
            adapters=adapters,
            capabilities=capabilities,
            manifests={"product": {"snapshot_sha256": "a" * 64, "entries": []}},
        )
        payload = json.loads(prompt.rsplit("INPUT JSON:\n", 1)[1])
        self.assertEqual(
            payload["workspace_manifests"]["product"]["snapshot_sha256"], "a" * 64
        )
        self.assertNotIn("adapter_id", json.dumps(payload["required_claims"]))

    def test_v4_campaign_records_shared_product_snapshot_and_additive_score(self):
        contract = load("eval/goal_verify/v0/phase6-preflight-v4-contract.json")
        contract["selected_cells"] = [
            row
            for row in contract["selected_cells"]
            if row["case_id"] == "create-cli-known-multiple-inputs"
        ]
        contract["samples_per_cell"] = 1
        corpus = load("eval/goal_verify/v0/corpus.json")
        case = next(
            row
            for row in corpus["cases"]
            if row["case_id"] == "create-cli-known-multiple-inputs"
        )
        proposal = load("tests/fixtures/verification_spec_v0/create.json")
        proposal["goal"] = case["goal"]
        proposal["profile"] = case["profile"]
        proposal["claims"][0]["id"] = case["required_claims"][0]["id"]
        proposal["oracles"][0]["claim_id"] = case["required_claims"][0]["id"]
        raw = json.dumps(proposal)
        baseline_calls = []

        def provider(**kwargs):
            self.assertIn("workspace_manifests", kwargs["prompt"])
            return {
                "status": "completed",
                "response": {"response": raw, "prompt_eval_count": 1, "eval_count": 2},
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

        fake_execution = {
            "evaluations": [
                {
                    "oracle_id": "o1",
                    "claim_id": case["required_claims"][0]["id"],
                    "classification": "executable",
                    "stage": "product",
                    "executed": True,
                    "result": "pass",
                    "actual": "5\n",
                    "observed_strength": "runtime",
                    "gold_used_for_execution": False,
                }
            ],
            "same_snapshot": True,
            "reference_fallback_count": 0,
            "gold_used_for_execution_count": 0,
        }
        with tempfile.TemporaryDirectory(dir=ROOT) as temporary:
            temporary_root = Path(temporary)
            contract_path = temporary_root / "contract.json"
            contract_path.write_text(json.dumps(contract), encoding="utf-8")
            run_dir = temporary_root / contract["contract_id"]
            with (
                mock.patch(
                    "eval_lib.goal_verify_live_v4.readiness_report",
                    return_value={"ready": True, "blockers": []},
                ),
                mock.patch(
                    "eval_lib.goal_verify_live_v4.verify_live_inputs_v3",
                    return_value={"commandagent_binary_sha256": "a" * 64},
                ),
                mock.patch(
                    "eval_lib.goal_verify_live_v4.validate_proposal",
                    side_effect=validation,
                ),
                mock.patch(
                    "eval_lib.goal_verify_live_v4.evaluate_candidate_spec_v4",
                    return_value=fake_execution,
                ),
            ):
                summary = run_campaign_v4(
                    root=ROOT,
                    corpus_path=ROOT / "eval/goal_verify/v0/corpus.json",
                    contract_path=contract_path,
                    schema_path=ROOT
                    / contract["generation"]["structured_output_schema"],
                    prompt_path=None,
                    validator=ROOT / "target/release/verification_spec_validate",
                    run_dir=run_dir,
                    execution_root=temporary_root / "execution",
                    provider=provider,
                    baseline_runner=baseline_runner,
                )
            record = json.loads(
                (
                    run_dir / "raw/create-cli-known-multiple-inputs/pair-01.json"
                ).read_text(encoding="utf-8")
            )
            self.assertEqual(len(baseline_calls), 1)
            self.assertEqual(summary["completed_pairs"], 1)
            self.assertIn("product", record["snapshot_manifests"])
            self.assertTrue(
                record["lanes"]["contract_conformance"]["execution"]["same_snapshot"]
            )
            self.assertIn(
                "combined_score",
                record["lanes"]["contract_conformance"]["additive_comparison"],
            )

    def test_v4_report_gates_reference_fallback_and_false_full(self):
        contract = load("eval/goal_verify/v0/phase6-preflight-v4-contract.json")
        contract["selected_cells"] = [{"case_id": "x"}]
        contract["samples_per_cell"] = 1
        additive = {
            "baseline_failure_overridden": False,
            "shadow_verdict": "pass",
            "combined_score": {"claims": [{"status": "strong"}]},
            "paired_delta": {
                "required_claim_recall": 1.0,
                "strong_binding": 1.0,
                "unverified_rate": -1.0,
            },
        }
        record = {
            "snapshot_manifests": {"product": {"snapshot_sha256": "a" * 64}},
            "lanes": {
                "held_out_synthesis": {
                    "validation": {"valid": True},
                    "execution": {
                        "same_snapshot": True,
                        "reference_fallback_count": 0,
                        "gold_used_for_execution_count": 0,
                        "evaluations": [
                            {
                                "classification": "executable",
                                "execution_attempt_recorded": True,
                                "executed": False,
                                "result": "blocked",
                            }
                        ],
                    },
                    "additive_comparison": additive,
                }
            },
        }
        report = build_report(
            contract=contract, records=[record], semantic_review_complete=True
        )
        self.assertTrue(report["ready_for_full_experiment_design"])
        record["lanes"]["held_out_synthesis"]["execution"][
            "reference_fallback_count"
        ] = 1
        blocked = build_report(
            contract=contract, records=[record], semantic_review_complete=True
        )
        self.assertFalse(blocked["checks"]["reference_fallback_zero"])


if __name__ == "__main__":
    unittest.main()
