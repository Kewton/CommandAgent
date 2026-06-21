import importlib.util
import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "eval_report", ROOT / "scripts" / "eval_report.py"
)
eval_report = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(eval_report)

OBS_SPEC = importlib.util.spec_from_file_location(
    "eval_failure_observation", ROOT / "scripts" / "eval_failure_observation.py"
)
eval_failure_observation = importlib.util.module_from_spec(OBS_SPEC)
OBS_SPEC.loader.exec_module(eval_failure_observation)


class EvalReportCategorizeTests(unittest.TestCase):
    def test_layer_categories(self):
        cases = {
            "plan_lint.profile_obligations:missing literal": "planning",
            "Gemini JSON parse failed: tool call is missing a tool name": "provider_transport",
            "tool_args_missing_required_field": "tool_protocol",
            "read_only_step_mutation": "step_policy",
            "profile_verification:nextjs_route_not_integrated": "profile",
            "dependency_missing": "setup",
            "port_in_use": "setup",
            "quality:blank_ui": "quality",
            "rc:1": "verifier",
            "command_failed:1": "verifier",
        }
        for reason, expected in cases.items():
            with self.subTest(reason=reason):
                self.assertEqual(eval_report.categorize(reason), expected)

    def test_semantic_mismatches_are_case_insensitive_for_semantic_checks(self):
        workspace = ROOT / "target" / "eval-report-test-workspace"
        workspace.mkdir(parents=True, exist_ok=True)
        readme = workspace / "README.md"
        readme.write_text("# CommandAgent\n\n## Usage\n", encoding="utf-8")

        mismatches = eval_report.semantic_mismatches(
            workspace,
            {
                "type": "semantic",
                "must_include": {"README.md": ["usage"]},
            },
            [],
        )

        self.assertEqual(mismatches, [])

    def test_terminal_state_observation_for_common_failures(self):
        cases = {
            "ok": "ok",
            "invalid plan YAML: unsupported block scalar style for instruction: >-": "plan_parse_failed",
            "plan_lint.profile_obligations:missing literal": "plan_lint_failed",
            "Gemini JSON parse failed: tool call is missing a tool name": "provider_parse_failed",
            "tool_args_missing_required_field:path": "tool_protocol_failed",
            "step_policy:read_only_step_mutation": "step_policy_failed",
            "profile_verification:nextjs_route_not_integrated": "profile_contract_failed",
            "dependency_missing": "dependency_missing",
            "setup:npm_eresolve_peer_dependency": "setup_failed",
            "missing:app/page.tsx": "missing_deliverable",
            "semantic_mismatch:README.md:usage": "eval_assertion_failed",
            "rc:1": "verifier_command_failed",
        }
        for reason, expected in cases.items():
            with self.subTest(reason=reason):
                observation = eval_failure_observation.normalize_observation(
                    {"reason": reason, "success": reason == "ok"}
                )
                self.assertEqual(observation["terminal_state"], expected)

    def test_eaddrinuse_is_port_in_use(self):
        observation = eval_failure_observation.normalize_observation(
            {
                "reason": "rc:1",
                "success": False,
                "stderr": "Error: listen EADDRINUSE: address already in use :::3011",
            }
        )

        self.assertEqual(observation["terminal_state"], "port_in_use")
        self.assertEqual(observation["failure_category"], "setup")
        self.assertEqual(observation["contract_layer"], "setup_bootstrap_contract")
        self.assertEqual(observation["port"], "3011")

    def test_transport_failure_wins_over_missing_artifacts(self):
        observation = eval_failure_observation.normalize_observation(
            {
                "reason": "missing:README.md",
                "success": False,
                "stderr": "ERROR: Ollama transport failed: error sending request for url",
            }
        )

        self.assertEqual(observation["terminal_state"], "provider_transport_failed")
        self.assertEqual(observation["failure_category"], "provider_transport")
        self.assertEqual(observation["contract_layer"], "execution_contract")

    def test_completion_authority_fields_classify_missing_evidence(self):
        observation = eval_failure_observation.normalize_observation(
            {
                "reason": "rc:1",
                "success": False,
                "evidence": "\n".join(
                    [
                        "- terminal_state=missing_evidence",
                        "- evidence_runner_status=missing",
                        "- completion_evidence_status=missing",
                        "- artifact_ledger_status=complete",
                    ]
                ),
            }
        )

        self.assertEqual(observation["terminal_state"], "missing_evidence")
        self.assertEqual(observation["failure_category"], "quality")
        self.assertEqual(observation["evidence_runner_status"], "missing")
        self.assertEqual(observation["artifact_ledger_status"], "complete")

    def test_missing_deliverable_wins_over_generic_evidence_status(self):
        observation = eval_failure_observation.normalize_observation(
            {
                "reason": "missing:app/page.tsx",
                "success": False,
                "completion_evidence_status": "failed",
                "artifact_ledger_status": "missing_required",
            }
        )

        self.assertEqual(observation["terminal_state"], "missing_deliverable")
        self.assertEqual(observation["artifact_ledger_status"], "missing_required")

    def test_render_report_backfills_terminal_state_for_legacy_rows(self):
        report = eval_report.render_report(
            [
                {
                    "case_id": "legacy",
                    "run": "1",
                    "rc": "1",
                    "elapsed_ms": "10",
                    "success": "false",
                    "reason": "dependency_missing",
                    "failure_category": "",
                    "contract_layer": "",
                }
            ]
        )

        self.assertIn("## Terminal States", report)
        self.assertIn("- dependency_missing: 1", report)
        self.assertIn("## Diagnostic Codes", report)
        self.assertIn("## Evidence Authority", report)

    def test_render_report_includes_dispatch_sections(self):
        report = eval_report.render_report(
            [
                {
                    "case_id": "dispatch",
                    "run": "1",
                    "rc": "1",
                    "elapsed_ms": "10",
                    "success": "false",
                    "reason": "tool_args_missing_required_field:path",
                    "failure_category": "tool_protocol",
                    "contract_layer": "execution_contract",
                    "active_job": "tool_protocol_correction",
                    "loop_control_action": "run_tool_protocol_correction",
                    "dispatch_status": "selected",
                }
            ]
        )

        self.assertIn("## Dispatch Status", report)
        self.assertIn("- selected: 1", report)
        self.assertIn("## Loop Control Actions", report)
        self.assertIn("- run_tool_protocol_correction: 1", report)


if __name__ == "__main__":
    unittest.main()
