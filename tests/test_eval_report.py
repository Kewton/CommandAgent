import importlib.util
import pathlib
import tempfile
import unittest
import csv


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
        self.assertEqual(observation["contract_layer"], "dev_server_port_contract")
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

    def test_artifact_ledger_signal_fields_are_extracted(self):
        observation = eval_failure_observation.normalize_observation(
            {
                "reason": "rc:1",
                "success": False,
                "evidence": "\n".join(
                    [
                        "- workspace_scope_kind=single_project_root",
                        "- workspace_scope_roots=[.]",
                        "- artifact_ledger_entries=2",
                        "- artifact_ledger_summary=entries:2;overflow:false",
                        "- artifact_ownership=owned",
                        "- artifact_ownership_reason=changed_by_tool",
                        "- artifact_source_of_truth=tool_record",
                        "- rejected_target_reason=candidate_without_ownership",
                        "- read_paths=[package.json]",
                        "- changed_paths=[app/page.tsx]",
                        "- verifier_mentioned_paths=[app/page.tsx]",
                        "- out_of_scope_paths=[node_modules/react/index.js]",
                    ]
                ),
            }
        )

        self.assertEqual(observation["workspace_scope_kind"], "single_project_root")
        self.assertEqual(observation["workspace_scope_roots"], "[.]")
        self.assertEqual(observation["artifact_ledger_entries"], "2")
        self.assertEqual(
            observation["artifact_ledger_summary"], "entries:2;overflow:false"
        )
        self.assertEqual(observation["artifact_ownership"], "owned")
        self.assertEqual(observation["artifact_ownership_reason"], "changed_by_tool")
        self.assertEqual(observation["artifact_source_of_truth"], "tool_record")
        self.assertEqual(
            observation["rejected_target_reason"], "candidate_without_ownership"
        )
        self.assertEqual(observation["read_paths"], "[package.json]")
        self.assertEqual(observation["changed_paths"], "[app/page.tsx]")
        self.assertEqual(observation["verifier_mentioned_paths"], "[app/page.tsx]")
        self.assertEqual(
            observation["out_of_scope_paths"], "[node_modules/react/index.js]"
        )

    def test_contract_evidence_diagnostic_code_wins_over_raw_rc(self):
        observation = eval_failure_observation.normalize_observation(
            {
                "reason": "rc:1",
                "success": False,
                "evidence": "\n".join(
                    [
                        "- diagnostic_code=rust_compile_error",
                        "- source_of_truth=original_verifier_diagnostic",
                        "- command: cargo check",
                    ]
                ),
            }
        )

        self.assertEqual(observation["terminal_state"], "verifier_command_failed")
        self.assertEqual(observation["diagnostic_code"], "rust_compile_error")
        self.assertEqual(observation["source_of_truth"], "original_verifier_diagnostic")
        self.assertEqual(observation["command"], "cargo check")

    def test_python_failure_taxonomy_matches_shared_fixture(self):
        taxonomy_path = ROOT / "scripts" / "failure_observation_taxonomy.tsv"
        with open(taxonomy_path, encoding="utf-8", newline="") as handle:
            rows = {
                row["terminal_state"]: row
                for row in csv.DictReader(handle, delimiter="\t")
            }

        self.assertEqual(
            set(rows),
            set(eval_failure_observation.TERMINAL_STATE_TAXONOMY),
        )
        for terminal_state, row in rows.items():
            self.assertEqual(
                eval_failure_observation.TERMINAL_STATE_TO_CATEGORY[terminal_state],
                row["failure_class"],
            )
            self.assertEqual(
                eval_failure_observation.TERMINAL_STATE_TO_CONTRACT_LAYER[terminal_state],
                row["contract_layer"],
            )
            self.assertEqual(
                eval_failure_observation.TERMINAL_STATE_TO_VIOLATED_CONTRACT[
                    terminal_state
                ],
                row["violated_contract"],
            )

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
        self.assertIn("## Failure Observation Summary", report)
        self.assertIn("## Producer Coverage", report)

    def test_render_report_includes_artifact_ledger_signal_sections(self):
        report = eval_report.render_report(
            [
                {
                    "case_id": "ledger",
                    "run": "1",
                    "rc": "1",
                    "elapsed_ms": "10",
                    "success": "false",
                    "reason": "rc:1",
                    "workspace_scope_kind": "single_project_root",
                    "artifact_ownership": "candidate_only",
                    "artifact_source_of_truth": "verifier_output",
                    "rejected_target_reason": "candidate_without_ownership",
                    "read_paths": "[package.json]",
                    "changed_paths": "[app/page.tsx]",
                    "out_of_scope_paths": "[node_modules/react/index.js]",
                }
            ]
        )

        self.assertIn("- workspace_scope_kind=single_project_root: 1", report)
        self.assertIn("## Artifact Ledger Signals", report)
        self.assertIn("- read_paths: 1", report)
        self.assertIn("- changed_paths: 1", report)
        self.assertIn("- out_of_scope_paths: 1", report)
        self.assertIn("## Artifact Ownership", report)
        self.assertIn("- ownership=candidate_only: 1", report)
        self.assertIn("- source_of_truth=verifier_output: 1", report)
        self.assertIn("## Rejected Targets", report)
        self.assertIn("- candidate_without_ownership: 1", report)

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
                    "repair_brief_status": "admitted",
                    "action_envelope_status": "admitted",
                    "selected_failure_cluster": "tool_protocol:tool_args_missing_required_field",
                    "semantic_failure_kind": "tool_protocol_failure",
                    "preferred_repair_role": "verifier_contract",
                    "weak_verifier_reason": "source_grep_verifies_text_not_behavior",
                    "admitted_cluster_targets": "src/main.rs",
                    "runtime_job_kind": "tool_protocol_correction",
                }
            ]
        )

        self.assertIn("## Dispatch Status", report)
        self.assertIn("- selected: 1", report)
        self.assertIn("## Loop Control Actions", report)
        self.assertIn("- run_tool_protocol_correction: 1", report)
        self.assertIn("## Repair Brief Status", report)
        self.assertIn("- admitted: 1", report)
        self.assertIn("## Runtime Jobs", report)
        self.assertIn("- tool_protocol_correction: 1", report)
        self.assertIn("## Action Envelope Status", report)
        self.assertIn("## Selected Failure Clusters", report)
        self.assertIn("## Semantic Failure Kinds", report)
        self.assertIn("- tool_protocol_failure: 1", report)
        self.assertIn("## Preferred Repair Roles", report)
        self.assertIn("- verifier_contract: 1", report)
        self.assertIn("## Weak Verifier Reasons", report)
        self.assertIn("- source_grep_verifies_text_not_behavior: 1", report)
        self.assertIn("## Admitted Cluster Targets", report)
        self.assertIn("- src/main.rs: 1", report)

    def test_render_report_includes_task_contract_sections(self):
        report = eval_report.render_report(
            [
                {
                    "case_id": "task-contract",
                    "run": "1",
                    "rc": "1",
                    "elapsed_ms": "10",
                    "success": "false",
                    "reason": "plan_lint.task_contract:missing owner",
                    "failure_category": "planning",
                    "contract_layer": "planning_contract",
                    "task_contract_kind": "new",
                    "task_contract_status": "admitted",
                    "behavior_obligation_codes": "nextjs_dependencies_required",
                    "behavior_obligation_status": "projected",
                    "artifact_role_projection_status": "projected",
                }
            ]
        )

        self.assertIn("## Task Contract", report)
        self.assertIn("- kind=new: 1", report)
        self.assertIn("- status=admitted: 1", report)
        self.assertIn("## Behavior Obligations", report)
        self.assertIn("- status=projected: 1", report)
        self.assertIn("## Artifact Role Projection", report)

    def test_render_report_flags_raw_rc_without_diagnostic(self):
        report = eval_report.render_report(
            [
                {
                    "case_id": "raw-rc",
                    "run": "1",
                    "rc": "1",
                    "elapsed_ms": "10",
                    "success": "false",
                    "reason": "rc:1",
                    "failure_category": "verifier",
                    "contract_layer": "verification_contract",
                    "terminal_state": "verifier_command_failed",
                    "diagnostic_code": "rc_1",
                    "producer": "verifier",
                    "actionability": "actionable",
                }
            ]
        )

        self.assertIn("## Failure Observation Summary", report)
        self.assertIn("- producer=verifier: 1", report)
        self.assertIn("- actionability=actionable: 1", report)
        self.assertIn("## Unknown/Raw Failure Coverage Defects", report)
        self.assertIn("raw-rc: raw_reason=rc:1 diagnostic_code=rc_1", report)

    def test_read_cases_recurses_and_parses_expected_fields(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            nested = root / "focused" / "case.yaml"
            nested.parent.mkdir(parents=True)
            nested.write_text(
                "\n".join(
                    [
                        "id: focused-assertion",
                        "profile: docs",
                        "style: default",
                        "prompt: \"Create README.md\"",
                        "expected_artifacts:",
                        "  - README.md",
                        "verify:",
                        "  - cat README.md",
                        "success_check:",
                        "  type: semantic",
                        "  required_paths:",
                        "    - README.md",
                        "expected_terminal_state: ok",
                        "expected_active_job: none",
                        "",
                    ]
                ),
                encoding="utf-8",
            )

            cases = eval_report.read_cases(root)

        self.assertIn("focused-assertion", cases)
        self.assertEqual(cases["focused-assertion"]["expected_fields"]["terminal_state"], "ok")
        self.assertEqual(cases["focused-assertion"]["expected_fields"]["active_job"], "none")

    def test_focused_assertion_mismatch_is_reported(self):
        report = eval_report.render_report(
            [
                {
                    "case_id": "focused",
                    "run": "1",
                    "rc": "0",
                    "elapsed_ms": "10",
                    "success": "true",
                    "reason": "ok",
                    "failure_category": "ok",
                    "contract_layer": "ok",
                    "terminal_state": "ok",
                    "active_job": "none",
                    "expected_assertion_status": "failed",
                    "expected_assertion_count": "1",
                    "expected_assertion_failures": "active_job:expected=setup_bootstrap;observed=none",
                }
            ]
        )

        self.assertIn("## Focused Assertions", report)
        self.assertIn("- failed: 1", report)
        self.assertIn("## Focused Assertion Failures", report)
        self.assertIn("active_job:expected=setup_bootstrap;observed=none", report)


if __name__ == "__main__":
    unittest.main()
