import hashlib
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
from eval_lib.goal_verify_baseline_product_v3 import build_product_argv
from eval_lib.goal_verify_live_v4 import (
    _build_prompt_v4,
    _campaign_namespace,
    _candidate_visible_execution_policy,
    _execute_spec_isolated,
    _select_campaign_pair_ids,
    _validate_proposal_v4,
    _validate_raw_proposal,
    run_campaign_v4,
)
from eval_lib.goal_verify_next_v4 import detect_executor_capabilities
from eval_lib.goal_verify_preflight_report_v4 import build_report, semantic_review_gate
from eval_lib.goal_verify_preflight_v4 import design_errors, readiness_report
from eval_lib.goal_verify_repairs_v4 import apply_meaning_preserving_repairs
from eval_lib.goal_verify_sandbox import _sandbox_profile, sandbox_backend_status
from eval_lib.goal_verify_task_contracts_v4 import (
    bind_existing_evidence_registry,
    bind_task_contract,
    candidate_visible_task_contract,
    load_task_contract_registry,
    selected_task_contract_errors,
    task_contract_registry_errors,
)
from eval_lib.goal_verify_v2 import normalize_v2_proposal
from eval_lib.goal_verify_v3 import (
    build_conformance_prompt,
    canonicalize_held_out_proposal,
)
from eval_lib.goal_verify_workspaces_v4 import (
    load_v4_workspace_registry,
    selected_product_workspace_errors,
)


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
            (root / ".goal-verify-baseline").mkdir()
            (root / ".goal-verify-baseline/completion-contract.json").write_text(
                "private baseline input", encoding="utf-8"
            )
            first = workspace_manifest(root)
            second = workspace_manifest(root)
            self.assertEqual(first, second)
            self.assertEqual([row["path"] for row in first["entries"]], ["app.py"])
            visible = candidate_visible_manifest(first)
            self.assertNotIn("secret", json.dumps(visible))
            self.assertNotIn("completion-contract", json.dumps(visible))

    def test_candidate_manifest_hides_generated_cache_and_python_environments(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for relative, contents in {
                "app.py": 'print("ok")\n',
                "package.json": "{}\n",
                "pyproject.toml": '[project]\nname = "demo"\n',
                "src/builder/main.py": 'print("source")\n',
                ".next/cache/turbopack/data.sst": "generated",
                ".venv/lib/python/site.py": "dependency",
                "venv/lib/python/site.py": "dependency",
                "__pycache__/app.cpython-312.pyc": "cache",
                ".pytest_cache/v/cache/nodeids": "cache",
                ".mypy_cache/3.12/app.data.json": "cache",
                ".ruff_cache/content": "cache",
                "dist/app.whl": "generated",
                "build/output.txt": "generated",
                "coverage/index.html": "generated",
                "htmlcov/index.html": "generated",
                ".coverage": "generated",
                "coverage.xml": "generated",
                "lcov.info": "generated",
                "default.profraw": "generated",
            }.items():
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(contents, encoding="utf-8")

            full = workspace_manifest(root)
            visible = candidate_visible_manifest(full)
            visible_paths = [row["path"] for row in visible["entries"]]

            self.assertGreater(len(full["entries"]), len(visible_paths))
            self.assertEqual(
                visible_paths,
                ["app.py", "package.json", "pyproject.toml", "src/builder/main.py"],
            )
            self.assertEqual(visible["snapshot_sha256"], full["snapshot_sha256"])
            self.assertEqual(
                visible["candidate_visibility_policy"],
                "commandagent.goal_verify.candidate_manifest.source_config_v1",
            )
            self.assertEqual(len(visible["candidate_entries_sha256"]), 64)
            encoded = json.dumps(visible, sort_keys=True)
            for hidden in (
                ".next",
                ".venv",
                "__pycache__",
                ".pytest_cache",
                "dist/app.whl",
                "build/output.txt",
                "coverage.xml",
                "default.profraw",
            ):
                self.assertNotIn(hidden, encoded)

    def test_hidden_artifact_changes_full_identity_not_visible_entries(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "app.py").write_text("print(1)\n", encoding="utf-8")
            generated = root / ".next/cache/output.bin"
            generated.parent.mkdir(parents=True)
            generated.write_text("first", encoding="utf-8")
            before = candidate_visible_manifest(workspace_manifest(root))

            generated.write_text("second", encoding="utf-8")
            after = candidate_visible_manifest(workspace_manifest(root))

            self.assertNotEqual(before["snapshot_sha256"], after["snapshot_sha256"])
            self.assertEqual(
                before["candidate_entries_sha256"],
                after["candidate_entries_sha256"],
            )
            self.assertEqual(before["entries"], after["entries"])

    def test_candidate_manifest_size_depends_on_source_not_next_build_output(self):
        source_entries = [
            {
                "path": "package.json",
                "kind": "file",
                "sha256": "a" * 64,
                "size": 100,
            },
            {
                "path": "app/page.js",
                "kind": "file",
                "sha256": "b" * 64,
                "size": 200,
            },
        ]
        generated_entries = [
            {
                "path": f".next/server/chunks/{index:04d}.js",
                "kind": "file",
                "sha256": hashlib.sha256(str(index).encode()).hexdigest(),
                "size": 1000 + index,
            }
            for index in range(500)
        ]
        visible = candidate_visible_manifest(
            {
                "schema_version": "commandagent.goal_verify.workspace_manifest.v4",
                "snapshot_sha256": "c" * 64,
                "entries": source_entries + generated_entries,
            }
        )

        self.assertEqual(visible["entries"], source_entries)
        self.assertLess(len(json.dumps(visible, sort_keys=True)), 1000)

    def test_candidate_manifest_preserves_rust_source_and_config(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for relative, contents in {
                "Cargo.toml": '[package]\nname = "demo"\nversion = "0.1.0"\n',
                "Cargo.lock": "# frozen lock\n",
                "src/main.rs": "fn main() {}\n",
                "target/debug/demo": "derived binary",
            }.items():
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(contents, encoding="utf-8")

            full = workspace_manifest(root)
            visible = candidate_visible_manifest(full)

            self.assertEqual(
                [entry["path"] for entry in visible["entries"]],
                ["Cargo.lock", "Cargo.toml", "src/main.rs"],
            )
            self.assertNotIn("target/debug/demo", json.dumps(full))

    def test_a5_task_contract_is_bound_to_both_product_and_candidate_input(self):
        corpus = load("eval/goal_verify/v0/corpus.json")
        case = next(
            row
            for row in corpus["cases"]
            if row["case_id"] == "create-cli-known-multiple-inputs"
        )
        registry = load_task_contract_registry(
            ROOT / "eval/goal_verify/v0/phase6-task-contracts-v4-a5.json"
        )
        bound = bind_task_contract(case, registry)
        self.assertIn("cli/main.py", bound["goal"])
        self.assertEqual(
            bound["task_contract"]["completion_contract"]["verify_commands"][0],
            "python3 cli/main.py 2 3",
        )
        argv = build_product_argv(
            commandagent_bin=Path("/tmp/commandagent"),
            workspace=Path("/tmp/workspace"),
            case=bound,
            model="m",
            completion_contract_path=Path("/tmp/completion-contract.json"),
        )
        self.assertIn("--completion-contract-json", argv)
        self.assertEqual(argv[-1], bound["goal"])
        contract = load("eval/goal_verify/v0/phase6-preflight-v4-contract.json")
        self.assertEqual(
            selected_task_contract_errors(
                corpus=corpus, contract=contract, registry=registry
            ),
            [],
        )

    def test_a9_task_contract_separates_goal_and_typed_constraints(self):
        corpus = load("eval/goal_verify/v0/corpus.json")
        case = next(
            row
            for row in corpus["cases"]
            if row["case_id"] == "create-cli-known-multiple-inputs"
        )
        registry = load_task_contract_registry(
            ROOT / "eval/goal_verify/v0/phase6-task-contracts-v4-a9.json"
        )
        bound = bind_task_contract(case, registry)
        self.assertEqual(bound["source_goal"], case["goal"])
        self.assertNotIn("--help", bound["goal"])
        self.assertFalse(bound["task_contract"]["operational_constraints"]["scored"])
        self.assertEqual(
            bound["task_contract"]["operational_constraints"]["entry_paths"],
            ["cli/main.py"],
        )
        argv = build_product_argv(
            commandagent_bin=Path("/tmp/commandagent"),
            workspace=Path("/tmp/workspace"),
            case=bound,
            model="m",
            recovery_plan_auto_runs=0,
        )
        self.assertIn(bound["goal"], argv[-1])
        self.assertIn('"entry_paths":["cli/main.py"]', argv[-1])
        self.assertIn("not additional acceptance claims", argv[-1])
        self.assertEqual(task_contract_registry_errors(registry), [])
        invalid = json.loads(json.dumps(registry))
        invalid["cases"][0]["operational_constraints"]["unexpected"] = True
        self.assertIn(
            "operational_constraints_unknown:create-build-only-functional:unexpected",
            task_contract_registry_errors(invalid),
        )
        invalid_binding = json.loads(json.dumps(registry))
        invalid_binding["cases"][2]["completion_contract"]["profile"] = "generic"
        with self.assertRaisesRegex(ValueError, "profile mismatch"):
            bind_task_contract(case, invalid_binding)

    def test_a9_baseline_argv_explicitly_disables_recovery_auto_runs(self):
        argv = build_product_argv(
            commandagent_bin=Path("/tmp/commandagent"),
            workspace=Path("/tmp/workspace"),
            case={"intent": "create", "profile": "generic", "goal": "goal"},
            model="m",
            recovery_plan_auto_runs=0,
        )
        index = argv.index("--recovery-plan-auto-runs")
        self.assertEqual(argv[index : index + 2], ["--recovery-plan-auto-runs", "0"])
        self.assertEqual(argv[-2:], ["--plan-run", "goal"])

    def test_a13_candidate_task_contract_excludes_scoring_metadata(self):
        task_contract = {
            "schema_version": "v",
            "completion_contract": {
                "goal": "duplicated goal",
                "profile": "generic",
                "required_paths": ["app.py"],
                "verify_commands": ["python3 app.py"],
                "deterministic_oracles": ["answer-key-like"],
                "evidence_hint_tokens": ["secret-hint"],
                "required_evidence": ["scoring-only"],
            },
            "completion_contract_goal_source": "shared_semantic_goal",
            "offline_dependencies": ["python3-stdlib"],
            "operational_constraints": {"network": "denied", "scored": False},
            "registered_claims": ["registered-id"],
            "source_goal_sha256": "a" * 64,
        }

        visible = candidate_visible_task_contract(task_contract)
        encoded = json.dumps(visible, sort_keys=True)

        self.assertEqual(
            visible["completion_contract"]["verify_commands"], ["python3 app.py"]
        )
        self.assertEqual(visible["operational_constraints"]["network"], "denied")
        for hidden in (
            "registered-id",
            "source_goal_sha256",
            "duplicated goal",
            "answer-key-like",
            "secret-hint",
            "scoring-only",
        ):
            self.assertNotIn(hidden, encoded)

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

    def test_next_web_plan_uses_host_owned_build_and_start_argv(self):
        concrete = concretize_candidate_oracle(
            oracle={
                "strategy": "dom",
                "setup": {
                    "argv": ["npx", "next", "dev", "-p", "4174"],
                    "cwd": ".",
                },
                "input": {
                    "kind": "dom",
                    "port": 4174,
                    "route": "/play",
                    "selector": "#count",
                },
                "observation": {"kind": "dom", "expected": "2"},
            },
            claim={"origin": {"source_kind": "goal"}},
            manifest={"entries": []},
        )
        self.assertEqual(concrete["classification"], "executable")
        self.assertEqual(
            concrete["plan"]["prepare_argv"],
            ["npx", "next", "build", "--webpack"],
        )
        self.assertEqual(
            concrete["plan"]["server_policy"],
            "next_production_webpack_build_start_v2",
        )
        self.assertEqual(
            concrete["plan"]["server_argv"],
            ["npx", "next", "start", "-p", "4174"],
        )
        self.assertNotIn("&&", json.dumps(concrete["plan"]))

    def test_command_sandbox_block_is_unverified_not_product_failure(self):
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary)
            outcome = execute_candidate_plan(
                {
                    "kind": "command",
                    "argv": ["npm", "run", "build"],
                    "cwd": ".",
                    "timeout_ms": 1000,
                    "observation": {"kind": "exit_code", "expected": 0},
                },
                workspace=workspace,
                runner=lambda _plan: {
                    "exit_code": 1,
                    "stdout": "",
                    "stderr": "sandbox denied",
                    "sandbox_blocked": "next_turbopack_network_bind_denied",
                },
            )
        self.assertTrue(outcome["execution_attempt_recorded"])
        self.assertFalse(outcome["executed"])
        self.assertEqual(outcome["result"], "blocked")
        self.assertEqual(
            outcome["reason"],
            "safe_execution_unavailable:next_turbopack_network_bind_denied",
        )

    def test_cli_text_comparison_removes_only_one_terminal_newline(self):
        def runner(_plan):
            return {
                "exit_code": 0,
                "stdout": "5\n",
                "stderr": "",
                "timed_out": False,
            }

        with tempfile.TemporaryDirectory() as temporary:
            result = execute_candidate_plan(
                {
                    "kind": "command",
                    "argv": ["python3", "app.py"],
                    "cwd": ".",
                    "timeout_ms": 1000,
                    "observation": {"kind": "stdout", "expected": "5"},
                },
                workspace=Path(temporary),
                runner=runner,
            )
            self.assertEqual(result["result"], "pass")
            self.assertEqual(
                result["comparison_normalization"],
                "cli_text_single_terminal_newline_v1",
            )

            def extra_newline(_plan):
                return {**runner(_plan), "stdout": "5\n\n"}

            mismatch = execute_candidate_plan(
                {
                    "kind": "command",
                    "argv": ["python3", "app.py"],
                    "cwd": ".",
                    "timeout_ms": 1000,
                    "observation": {"kind": "stdout", "expected": "5"},
                },
                workspace=Path(temporary),
                runner=extra_newline,
            )
            self.assertEqual(mismatch["result"], "fail")

    def test_existing_fix_evidence_uses_bound_artifact_without_gold(self):
        claim = {
            "origin": {
                "source_kind": "fix_requirement",
                "artifact_path": "evidence/fix-evidence.json",
                "requirement_id": "after_passes",
                "stage": "after",
                "expected_polarity": "success",
                "lineage": "case-a",
                "epoch": 1,
            }
        }
        oracle = {
            "strategy": "existing_fix_evidence",
            "observation": {
                "kind": "existing_binding",
                "artifact_path": "evidence/fix-evidence.json",
            },
        }
        concrete = concretize_candidate_oracle(
            oracle=oracle, claim=claim, manifest={"entries": []}
        )
        self.assertEqual(concrete["classification"], "executable")
        self.assertEqual(concrete["stage"], "product")
        self.assertFalse(concrete["gold_used_for_concretization"])
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "evidence").mkdir()
            (root / "evidence/fix-evidence.json").write_text(
                json.dumps(
                    {
                        "bindings": [
                            {
                                "requirement_id": "after_passes",
                                "stage": "after",
                                "expected": "success",
                                "lineage": "case-a",
                                "epoch": 1,
                                "executed": True,
                                "outcome": "success",
                            }
                        ]
                    }
                ),
                encoding="utf-8",
            )
            result = execute_candidate_plan(concrete["plan"], workspace=root)
            self.assertEqual(result["result"], "pass")

    def test_existing_before_binding_selects_product_workspace_once(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            product = root / "product"
            before = root / "before"
            (product / "evidence").mkdir(parents=True)
            before.mkdir()
            (product / "evidence/fix-evidence.json").write_text(
                json.dumps(
                    {
                        "bindings": [
                            {
                                "requirement_id": "before_fails",
                                "stage": "before",
                                "expected": "failure",
                                "lineage": "case-before",
                                "epoch": 1,
                                "executed": True,
                                "outcome": "failure",
                            }
                        ]
                    }
                ),
                encoding="utf-8",
            )
            product_manifest = workspace_manifest(product)
            before_manifest = workspace_manifest(before)
            result = evaluate_candidate_spec_v4(
                spec={
                    "claims": [
                        {
                            "id": "before",
                            "origin": {
                                "source_kind": "fix_requirement",
                                "artifact_path": "evidence/fix-evidence.json",
                                "requirement_id": "before_fails",
                                "stage": "before",
                                "expected_polarity": "failure",
                                "lineage": "case-before",
                                "epoch": 1,
                            },
                        }
                    ],
                    "oracles": [
                        {
                            "id": "before-oracle",
                            "claim_id": "before",
                            "strategy": "existing_fix_evidence",
                            "observation": {
                                "kind": "existing_binding",
                                "artifact_path": "evidence/fix-evidence.json",
                            },
                        }
                    ],
                },
                workspaces={"product": product, "before": before},
                frozen_snapshot_sha256={
                    "product": product_manifest["snapshot_sha256"],
                    "before": before_manifest["snapshot_sha256"],
                },
            )
            evaluation = result["evaluations"][0]
            self.assertEqual(evaluation["stage"], "product")
            self.assertEqual(evaluation["result"], "pass")
            self.assertEqual(
                evaluation["execution_snapshot_sha256"],
                product_manifest["snapshot_sha256"],
            )
            self.assertTrue(result["same_snapshot"])

    def test_next_capability_uses_pinned_version_policy(self):
        expected = {
            "14.2.35": ["npx", "next", "build"],
            "16.3.1": ["npx", "next", "build", "--webpack"],
        }
        for version, build_argv in expected.items():
            with (
                self.subTest(version=version),
                tempfile.TemporaryDirectory() as temporary,
            ):
                root = Path(temporary)
                package = root / "node_modules/next/package.json"
                package.parent.mkdir(parents=True)
                package.write_text(json.dumps({"version": version}), encoding="utf-8")
                capabilities = detect_executor_capabilities(root, profile="nextjs")
                self.assertEqual(capabilities["next"]["safe_build_argv"], build_argv)
                concrete = concretize_candidate_oracle(
                    oracle={
                        "strategy": "dom",
                        "setup": {
                            "argv": ["npx", "next", "start", "-p", "4174"],
                            "cwd": ".",
                        },
                        "input": {
                            "kind": "dom",
                            "port": 4174,
                            "route": "/",
                            "selector": "#count",
                        },
                        "observation": {"kind": "dom", "expected": "2"},
                    },
                    claim={"origin": {"source_kind": "goal"}},
                    manifest={"entries": []},
                    executor_capabilities=capabilities,
                )
                self.assertEqual(concrete["classification"], "executable")
                self.assertEqual(concrete["plan"]["prepare_argv"], build_argv)
                self.assertEqual(concrete["plan"]["next_version"], version)
                npm_build = concretize_candidate_oracle(
                    oracle={
                        "strategy": "command",
                        "setup": {"argv": ["npm", "run", "build"], "cwd": "."},
                        "input": {"kind": "none"},
                        "observation": {"kind": "exit_code", "expected": 0},
                    },
                    claim={"origin": {}},
                    manifest={"entries": []},
                    executor_capabilities=capabilities,
                )
                self.assertEqual(npm_build["classification"], "policy_rejected")
                self.assertEqual(npm_build["reason"], "next_build_argv_not_registered")

    def test_unregistered_next_is_unavailable_without_global_flag_policy(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            package = root / "node_modules/next/package.json"
            package.parent.mkdir(parents=True)
            package.write_text(json.dumps({"version": "15.0.0"}), encoding="utf-8")
            capabilities = detect_executor_capabilities(root, profile="nextjs")
            unavailable = concretize_candidate_oracle(
                oracle={
                    "strategy": "command",
                    "setup": {"argv": ["npx", "next", "build"], "cwd": "."},
                    "input": {"kind": "none"},
                    "observation": {"kind": "exit_code", "expected": 0},
                },
                claim={"origin": {}},
                manifest={"entries": []},
                executor_capabilities=capabilities,
            )
            self.assertEqual(unavailable["classification"], "executor_unavailable")
            python = concretize_candidate_oracle(
                oracle={
                    "strategy": "command",
                    "setup": {
                        "argv": ["python3", "cli.py", "--unknown"],
                        "cwd": ".",
                    },
                    "input": {"kind": "none"},
                    "observation": {"kind": "exit_code", "expected": 2},
                },
                claim={"origin": {}},
                manifest={"entries": []},
                executor_capabilities=capabilities,
            )
            self.assertEqual(python["classification"], "executable")

    def test_next_capability_is_bound_to_the_executed_workspace(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            package = root / "node_modules/next/package.json"
            package.parent.mkdir(parents=True)
            package.write_text(json.dumps({"version": "14.2.35"}), encoding="utf-8")
            capabilities = detect_executor_capabilities(root, profile="nextjs")
            package.write_text(json.dumps({"version": "16.3.1"}), encoding="utf-8")
            manifest = workspace_manifest(root)
            result = evaluate_candidate_spec_v4(
                spec={
                    "claims": [{"id": "build", "origin": {}}],
                    "oracles": [
                        {
                            "id": "build-oracle",
                            "claim_id": "build",
                            "strategy": "command",
                            "setup": {
                                "argv": ["npx", "next", "build"],
                                "cwd": ".",
                            },
                            "input": {"kind": "none"},
                            "observation": {"kind": "exit_code", "expected": 0},
                        }
                    ],
                },
                workspaces={"product": root},
                frozen_snapshot_sha256={"product": manifest["snapshot_sha256"]},
                executor_capabilities=capabilities,
            )
            self.assertFalse(result["same_snapshot"])
            self.assertEqual(
                result["evaluations"][0]["reason"],
                "executor_capability_workspace_mismatch",
            )

    def test_a5_scoring_rejects_a_correct_status_on_the_wrong_port(self):
        adapters = load("eval/goal_verify/v0/phase6-command-adapters-v4-a5.json")[
            "adapters"
        ]
        oracle = {
            "id": "http-1",
            "claim_id": "port-path",
            "strategy": "http",
            "expected_polarity": "success",
            "input": {
                "kind": "http",
                "method": "GET",
                "port": 3000,
                "path": "/play",
            },
            "observation": {"kind": "http_status", "expected": 200},
        }
        outcome = {
            "executed": True,
            "result": "pass",
            "actual": 200,
            "observed_strength": "runtime",
        }
        wrong = score_candidate_outcomes(
            case_id="create-ui-copy-style-port-path",
            lane="contract_conformance",
            oracles=[oracle],
            outcomes=[outcome],
            adapters=adapters,
        )
        self.assertIsNone(wrong[0]["adapter_id"])
        oracle["input"]["port"] = 4173
        correct = score_candidate_outcomes(
            case_id="create-ui-copy-style-port-path",
            lane="contract_conformance",
            oracles=[oracle],
            outcomes=[outcome],
            adapters=adapters,
        )
        self.assertEqual(correct[0]["adapter_id"], "port-path")

    def test_a13_unavailable_adapter_is_not_credited_by_scoring(self):
        corpus = load("eval/goal_verify/v0/phase6-main-corpus-v4.json")
        case = next(
            row
            for row in corpus["cases"]
            if row["case_id"] == "phase6-main-c02-task-01"
        )
        adapters = load("eval/goal_verify/v0/phase6-command-adapters-v4-main.json")[
            "adapters"
        ]
        oracle = {
            "id": "network-source-proxy",
            "claim_id": "no-network",
            "strategy": "command",
            "expected_polarity": "absent",
            "input": {"kind": "none"},
            "observation": {"kind": "stdout", "expected": "0"},
        }

        rows = score_candidate_outcomes(
            case_id=case["case_id"],
            lane="contract_conformance",
            oracles=[oracle],
            outcomes=[
                {
                    "executed": True,
                    "result": "pass",
                    "actual": "0",
                    "observed_strength": "runtime",
                }
            ],
            adapters=adapters,
        )

        self.assertIsNone(rows[0]["adapter_id"])
        self.assertFalse(rows[0]["observation_match"])
        self.assertIsNone(rows[0]["observed_strength"])

    def test_a13_unavailable_adapter_is_not_prompted_as_an_expected_observation(self):
        corpus = load("eval/goal_verify/v0/phase6-main-corpus-v4.json")
        case = next(
            row
            for row in corpus["cases"]
            if row["case_id"] == "phase6-main-c02-task-01"
        )
        adapters = load("eval/goal_verify/v0/phase6-command-adapters-v4-main.json")[
            "adapters"
        ]
        prompt = build_conformance_prompt(
            "base",
            case,
            "a13-unavailable",
            "{}",
            adapters=adapters,
        )
        request = json.loads(prompt.rsplit("INPUT JSON:\n", 1)[1])
        claim = request["required_claims"][0]

        self.assertEqual(claim["executor_status"], "unavailable")
        self.assertEqual(claim["expected_observations"], [])
        self.assertRegex(claim["semantic_policy_sha256"], r"^[0-9a-f]{64}$")
        policy = _candidate_visible_execution_policy(prompt)
        self.assertEqual(policy["claim_strategies"]["no-network"], [])

    def test_a13_candidate_visible_policy_rejects_before_execution(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            result = _execute_spec_isolated(
                spec={
                    "claims": [{"id": "no-network"}],
                    "oracles": [
                        {
                            "id": "network-source-proxy",
                            "claim_id": "no-network",
                            "strategy": "command",
                        }
                    ],
                },
                lane_root=root / "lane",
                frozen_product=root / "product-does-not-need-to-exist",
                frozen_before=None,
                snapshot_manifests={},
                browser_toolchain=None,
                executor_capabilities={},
                candidate_execution_policy={
                    "source": "candidate_visible_prompt",
                    "semantic_policy_sha256": "a" * 64,
                    "claim_strategies": {"no-network": []},
                },
            )
        evaluation = result["evaluations"][0]
        self.assertEqual(evaluation["classification"], "semantic_rejected")
        self.assertFalse(evaluation["execution_attempt_recorded"])
        self.assertEqual(evaluation["result"], "unverified")
        self.assertFalse(evaluation["gold_used_for_execution"])

    def test_a13_generic_investigation_binding_is_semantically_unavailable(self):
        corpus = load("eval/goal_verify/v0/phase6-main-corpus-v4.json")
        case = next(
            row
            for row in corpus["cases"]
            if row["case_id"] == "phase6-main-c09-task-01"
        )
        adapters = load("eval/goal_verify/v0/phase6-command-adapters-v4-main.json")[
            "adapters"
        ]
        oracle = {
            "id": "generic-diagnosis-binding",
            "claim_id": "causal-intervention",
            "strategy": "existing_investigation_binding",
            "expected_polarity": "success",
            "input": {"kind": "none"},
            "observation": {"kind": "existing_binding"},
        }

        rows = score_candidate_outcomes(
            case_id=case["case_id"],
            lane="contract_conformance",
            oracles=[oracle],
            outcomes=[
                {
                    "executed": True,
                    "result": "pass",
                    "actual": True,
                    "observed_strength": "runtime",
                }
            ],
            adapters=adapters,
            case=case,
        )

        self.assertIsNone(rows[0]["adapter_id"])
        self.assertFalse(rows[0]["semantic_admissible"])
        self.assertEqual(
            rows[0]["semantic_admissibility_reason"],
            "semantic_capability_unavailable",
        )
        self.assertFalse(rows[0]["observation_match"])
        self.assertIsNone(rows[0]["observed_strength"])
        self.assertRegex(rows[0]["semantic_policy_sha256"], r"^[0-9a-f]{64}$")

        prompt = build_conformance_prompt(
            "base",
            case,
            "a13-investigation",
            "{}",
            adapters=adapters,
        )
        request = json.loads(prompt.rsplit("INPUT JSON:\n", 1)[1])
        self.assertEqual(
            request["required_claims"][0]["executor_status"], "unavailable"
        )
        self.assertEqual(request["required_claims"][0]["expected_observations"], [])

    def test_a13_explicit_investigation_semantic_capability_can_be_credited(self):
        corpus = load("eval/goal_verify/v0/phase6-main-corpus-v4.json")
        case = next(
            row
            for row in corpus["cases"]
            if row["case_id"] == "phase6-main-c09-task-01"
        )
        adapters = load("eval/goal_verify/v0/phase6-command-adapters-v4-main.json")[
            "adapters"
        ]
        adapter = next(
            row
            for row in adapters
            if row["case_id"] == case["case_id"]
            and row["claim_id"] == "causal-intervention"
        )
        adapter = json.loads(json.dumps(adapter))
        adapter["executor"]["supported_oracle_kinds"] = ["controlled_experiment"]
        oracle = {
            "id": "specific-causal-binding",
            "claim_id": "causal-intervention",
            "strategy": "existing_investigation_binding",
            "expected_polarity": "success",
            "input": {"kind": "none"},
            "observation": {"kind": "existing_binding"},
        }

        rows = score_candidate_outcomes(
            case_id=case["case_id"],
            lane="contract_conformance",
            oracles=[oracle],
            outcomes=[
                {
                    "executed": True,
                    "result": "pass",
                    "actual": True,
                    "observed_strength": "runtime",
                }
            ],
            adapters=[adapter],
            case=case,
        )

        self.assertEqual(rows[0]["adapter_id"], adapter["adapter_id"])
        self.assertTrue(rows[0]["semantic_admissible"])
        self.assertTrue(rows[0]["observation_match"])
        self.assertEqual(rows[0]["observed_strength"], "runtime")

        held_out_oracle = json.loads(json.dumps(oracle))
        held_out_oracle["claim_id"] = "held-temporary-claim"
        held_out_rows = score_candidate_outcomes(
            case_id=case["case_id"],
            lane="held_out_synthesis",
            oracles=[held_out_oracle],
            outcomes=[
                {
                    "executed": True,
                    "result": "pass",
                    "actual": True,
                    "observed_strength": "runtime",
                }
            ],
            adapters=[adapter],
            case=case,
        )
        self.assertEqual(held_out_rows[0]["adapter_id"], adapter["adapter_id"])
        self.assertTrue(held_out_rows[0]["semantic_admissible"])

    def test_a13_task_binding_uses_shared_semantic_goal_for_completion(self):
        corpus = load("eval/goal_verify/v0/phase6-main-corpus-v4.json")
        case = next(
            row
            for row in corpus["cases"]
            if row["case_id"] == "phase6-main-c04-task-01"
        )
        registry = load_task_contract_registry(
            ROOT / "eval/goal_verify/v0/phase6-task-contracts-v4-main.json"
        )

        bound = bind_task_contract(case, registry)
        completion = bound["task_contract"]["completion_contract"]

        self.assertEqual(completion["goal"], case["goal"])
        self.assertIn("/play-01", completion["goal"])
        self.assertIn("4201", completion["goal"])
        self.assertNotIn("4173", completion["goal"])
        self.assertEqual(
            bound["task_contract"]["completion_contract_goal_source"],
            "shared_semantic_goal",
        )

    def test_a13_contract_binds_semantic_policy_and_phase_timing(self):
        contract = load("eval/goal_verify/v0/phase6-main-v4-a13-contract.json")
        self.assertEqual(design_errors(root=ROOT, contract=contract), [])

        invalid = json.loads(json.dumps(contract))
        invalid["semantic_oracle_policy"]["sha256"] = "0" * 64
        self.assertIn(
            "semantic_policy_hash_mismatch",
            design_errors(root=ROOT, contract=invalid),
        )

    def test_a5_registry_binds_dynamic_product_evidence_without_outcome_leak(self):
        corpus = load("eval/goal_verify/v0/corpus.json")
        case = next(
            row
            for row in corpus["cases"]
            if row["case_id"] == "fix-reproduced-after-regression"
        )
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary)
            evidence = workspace / "evidence"
            evidence.mkdir()
            (evidence / "fix-run-after.json").write_text(
                json.dumps(
                    {
                        "requirement_id": "after_passes",
                        "stage": "after",
                        "expected": "success",
                        "lineage": "reproducer:abc",
                        "epoch": 2,
                        "executed": True,
                        "outcome": "success",
                    }
                ),
                encoding="utf-8",
            )
            bound = bind_existing_evidence_registry(case, workspace)
        row = next(
            item
            for item in bound["existing_evidence_registry"]
            if item["claim_id"] == "before-after"
        )
        self.assertEqual(row["artifact_path"], "evidence/fix-run-after.json")
        self.assertEqual(row["stage"], "after")
        self.assertNotIn("outcome", row)

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

    def test_a10_raw_schema_requires_semantics_and_rejects_host_owned_fields(self):
        schema = load(
            "eval/goal_verify/v0/verification-spec-raw-preflight-v4-a10.schema.json"
        )
        proposal = load("tests/fixtures/goal_verify_v4/raw-proposal-a10-create.json")
        valid = _validate_raw_proposal(raw=json.dumps(proposal), schema=schema)
        self.assertTrue(valid["valid"], valid)

        incomplete = json.loads(json.dumps(proposal))
        incomplete["claims"][0] = {"oracle_ids": ["hello-output"]}
        rejected = _validate_raw_proposal(raw=json.dumps(incomplete), schema=schema)
        self.assertFalse(rejected["valid"])
        self.assertTrue(
            any("'id' is a required property" in error for error in rejected["errors"]),
            rejected,
        )

        host_fields = json.loads(json.dumps(proposal))
        host_fields["claims"][0]["origin"] = {
            "source_kind": "goal",
            "start_byte": 0,
            "end_byte": 1,
        }
        host_fields["oracles"][0]["lifecycle"] = "proposed"
        rejected = _validate_raw_proposal(raw=json.dumps(host_fields), schema=schema)
        self.assertFalse(rejected["valid"])
        self.assertTrue(
            any("Additional properties" in error for error in rejected["errors"]),
            rejected,
        )

    def test_a10_canonicalizers_inject_only_host_owned_fields(self):
        proposal = load("tests/fixtures/goal_verify_v4/raw-proposal-a10-create.json")
        raw = json.dumps(proposal, ensure_ascii=False)
        case = {
            "case_id": "raw-a10-create",
            "goal": proposal["goal"],
            "intent": "create",
            "profile": proposal["profile"],
            "required_claims": [{"id": "hello-output"}],
        }
        canonical_raw = normalize_v2_proposal(
            raw,
            case=case,
            model="test-model",
            request_id="a10-conformance",
        )
        canonical = json.loads(canonical_raw)
        claim = canonical["claims"][0]
        oracle = canonical["oracles"][0]
        self.assertEqual(claim["id"], "hello-output")
        self.assertEqual(claim["normalized_requirement"], "The CLI prints hello.")
        self.assertEqual(claim["kind"], "behavior")
        self.assertEqual(claim["origin"]["source_kind"], "goal")
        self.assertTrue(claim["required"])
        self.assertEqual(claim["oracle_ids"], [oracle["id"]])
        self.assertEqual(oracle["claim_id"], "hello-output")
        self.assertEqual(oracle["lifecycle"], "proposed")
        self.assertEqual(oracle["result"], "unverified")
        self.assertEqual(
            canonical["generation"]["raw_response_sha256"],
            hashlib.sha256(raw.encode("utf-8")).hexdigest(),
        )
        canonical_schema = load(
            "eval/goal_verify/v0/verification-spec-preflight-v4-a5.schema.json"
        )
        self.assertTrue(
            _validate_raw_proposal(raw=canonical_raw, schema=canonical_schema)["valid"]
        )

        held_out_raw = canonicalize_held_out_proposal(
            raw,
            case=case,
            model="test-model",
            request_id="a10-held-out",
        )
        held_out = json.loads(held_out_raw)
        self.assertTrue(held_out["claims"][0]["id"].startswith("held-"))
        self.assertEqual(
            held_out["claims"][0]["oracle_ids"],
            [held_out["oracles"][0]["id"]],
        )

        extra = json.loads(json.dumps(proposal))
        extra["claims"][0]["id"] = "extra-claim"
        extra["oracles"][0]["claim_id"] = "extra-claim"
        with self.assertRaisesRegex(ValueError, "exactly the registered"):
            normalize_v2_proposal(
                json.dumps(extra),
                case=case,
                model="test-model",
                request_id="a10-extra",
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
    def test_v4_host_repairs_are_conditional_and_record_binding_hashes(self):
        proposal = load("tests/fixtures/verification_spec_v0/create.json")
        oracle = proposal["oracles"][0]
        oracle["strategy"] = "interaction"
        oracle["setup"].pop("fixture_paths")
        oracle["input"] = {
            "kind": "dom",
            "route": "/",
            "selector": "#count",
            "port": 4174,
            "computed_style_property": "color",
        }
        oracle["observation"] = {
            "kind": "interaction",
            "expected": "2",
            "actions": [{"kind": "click", "selector": "#add", "repeat": 2}],
        }
        repaired, rows = apply_meaning_preserving_repairs(proposal)
        repaired_oracle = repaired["oracles"][0]
        self.assertEqual(repaired_oracle["setup"]["fixture_paths"], [])
        self.assertEqual(repaired_oracle["input"]["property"], "color")
        self.assertEqual(repaired_oracle["input"]["actions"][0]["repeat"], 2)
        self.assertNotIn("actions", repaired_oracle["observation"])
        self.assertEqual(len(rows), 1)
        self.assertNotEqual(
            rows[0]["before_binding_sha256"], rows[0]["after_binding_sha256"]
        )
        self.assertTrue(rows[0]["semantic_equivalence"])

    def test_v4_host_repairs_reject_ambiguous_or_fixture_defaults(self):
        proposal = load("tests/fixtures/verification_spec_v0/fix.json")
        oracle = proposal["oracles"][0]
        oracle["setup"].pop("fixture_paths")
        oracle["observation"]["actions"] = []
        repaired, rows = apply_meaning_preserving_repairs(proposal)
        self.assertNotIn("fixture_paths", repaired["oracles"][0]["setup"])
        self.assertIn("actions", repaired["oracles"][0]["observation"])
        self.assertEqual(rows, [])

    def test_v4_validator_reports_yield_before_and_after_host_repairs(self):
        proposal = load("tests/fixtures/verification_spec_v0/create.json")
        proposal["oracles"][0]["setup"].pop("fixture_paths")

        def validator(**kwargs):
            spec = json.loads(kwargs["normalized_raw"])
            valid = "fixture_paths" in spec["oracles"][0]["setup"]
            return {
                "valid": valid,
                "spec": spec if valid else None,
                "errors": [] if valid else ["schema_invalid:missing fixture_paths"],
            }

        with mock.patch(
            "eval_lib.goal_verify_live_v4.validate_proposal", side_effect=validator
        ):
            result = _validate_proposal_v4(
                validator=Path("validator"),
                goal=proposal["goal"],
                intent=proposal["intent"],
                normalized_raw=json.dumps(proposal),
            )
        self.assertTrue(result["valid"])
        self.assertFalse(result["valid_before_host_repairs"])
        self.assertEqual(len(result["host_repairs"]), 1)

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

    def test_a5_schema_and_validator_accept_only_explicit_honest_unknown(self):
        schema = load(
            "eval/goal_verify/v0/verification-spec-preflight-v4-a5.schema.json"
        )
        proposal = load("tests/fixtures/verification_spec_v0/create.json")
        proposal["claims"][0]["oracle_ids"] = []
        proposal["claims"][0]["unverifiable_reason"] = "executor_capability_unavailable"
        proposal["oracles"] = []
        result = _validate_proposal_v4(
            validator=Path("unused-for-all-unverifiable"),
            goal=proposal["goal"],
            intent=proposal["intent"],
            normalized_raw=json.dumps(proposal),
            proposal_schema=schema,
        )
        self.assertTrue(result["valid"])
        self.assertEqual(
            result["unverifiable_claims"],
            [
                {
                    "claim_id": proposal["claims"][0]["id"],
                    "reason": "executor_capability_unavailable",
                }
            ],
        )
        proposal["claims"][0]["unverifiable_reason"] = "made_up_reason"
        rejected = _validate_proposal_v4(
            validator=Path("unused-for-all-unverifiable"),
            goal=proposal["goal"],
            intent=proposal["intent"],
            normalized_raw=json.dumps(proposal),
            proposal_schema=schema,
        )
        self.assertFalse(rejected["valid"])
        self.assertTrue(rejected["errors"][0].startswith("schema_invalid:"))

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

    def test_a5_contract_is_frozen_with_exact_ci_evidence(self):
        path = ROOT / "eval/goal_verify/v0/phase6-preflight-v4-a5-contract.json"
        contract = load("eval/goal_verify/v0/phase6-preflight-v4-a5-contract.json")
        self.assertEqual(design_errors(root=ROOT, contract=contract), [])
        report = readiness_report(root=ROOT, contract_path=path)
        self.assertNotIn("contract_not_frozen", report["blockers"])
        self.assertNotIn("exact_code_sha_missing", report["blockers"])
        self.assertNotIn("exact_sha_ci_evidence_missing", report["blockers"])
        self.assertEqual(
            contract["code_sha"],
            "de99dbacc89ec6a37fcab5d8cdacfa8cf0921897",
        )
        self.assertEqual(
            contract["claim_policy"]["scoring"],
            "retain claim in denominator as unverified",
        )

    def test_a6_contract_is_frozen_with_exact_ci_evidence(self):
        path = ROOT / "eval/goal_verify/v0/phase6-preflight-v4-a6-contract.json"
        contract = load("eval/goal_verify/v0/phase6-preflight-v4-a6-contract.json")
        self.assertEqual(design_errors(root=ROOT, contract=contract), [])
        report = readiness_report(root=ROOT, contract_path=path)
        self.assertNotIn("contract_not_frozen", report["blockers"])
        self.assertNotIn("exact_code_sha_missing", report["blockers"])
        self.assertNotIn("exact_sha_ci_evidence_missing", report["blockers"])
        self.assertEqual(
            contract["code_sha"],
            "05bda1cc7047ab37f96e3b45939fed68f8dac3f6",
        )
        self.assertFalse(contract["baseline"]["completion_verify_result_required"])
        self.assertTrue(contract["baseline"]["honest_terminal_required"])

    def test_a7_contract_is_frozen_with_exact_ci_evidence(self):
        path = ROOT / "eval/goal_verify/v0/phase6-preflight-v4-a7-contract.json"
        contract = load("eval/goal_verify/v0/phase6-preflight-v4-a7-contract.json")
        self.assertEqual(design_errors(root=ROOT, contract=contract), [])
        report = readiness_report(root=ROOT, contract_path=path)
        self.assertNotIn("contract_not_frozen", report["blockers"])
        self.assertNotIn("exact_code_sha_missing", report["blockers"])
        self.assertNotIn("exact_sha_ci_evidence_missing", report["blockers"])
        self.assertEqual(
            contract["code_sha"],
            "4234e0a171deaf9db085e9d3f50a90959d450bee",
        )
        self.assertIn("--webpack", contract["pre_live_amendments"][-1]["change"])
        self.assertIn("same-sandbox", contract["execution"]["server_policy"])

    def test_a8_contract_is_frozen_with_exact_ci_evidence(self):
        path = ROOT / "eval/goal_verify/v0/phase6-preflight-v4-a8-contract.json"
        contract = load("eval/goal_verify/v0/phase6-preflight-v4-a8-contract.json")
        self.assertEqual(design_errors(root=ROOT, contract=contract), [])
        report = readiness_report(root=ROOT, contract_path=path)
        self.assertNotIn("contract_not_frozen", report["blockers"])
        self.assertNotIn("exact_code_sha_missing", report["blockers"])
        self.assertNotIn("exact_sha_ci_evidence_missing", report["blockers"])
        self.assertEqual(
            contract["code_sha"],
            "37df7f95d7ab6c5b2e0d9ff56e1ffe8de62f5bf8",
        )
        prompt = (ROOT / contract["generation"]["prompt"]).read_text()
        self.assertIn('["npx","next","build","--webpack"]', prompt)
        self.assertIn("safe_execution_unavailable", prompt)

    def test_a9_draft_preserves_scoring_and_preregisters_smoke(self):
        contract = load("eval/goal_verify/v0/phase6-preflight-v4-a9-contract.json")
        previous = load("eval/goal_verify/v0/phase6-preflight-v4-a8-contract.json")
        self.assertEqual(design_errors(root=ROOT, contract=contract), [])
        self.assertEqual(
            contract["pre_live_amendments"][:3], previous["pre_live_amendments"]
        )
        amendment = contract["pre_live_amendments"][-1]
        self.assertTrue(amendment["result_dependent"])
        self.assertFalse(amendment["quality_effect_estimate_used"])
        self.assertEqual(contract["scoring"], previous["scoring"])
        self.assertEqual(
            contract["baseline"]["recovery_plan_argv"],
            [
                "--recovery-plan-auto-runs",
                "0",
            ],
        )
        self.assertEqual(len(contract["smoke"]["pair_ids"]), 9)
        self.assertNotEqual(
            contract["smoke"]["request_namespace"], contract["contract_id"]
        )
        self.assertTrue(contract["authorization"]["implementation_authorized"])
        self.assertTrue(contract["authorization"]["live_collection_authorized"])

    def test_a10_frozen_contract_separates_raw_and_canonical_schemas(self):
        contract = load("eval/goal_verify/v0/phase6-preflight-v4-a10-contract.json")
        previous = load("eval/goal_verify/v0/phase6-preflight-v4-a9-contract.json")
        self.assertEqual(design_errors(root=ROOT, contract=contract), [])
        self.assertEqual(
            contract["pre_live_amendments"][:4], previous["pre_live_amendments"]
        )
        amendment = contract["pre_live_amendments"][-1]
        self.assertEqual(amendment["id"], "v4-A10")
        self.assertTrue(amendment["result_dependent"])
        self.assertFalse(amendment["quality_effect_estimate_used"])
        self.assertEqual(amendment["superseded_run_status"], "never_started")
        self.assertEqual(contract["status"], "frozen")
        self.assertEqual(
            contract["code_sha"],
            "8e73df5701bbc4446c17e7c800c2d08c765c51a6",
        )
        self.assertEqual(
            contract["exact_sha_ci_evidence"],
            "eval/goal_verify/v0/exact-sha-ci-8e73df57.json",
        )
        generation = contract["generation"]
        self.assertNotEqual(
            generation["structured_output_schema"], generation["canonical_schema"]
        )
        self.assertEqual(contract["scoring"], previous["scoring"])
        self.assertEqual(contract["preflight"], previous["preflight"])
        self.assertEqual(contract["full_experiment"], previous["full_experiment"])
        self.assertEqual(
            contract["task_contract_registry"], previous["task_contract_registry"]
        )
        self.assertEqual(contract["smoke"]["pair_ids"], previous["smoke"]["pair_ids"])
        self.assertNotEqual(
            contract["smoke"]["request_namespace"],
            previous["smoke"]["request_namespace"],
        )
        prompt = (ROOT / generation["prompt"]).read_text(encoding="utf-8")
        self.assertIn("RAW PROPOSAL CONTRACT", prompt)
        self.assertIn("oracle_ids` is never a", prompt)
        self.assertIn("Missing `id`, `normalized_requirement`, or `kind`", prompt)

    def test_v4_a4_addition_supplies_every_selected_product_workspace(self):
        contract = load("eval/goal_verify/v0/phase6-preflight-v4-contract.json")
        contract["workspace_registry_additions"] = (
            "eval/goal_verify/v0/phase6-real-workspaces-v4-a4.json"
        )
        registry = load_v4_workspace_registry(root=ROOT, contract=contract)
        self.assertEqual(
            selected_product_workspace_errors(
                root=ROOT, contract=contract, registry=registry
            ),
            [],
        )
        negative = next(
            row
            for row in registry["workspaces"]
            if row["case_id"] == "create-negative-constraint-injection"
        )
        self.assertEqual(negative["product_run"]["initial_stage"], "initial")

    def test_v4_readiness_rejects_missing_selected_product_workspace(self):
        contract = load("eval/goal_verify/v0/phase6-preflight-v4-contract.json")
        contract.pop("workspace_registry_additions")
        registry = load_v4_workspace_registry(root=ROOT, contract=contract)
        self.assertIn(
            "selected_product_workspace_missing:create-negative-constraint-injection",
            selected_product_workspace_errors(
                root=ROOT, contract=contract, registry=registry
            ),
        )

    def test_v4_readiness_rejects_wrong_selected_product_stage(self):
        contract = load("eval/goal_verify/v0/phase6-preflight-v4-contract.json")
        contract["selected_cells"] = [
            {
                "case_id": "create-negative-constraint-injection",
                "intent": "create",
            }
        ]
        registry = {
            "workspaces": [
                {
                    "case_id": "create-negative-constraint-injection",
                    "intent": "create",
                    "stages": {"before": "wrong stage"},
                    "product_run": {"initial_stage": "before"},
                }
            ]
        }
        errors = selected_product_workspace_errors(
            root=ROOT, contract=contract, registry=registry
        )
        self.assertIn(
            "selected_product_stage_missing:create-negative-constraint-injection:initial",
            errors,
        )
        self.assertIn(
            "selected_product_stage_contract_mismatch:create-negative-constraint-injection:initial",
            errors,
        )

    def test_v4_readiness_rejects_missing_selected_product_stage_directory(self):
        contract = load("eval/goal_verify/v0/phase6-preflight-v4-contract.json")
        contract["selected_cells"] = [
            {
                "case_id": "create-negative-constraint-injection",
                "intent": "create",
            }
        ]
        registry = {
            "workspaces": [
                {
                    "case_id": "create-negative-constraint-injection",
                    "intent": "create",
                    "root": "tests/fixtures/goal_verify_v4/not-authored/",
                    "stages": {"initial": "declared but absent"},
                    "product_run": {"initial_stage": "initial"},
                }
            ]
        }
        self.assertIn(
            "selected_product_stage_directory_missing:create-negative-constraint-injection:initial",
            selected_product_workspace_errors(
                root=ROOT, contract=contract, registry=registry
            ),
        )

    def test_v4_campaign_rejects_schema_path_not_frozen_by_contract(self):
        contract_path = ROOT / "eval/goal_verify/v0/phase6-preflight-v4-contract.json"
        with self.assertRaisesRegex(
            ValueError,
            "schema path differs from contract.generation.structured_output_schema",
        ):
            run_campaign_v4(
                root=ROOT,
                corpus_path=ROOT / "eval/goal_verify/v0/corpus.json",
                contract_path=contract_path,
                schema_path=ROOT / "eval/goal_verify/v0/verification-spec.schema.json",
                prompt_path=None,
                validator=ROOT / "target/release/verification_spec_validate",
                run_dir=ROOT / "unused",
                execution_root=ROOT / "unused-execution",
            )

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
            executor_capabilities={
                "next": {
                    "status": "registered",
                    "next_version": "14.2.35",
                    "safe_build_argv": ["npx", "next", "build"],
                    "safe_start_argv_template": [
                        "npx",
                        "next",
                        "start",
                        "-p",
                        "<port>",
                    ],
                }
            },
        )
        payload = json.loads(prompt.rsplit("INPUT JSON:\n", 1)[1])
        self.assertEqual(
            payload["workspace_manifests"]["product"]["snapshot_sha256"], "a" * 64
        )
        self.assertNotIn("adapter_id", json.dumps(payload["required_claims"]))
        self.assertEqual(
            payload["executor_capabilities"]["next"]["next_version"], "14.2.35"
        )

    def test_a9_smoke_selector_requires_exact_unique_registered_pairs(self):
        selected = [{"case_id": "case-a"}, {"case_id": "case-b"}]
        pair_ids = ["case-a--pair-02", "case-b--pair-01"]
        self.assertEqual(
            _select_campaign_pair_ids(
                selected=selected, samples_per_cell=2, pair_ids=pair_ids
            ),
            pair_ids,
        )
        with self.assertRaisesRegex(ValueError, "present and unique"):
            _select_campaign_pair_ids(
                selected=selected,
                samples_per_cell=2,
                pair_ids=["case-a--pair-01", "case-a--pair-01"],
            )
        with self.assertRaisesRegex(ValueError, "not in the contract"):
            _select_campaign_pair_ids(
                selected=selected,
                samples_per_cell=2,
                pair_ids=["case-c--pair-01"],
            )
        contract = {
            "contract_id": "live",
            "smoke": {"pair_ids": pair_ids, "request_namespace": "smoke"},
        }
        self.assertEqual(
            _campaign_namespace(
                contract=contract,
                pair_ids=pair_ids,
                request_namespace="smoke",
                limit=None,
            ),
            "smoke",
        )
        with self.assertRaisesRegex(ValueError, "plain pair limit"):
            _campaign_namespace(
                contract=contract,
                pair_ids=pair_ids,
                request_namespace="smoke",
                limit=1,
            )
        with self.assertRaisesRegex(ValueError, "exact set"):
            _campaign_namespace(
                contract=contract,
                pair_ids=list(reversed(pair_ids)),
                request_namespace="smoke",
                limit=None,
            )

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

    def test_a10_campaign_validates_raw_before_host_canonicalization(self):
        contract = load("eval/goal_verify/v0/phase6-preflight-v4-a10-contract.json")
        contract["selected_cells"] = [
            row
            for row in contract["selected_cells"]
            if row["case_id"] == "create-cli-known-multiple-inputs"
        ]
        contract["samples_per_cell"] = 1
        corpus = load("eval/goal_verify/v0/corpus.json")
        source_case = next(
            row
            for row in corpus["cases"]
            if row["case_id"] == "create-cli-known-multiple-inputs"
        )
        task_registry = load_task_contract_registry(
            ROOT / contract["task_contract_registry"]
        )
        case = bind_task_contract(source_case, task_registry)
        proposal = load("tests/fixtures/goal_verify_v4/raw-proposal-a10-create.json")
        proposal["goal"] = case["goal"]
        proposal["profile"] = case["profile"]
        proposal["claims"][0]["id"] = case["required_claims"][0]["id"]
        proposal["oracles"][0]["claim_id"] = case["required_claims"][0]["id"]
        raw = json.dumps(proposal)
        baseline_calls = []
        provider_schemas = []

        def provider(**kwargs):
            provider_schemas.append(kwargs["schema"])
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
            manifest = json.loads(
                (run_dir / "campaign-manifest.json").read_text(encoding="utf-8")
            )
            self.assertEqual(summary["completed_pairs"], 1)
            self.assertEqual(len(baseline_calls), 1)
            self.assertEqual(len(provider_schemas), 2)
            self.assertNotIn("generation", provider_schemas[0]["properties"])
            self.assertIn("canonical_schema_sha256", manifest)
            for lane in ("contract_conformance", "held_out_synthesis"):
                attempt = record["lanes"][lane]["attempts"][0]
                self.assertEqual(
                    attempt["raw_schema_validation"],
                    {"applied": True, "valid": True, "errors": []},
                )
                canonical = json.loads(attempt["normalized_proposal"])
                self.assertIn("origin", canonical["claims"][0])
                self.assertIn("oracle_ids", canonical["claims"][0])
                self.assertIn("generation", canonical)
                self.assertNotIn("origin", proposal["claims"][0])

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
        self.assertEqual(report["counts"]["schema_valid_before_host_repairs"], 1)
        self.assertEqual(report["counts"]["host_repaired_lanes"], 0)
        record["lanes"]["held_out_synthesis"]["execution"][
            "reference_fallback_count"
        ] = 1
        blocked = build_report(
            contract=contract, records=[record], semantic_review_complete=True
        )
        self.assertFalse(blocked["checks"]["reference_fallback_zero"])

    def test_a5_report_requires_bound_discovered_baseline_attempt(self):
        contract = load("eval/goal_verify/v0/phase6-preflight-v4-a5-contract.json")
        contract["selected_cells"] = [{"case_id": "x"}]
        contract["samples_per_cell"] = 1
        record = {
            "baseline": {
                "completion_contract_bound": True,
                "product_run_dir": "/run/1",
                "completion_verify_attempt_recorded": True,
                "observations": [],
            },
            "snapshot_manifests": {"product": {"snapshot_sha256": "a" * 64}},
            "lanes": {
                "held_out_synthesis": {
                    "validation": {
                        "valid": True,
                        "unverifiable_claims": [
                            {
                                "claim_id": "c",
                                "reason": "executor_capability_unavailable",
                            }
                        ],
                    },
                    "execution": {
                        "same_snapshot": True,
                        "reference_fallback_count": 0,
                        "gold_used_for_execution_count": 0,
                        "evaluations": [],
                    },
                    "additive_comparison": {
                        "baseline_failure_overridden": False,
                        "shadow_verdict": "unverified",
                        "combined_score": {"claims": [{"status": "unverified"}]},
                        "paired_delta": {
                            "required_claim_recall": 0.0,
                            "strong_binding": 0.0,
                            "unverified_rate": 0.0,
                        },
                    },
                }
            },
        }
        report = build_report(
            contract=contract, records=[record], semantic_review_complete=True
        )
        self.assertTrue(report["ready_for_full_experiment_design"])
        self.assertEqual(report["counts"]["explicit_unverifiable_claims"], 1)
        record["baseline"]["completion_verify_attempt_recorded"] = False
        blocked = build_report(
            contract=contract, records=[record], semantic_review_complete=True
        )
        self.assertFalse(blocked["checks"]["baseline_completion_verify_attempted"])

    def test_a6_report_accepts_recorded_honest_early_baseline_failure(self):
        contract = load("eval/goal_verify/v0/phase6-preflight-v4-a5-contract.json")
        contract["selected_cells"] = [{"case_id": "x"}]
        contract["samples_per_cell"] = 1
        contract["baseline"].update(
            {
                "completion_verify_result_required": False,
                "task_contract_bound_required": True,
                "product_run_discovered_required": True,
                "honest_terminal_required": True,
            }
        )
        record = {
            "baseline": {
                "completion_contract_bound": True,
                "product_run_dir": "/run/1",
                "completion_verify_attempt_recorded": False,
                "status": "failed",
                "returncode": 1,
                "observations": [],
            },
            "snapshot_manifests": {"product": {"snapshot_sha256": "a" * 64}},
            "lanes": {
                "held_out_synthesis": {
                    "validation": {"valid": True},
                    "execution": {
                        "same_snapshot": True,
                        "reference_fallback_count": 0,
                        "gold_used_for_execution_count": 0,
                        "evaluations": [],
                    },
                    "additive_comparison": {
                        "baseline_failure_overridden": False,
                        "shadow_verdict": "failure",
                        "combined_score": {"claims": [{"status": "unverified"}]},
                        "paired_delta": {
                            "required_claim_recall": 0.0,
                            "strong_binding": 0.0,
                            "unverified_rate": 0.0,
                        },
                    },
                }
            },
        }
        report = build_report(
            contract=contract, records=[record], semantic_review_complete=True
        )
        self.assertTrue(report["ready_for_full_experiment_design"])
        self.assertTrue(report["checks"]["baseline_honest_terminal_recorded"])
        self.assertEqual(report["counts"]["baseline_honest_early_failures"], 1)
        record["baseline"]["returncode"] = 0
        blocked = build_report(
            contract=contract, records=[record], semantic_review_complete=True
        )
        self.assertFalse(blocked["checks"]["baseline_honest_terminal_recorded"])

    def test_preflight_semantic_gate_cannot_be_set_by_boolean_only(self):
        contract = load("eval/goal_verify/v0/phase6-preflight-v4-a5-contract.json")
        self.assertFalse(semantic_review_gate(contract=contract, blind_report=None))
        report = {
            "semantic_review_complete": True,
            "checks": {"human_review_complete": True},
            "human_review": {
                "valid": True,
                "reviewer_type": "human",
                "contract_authoring_involvement": False,
                "independence_confirmed": True,
            },
        }
        self.assertTrue(semantic_review_gate(contract=contract, blind_report=report))
        report["human_review"]["reviewer_type"] = "model"
        self.assertFalse(semantic_review_gate(contract=contract, blind_report=report))

    def test_preflight_semantic_gate_accepts_only_contract_authorized_ai(self):
        contract = load("eval/goal_verify/v0/phase6-preflight-v4-a12-contract.json")
        authorization = contract["semantic_review"]["calibration_reviewer_policy"][
            "authorized_ai_reviewer"
        ]
        calibration = {
            **authorization,
            "valid": True,
            "reviewer_type": "ai",
            "user_authorized": True,
            "source_blind_confirmed": True,
            "forbidden_materials_not_accessed": True,
            "reviewer_output_independence_confirmed": True,
        }
        report = {
            "semantic_review_complete": True,
            "checks": {
                "all_items_have_stable_ids": True,
                "all_model_reviews_valid": True,
                "at_least_two_model_reviews": True,
                "distinct_model_families": True,
                "human_sample_is_ten": True,
                "human_items_match_master": True,
                "human_review_complete": False,
                "calibration_review_complete": True,
            },
            "calibration_review": calibration,
        }
        self.assertTrue(semantic_review_gate(contract=contract, blind_report=report))
        calibration["authorization_id"] = "different-authorization"
        self.assertFalse(semantic_review_gate(contract=contract, blind_report=report))


if __name__ == "__main__":
    unittest.main()
