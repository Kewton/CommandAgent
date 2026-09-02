from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib import generate_goal_verify_recovery_a26 as a26_generator
from eval_lib import generate_goal_verify_recovery_a27 as a27_generator
from eval_lib.goal_verify_recovery_deterministic_pair import (
    A26_REPORT_SCHEMA_VERSION,
    NEXTJS_REPRO_COMMAND,
    SCENARIO_ORDER,
    SCENARIOS,
    ScriptedNextjsFixRecoveryProvider,
    _build_arm_report,
    _path_manifest,
    _write_nextjs_fix_fixture,
    build_pilot_report,
    contract_errors,
    fixture_manifest_sha256,
)


def event(name: str, **fields):
    return {"event": name, **fields}


def valid_arm(scenario_id: str, arm: str) -> dict:
    endpoint = arm == "treatment"
    return {
        "scenario_id": scenario_id,
        "profile": SCENARIOS[scenario_id].profile,
        "arm": arm,
        "arm_valid": True,
        "binary_sha256": "a" * 64,
        "input_snapshot_sha256": f"input-{scenario_id}",
        "boundary_signature_sha256": f"boundary-{scenario_id}",
        "registered_endpoint_success": endpoint,
    }


class DeterministicRecoveryPairTest(unittest.TestCase):
    def test_nextjs_provider_reads_then_repairs_registered_target(self):
        provider = ScriptedNextjsFixRecoveryProvider("ready\n")
        planner = lambda text: {"messages": [{"content": text}], "tools": []}
        execution = {"messages": [], "tools": [{"function": {"name": "Read"}}]}

        provider.response_for(
            planner("Inspect the current workspace before changing files")
        )
        inspect = provider.response_for(execution)
        self.assertEqual(
            inspect["message"]["tool_calls"][0]["function"],
            {"name": "Read", "arguments": {"path": "lib/label.mjs"}},
        )

        provider.response_for(
            planner("Repair the incomplete work for the failed phase")
        )
        repair = provider.response_for(execution)
        self.assertEqual(
            repair["message"]["tool_calls"][0]["function"],
            {
                "name": "Write",
                "arguments": {"path": "lib/label.mjs", "content": "ready\n"},
            },
        )

        verify_plan = provider.response_for(
            planner("Verify the recovered output with deterministic checks")
        )
        self.assertIn(NEXTJS_REPRO_COMMAND, verify_plan["message"]["content"])

    def test_pilot_report_is_go_for_one_valid_pair_per_profile(self):
        arms = [
            valid_arm(scenario_id, arm)
            for scenario_id in SCENARIO_ORDER
            for arm in ("control", "treatment")
        ]
        report = build_pilot_report(
            contract={"contract_id": "a26", "binary_sha256": "a" * 64},
            arm_reports=arms,
        )

        self.assertEqual(report["schema_version"], A26_REPORT_SCHEMA_VERSION)
        self.assertTrue(report["instrument_ready"])
        self.assertEqual(report["pilot_go_no_go"], "GO")
        self.assertFalse(report["effect_claim_allowed"])
        self.assertFalse(report["conditional_effect_estimate_reported"])
        self.assertEqual(
            report["next_design_decision"],
            "request_owner_review_before_a27_confirmatory_preregistration",
        )

    def test_pilot_report_fails_closed_on_boundary_mismatch(self):
        arms = [
            valid_arm(scenario_id, arm)
            for scenario_id in SCENARIO_ORDER
            for arm in ("control", "treatment")
        ]
        arms[1]["boundary_signature_sha256"] = "different"

        report = build_pilot_report(
            contract={"contract_id": "a26", "binary_sha256": "a" * 64},
            arm_reports=arms,
        )

        self.assertFalse(report["instrument_ready"])
        self.assertEqual(report["pilot_go_no_go"], "NO-GO")
        self.assertEqual(
            report["next_design_decision"],
            "a26_invalid_requires_forward_only_diagnosis",
        )

    def test_nextjs_control_requires_observed_stale_route(self):
        scenario = SCENARIOS["nextjs-fix"]
        target_hash = "b" * 64
        manifest = {scenario.target_path: target_hash}
        rows = [
            event("recovery_plan_auto_run_configured", recovery_plan_auto_runs=0),
            event(
                "fix_evidence_recorded",
                requirement_id="before_fails",
                binding_id=scenario.verify_commands[0],
                executed=True,
                outcome="failure",
            ),
            event("recovery_prompt_saved", status="incomplete"),
            event(
                "ultra_final_acceptance",
                ok=False,
                verdict="failed",
                final_acceptance_status="failed",
                assurance_level="failed",
            ),
        ]
        diagnostics = [
            {"command": command, "returncode": 1 if index == 0 else 0}
            for index, command in enumerate(scenario.verify_commands)
        ]

        missing_route = _build_arm_report(
            scenario=scenario,
            arm="control",
            recovery_auto_runs=0,
            rows=rows,
            process_returncode=0,
            provider_trace=[],
            input_manifest=manifest,
            final_manifest=manifest,
            input_protected_manifest={},
            final_protected_manifest={},
            diagnostics=diagnostics,
            route=None,
            binary_sha256="a" * 64,
        )
        self.assertFalse(missing_route["checks"]["route_endpoint_remained_failed"])
        self.assertFalse(missing_route["arm_valid"])

        observed_route = _build_arm_report(
            scenario=scenario,
            arm="control",
            recovery_auto_runs=0,
            rows=rows,
            process_returncode=0,
            provider_trace=[],
            input_manifest=manifest,
            final_manifest=manifest,
            input_protected_manifest={},
            final_protected_manifest={},
            diagnostics=diagnostics,
            route={"http_observed": True, "target_text": "stale-02"},
            binary_sha256="a" * 64,
        )
        self.assertTrue(observed_route["arm_valid"])

    def test_fixture_manifests_cover_all_preregistered_scenarios(self):
        for scenario_id in SCENARIO_ORDER:
            with self.subTest(scenario_id=scenario_id):
                digest = fixture_manifest_sha256(scenario_id)
                self.assertEqual(len(digest), 64)
                int(digest, 16)

    def test_generator_freezes_paired_instrument_scope(self):
        code_sha = "a" * 40
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary = root / "commandagent"
            binary.write_bytes(b"binary")
            node_modules = root / "node_modules"
            node_modules.mkdir()
            (node_modules / ".package-lock.json").write_text("{}\n", encoding="utf-8")
            with (
                patch.object(a26_generator, "_validate_exact_sha_evidence"),
                patch.object(
                    a26_generator,
                    "_binary_version",
                    return_value=f"commandagent 0.1.0 {code_sha[:8]}",
                ),
            ):
                contract = a26_generator.build_contract(
                    code_sha=code_sha,
                    exact_sha_ci_evidence="eval/goal_verify/v0/fake.json",
                    commandagent_bin=binary,
                    node_modules_source=node_modules,
                    authorized=True,
                )

        self.assertEqual(contract["design"]["arm_order"], ["control", "treatment"])
        self.assertEqual(
            contract["design"]["recovery_auto_runs"],
            {"control": 0, "treatment": 1},
        )
        self.assertFalse(contract["effect_claim_allowed"])
        self.assertFalse(contract["full_collection_allowed"])
        self.assertTrue(contract["authorization"]["pilot_collection_authorized"])
        self.assertFalse(
            contract["authorization"]["confirmatory_collection_authorized"]
        )

    def test_a27_generator_keeps_scope_and_freezes_forward_corrections(self):
        code_sha = "b" * 40
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary = root / "commandagent"
            binary.write_bytes(b"binary")
            node_modules = root / "node_modules"
            node_modules.mkdir()
            (node_modules / ".package-lock.json").write_text("{}\n", encoding="utf-8")
            with (
                patch.object(a26_generator, "_validate_exact_sha_evidence"),
                patch.object(
                    a26_generator,
                    "_binary_version",
                    return_value=f"commandagent 0.1.0 {code_sha[:8]}",
                ),
            ):
                contract = a27_generator.build_contract(
                    code_sha=code_sha,
                    exact_sha_ci_evidence="eval/goal_verify/v0/fake.json",
                    commandagent_bin=binary,
                    node_modules_source=node_modules,
                    authorized=True,
                )

        self.assertEqual(contract["contract_id"], a27_generator.CONTRACT_ID)
        self.assertEqual(contract["supersedes_contract"], a27_generator.A26_CONTRACT_ID)
        self.assertEqual(contract["design"]["scenario_order"], list(SCENARIO_ORDER))
        self.assertIn(
            "__pycache__",
            contract["forward_corrections"][
                "protected_manifest_runtime_cache_exclusions"
            ],
        )
        self.assertIn(
            "symlinks=True",
            contract["forward_corrections"]["nextjs_provisioning_symlink_policy"],
        )
        self.assertFalse(contract["effect_claim_allowed"])

    def test_frozen_a26_contract_keeps_historical_scope(self):
        path = (
            ROOT / "eval/goal_verify/v0/"
            "phase6-recovery-deterministic-a26-pilot-contract.json"
        )
        contract = json.loads(path.read_text(encoding="utf-8"))

        self.assertEqual(
            contract["code_sha"], "1ba6a257baa0625e29833584d76a6609518f0dd3"
        )
        self.assertEqual(contract["design"]["scenario_order"], list(SCENARIO_ORDER))
        self.assertFalse(contract["effect_claim_allowed"])
        self.assertFalse(contract["generalization_claim_allowed"])

    def test_frozen_a27_contract_matches_forward_only_sources(self):
        path = (
            ROOT / "eval/goal_verify/v0/"
            "phase6-recovery-deterministic-a27-pilot-contract.json"
        )
        contract = json.loads(path.read_text(encoding="utf-8"))

        self.assertEqual(contract_errors(contract), [])
        self.assertEqual(
            contract["code_sha"], "7c5e99eb9e246358aed3c25b3f3b0ea77c6da2be"
        )
        self.assertEqual(contract["supersedes_contract"], a27_generator.A26_CONTRACT_ID)
        self.assertFalse(contract["estimand"]["confirmatory_effect_estimate_in_a27"])
        self.assertFalse(contract["effect_claim_allowed"])
        self.assertFalse(contract["generalization_claim_allowed"])

    def test_protected_manifest_ignores_runtime_cache_files(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            tests = root / "tests"
            tests.mkdir()
            (tests / "test_pipeline.py").write_text("assert True\n", encoding="utf-8")
            before = _path_manifest(root, ("tests",))
            cache = tests / "__pycache__"
            cache.mkdir()
            (cache / "test_pipeline.pyc").write_bytes(b"cache")
            pytest_cache = tests / ".pytest_cache"
            pytest_cache.mkdir()
            (pytest_cache / "state").write_text("state\n", encoding="utf-8")
            after = _path_manifest(root, ("tests",))

        self.assertEqual(before, after)

    def test_nextjs_provisioning_copy_preserves_executable_symlink(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source"
            executable = source / "next/dist/bin/next"
            executable.parent.mkdir(parents=True)
            executable.write_text("#!/usr/bin/env node\n", encoding="utf-8")
            bin_dir = source / ".bin"
            bin_dir.mkdir()
            (bin_dir / "next").symlink_to("../next/dist/bin/next")
            workspace = root / "workspace"
            workspace.mkdir()

            _write_nextjs_fix_fixture(workspace, source)
            copied = workspace / "node_modules/.bin/next"

            self.assertTrue(copied.is_symlink())
            self.assertEqual(copied.readlink(), Path("../next/dist/bin/next"))


if __name__ == "__main__":
    unittest.main()
