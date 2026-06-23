import importlib.util
import json
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

JOB_REPORT_SPEC = importlib.util.spec_from_file_location(
    "eval_runtime_job_report", ROOT / "scripts" / "eval_runtime_job_report.py"
)
eval_runtime_job_report = importlib.util.module_from_spec(JOB_REPORT_SPEC)
JOB_REPORT_SPEC.loader.exec_module(eval_runtime_job_report)

CASE_SCHEMA_SPEC = importlib.util.spec_from_file_location(
    "eval_case_schema", ROOT / "scripts" / "eval_case_schema.py"
)
eval_case_schema = importlib.util.module_from_spec(CASE_SCHEMA_SPEC)
CASE_SCHEMA_SPEC.loader.exec_module(eval_case_schema)


class EvalReportCategorizeTests(unittest.TestCase):
    def test_layer_categories(self):
        cases = {
            "plan_lint.profile_obligations:missing literal": "planning",
            "plan_lint.invalid_expected_path": "planning",
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
            "plan_lint.invalid_expected_path": "plan_lint_failed",
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

    def test_eval_timeout_is_provider_transport_evidence(self):
        observation = eval_failure_observation.normalize_observation(
            {
                "reason": "provider_transport:eval_timeout",
                "success": False,
                "stderr": "ERROR: eval command timed out after 1200s",
            }
        )

        self.assertEqual(observation["terminal_state"], "provider_transport_failed")
        self.assertEqual(observation["failure_category"], "provider_transport")
        self.assertEqual(observation["diagnostic_code"], "provider_transport:eval_timeout")

    def test_ollama_native_tool_xml_error_is_provider_parse_evidence(self):
        observation = eval_failure_observation.normalize_observation(
            {
                "reason": "rc:1",
                "success": False,
                "stderr": (
                    'ERROR: initial turn error: model error: Ollama ollama failed: '
                    'status 500: {"error":"XML syntax error on line 4: '
                    'element <function> closed by </parameter>"}'
                ),
            }
        )

        self.assertEqual(observation["terminal_state"], "provider_parse_failed")
        self.assertEqual(observation["failure_category"], "provider_transport")
        self.assertEqual(observation["contract_layer"], "execution_contract")

    def test_raw_rc_with_loop_exhaustion_uses_specific_diagnostic(self):
        observation = eval_failure_observation.normalize_observation(
            {
                "reason": "rc:1",
                "success": False,
                "stderr": "ERROR: initial turn error: minimal loop reached max iterations",
            }
        )

        self.assertEqual(observation["terminal_state"], "verifier_command_failed")
        self.assertEqual(observation["diagnostic_code"], "minimal_loop_max_iterations")

    def test_raw_rc_with_blocked_bash_uses_tool_policy_diagnostic(self):
        observation = eval_failure_observation.normalize_observation(
            {
                "reason": "rc:1",
                "success": False,
                "evidence": (
                    "reason: turn_error\n"
                    "diagnostic: tool error: bash command blocked as Unknown: "
                    "compound shell commands, pipes, redirects, and shell substitutions "
                    "are blocked"
                ),
            }
        )

        self.assertEqual(observation["terminal_state"], "verifier_command_failed")
        self.assertEqual(observation["diagnostic_code"], "blocked_bash_command_policy")

    def test_plan_lint_invalid_expected_path_has_specific_diagnostic(self):
        observation = eval_failure_observation.normalize_observation(
            {
                "reason": "plan_lint.invalid_expected_path",
                "success": False,
            }
        )

        self.assertEqual(observation["terminal_state"], "plan_lint_failed")
        self.assertEqual(observation["failure_category"], "planning")
        self.assertEqual(observation["diagnostic_code"], "plan_lint:invalid_expected_path")

    def test_semantic_mismatch_reason_wins_over_stale_repair_evidence(self):
        observation = eval_failure_observation.normalize_observation(
            {
                "reason": "semantic_mismatch:README.md:PHASE7_IMPOSSIBLE_LITERAL",
                "success": False,
                "evidence": "- evidence_binding_status=failed",
            }
        )

        self.assertEqual(observation["terminal_state"], "eval_assertion_failed")
        self.assertEqual(observation["failure_category"], "quality")
        self.assertEqual(observation["contract_layer"], "eval_success_contract")

    def test_profile_dependency_conflict_maps_to_manifest_repair(self):
        reason = "profile_verification:nextjs_dependency_version_conflict"

        self.assertEqual(eval_report.derive_active_job(reason), "manifest_repair")
        self.assertEqual(eval_report.derive_recovery_owner(reason), "manifest")
        self.assertEqual(
            eval_report.derive_repair_action(reason),
            "resolve_manifest_conflict",
        )

    def test_phase26_expected_fields_are_parsed_for_focused_cases(self):
        with tempfile.TemporaryDirectory() as tmp:
            case_path = pathlib.Path(tmp) / "case.yaml"
            case_path.write_text(
                "\n".join(
                    [
                        "id: phase26-field-parse",
                        "profile: nextjs",
                        "style: default",
                        "prompt: test",
                        "expected_loop_control_action: run_bounded_repair_task",
                        "expected_setup_readiness: dependency_missing",
                        "expected_setup_command_authority: verifier_owned_setup_only",
                        "expected_requested_port: 3011",
                        "expected_port_preflight: available",
                        "expected_endpoint_smoke: timeout",
                        "expected_profile_project_kind: nextjs",
                        "expected_profile_failure_mapping: route|manifest",
                        "expected_selected_failure_cluster: route:profile_contract",
                        "expected_repair_root_cause: route not integrated",
                        "expected_repair_hypothesis: connect route",
                        "expected_expected_improvement: profile verification passes",
                        "expected_success_check: npm run build",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            parsed = eval_case_schema.read_eval_case(case_path)

        expected = parsed["expected_fields"]
        self.assertEqual(expected["loop_control_action"], "run_bounded_repair_task")
        self.assertEqual(expected["setup_readiness"], "dependency_missing")
        self.assertEqual(expected["requested_port"], "3011")
        self.assertEqual(expected["port_preflight"], "available")
        self.assertEqual(expected["endpoint_smoke"], "timeout")
        self.assertEqual(expected["profile_project_kind"], "nextjs")
        self.assertEqual(expected["selected_failure_cluster"], "route:profile_contract")
        self.assertEqual(expected["expected_improvement"], "profile verification passes")

    def test_phase27_expected_fields_are_parsed_for_target_patch_and_rollback(self):
        with tempfile.TemporaryDirectory() as tmp:
            case_path = pathlib.Path(tmp) / "case.yaml"
            case_path.write_text(
                "\n".join(
                    [
                        "id: phase27-field-parse",
                        "profile: rust",
                        "style: default",
                        "prompt: test",
                        "expected_target_candidate_count: 3",
                        "expected_target_admitted_count: 1",
                        "expected_target_rejected_count: 2",
                        "expected_current_excerpt_available: true",
                        "expected_target_priority_components: source=verifier_diagnostic",
                        "expected_patch_validation_source: mechanical_adapter",
                        "expected_patch_validation_outcomes: noop|duplicate",
                        "expected_patch_validation_rejected_paths: src/lib.rs",
                        "expected_mechanical_adapter: rust_compile_diagnostic",
                        "expected_mechanical_adapter_status: admitted",
                        "expected_mechanical_adapter_action: repair_rust_compile_error",
                        "expected_rollback_admission_status: rejected",
                        "expected_rollback_reason: safe_rollback_data_missing",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            parsed = eval_case_schema.read_eval_case(case_path)

        expected = parsed["expected_fields"]
        self.assertEqual(expected["target_candidate_count"], "3")
        self.assertEqual(expected["target_admitted_count"], "1")
        self.assertEqual(expected["target_rejected_count"], "2")
        self.assertEqual(expected["current_excerpt_available"], "true")
        self.assertEqual(
            expected["target_priority_components"], "source=verifier_diagnostic"
        )
        self.assertEqual(expected["patch_validation_source"], "mechanical_adapter")
        self.assertEqual(expected["patch_validation_outcomes"], "noop|duplicate")
        self.assertEqual(
            expected["mechanical_adapter_action"], "repair_rust_compile_error"
        )
        self.assertEqual(expected["rollback_admission_status"], "rejected")

    def test_phase28_expected_fields_are_parsed_for_contract_conflict(self):
        with tempfile.TemporaryDirectory() as tmp:
            case_path = pathlib.Path(tmp) / "case.yaml"
            case_path.write_text(
                "\n".join(
                    [
                        "id: phase28-field-parse",
                        "profile: rust",
                        "style: default",
                        "prompt: test",
                        "expected_contract_conflict_status: resolved",
                        "expected_contract_conflict_sides: implementation|test",
                        "expected_contract_conflict_authority: test_authoritative",
                        "expected_contract_conflict_repair_target_side: implementation",
                        "expected_contract_conflict_selected_action: edit_source_for_diagnostic",
                        "expected_contract_conflict_safe_stop_reason: none",
                        "expected_contract_conflict_missing_evidence: none",
                        "expected_contract_conflict_source_of_truth: test_contract_and_original_verifier",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            parsed = eval_case_schema.read_eval_case(case_path)

        expected = parsed["expected_fields"]
        self.assertEqual(expected["contract_conflict_status"], "resolved")
        self.assertEqual(
            expected["contract_conflict_sides"], "implementation|test"
        )
        self.assertEqual(
            expected["contract_conflict_authority"], "test_authoritative"
        )
        self.assertEqual(
            expected["contract_conflict_repair_target_side"], "implementation"
        )
        self.assertEqual(
            expected["contract_conflict_selected_action"],
            "edit_source_for_diagnostic",
        )
        self.assertEqual(expected["contract_conflict_safe_stop_reason"], "none")
        self.assertEqual(expected["contract_conflict_missing_evidence"], "none")
        self.assertEqual(
            expected["contract_conflict_source_of_truth"],
            "test_contract_and_original_verifier",
        )

    def test_phase29_expected_fields_are_parsed_for_runtime_support(self):
        with tempfile.TemporaryDirectory() as tmp:
            case_path = pathlib.Path(tmp) / "case.yaml"
            case_path.write_text(
                "\n".join(
                    [
                        "id: phase29-field-parse",
                        "profile: python",
                        "style: default",
                        "prompt: test",
                        "expected_phase29_support_rows: C35|C37|C39|C43",
                        "expected_language_repair_adapter_status: projected",
                        "expected_effective_tool_policy: file_mutation_repair",
                        "expected_effective_tool_policy_status: projected",
                        "expected_tool_failure_recovery_status: bounded_correction",
                        "expected_setup_command_classification: verifier",
                        "expected_command_authority: original_verifier",
                        "expected_command_classification_reason: command_is_an_original_verifier_or_test_runner",
                        "expected_workspace_candidate_status: observed:1|excluded:2",
                        "expected_workspace_ignored_dir_policy: single_source_of_truth",
                        "expected_workspace_candidate_ignored_reasons: build_output|dependency_cache",
                        "expected_job_report_status: projected",
                        "expected_job_report_owner_action: source_implementation_repair:edit_source_for_diagnostic",
                        "expected_scaffold_contract_status: artifact_obligation",
                        "expected_noncoding_evidence_status: generic_producer",
                        "expected_answer_work_mode_status: deterministic_gate",
                        "expected_lifecycle_projection_status: selected",
                        "expected_provider_boundary_status: transport_only",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            parsed = eval_case_schema.read_eval_case(case_path)

        expected = parsed["expected_fields"]
        self.assertEqual(expected["phase29_support_rows"], "C35|C37|C39|C43")
        self.assertEqual(expected["setup_command_classification"], "verifier")
        self.assertEqual(expected["command_authority"], "original_verifier")
        self.assertEqual(
            expected["workspace_ignored_dir_policy"], "single_source_of_truth"
        )
        self.assertEqual(expected["provider_boundary_status"], "transport_only")

    def test_phase29_runtime_support_report_section(self):
        report = eval_report.render_report(
            [
                {
                    "case_id": "phase29-runtime-support",
                    "run": "1",
                    "rc": "1",
                    "elapsed_ms": "1",
                    "success": "false",
                    "reason": "provider_transport:eval_timeout",
                    "failure_category": "provider_transport",
                    "contract_layer": "execution_contract",
                    "phase29_support_rows": "C35|C36|C39|C43|C44",
                    "effective_tool_policy": "tool_protocol_correction",
                    "effective_tool_policy_status": "projected",
                    "tool_failure_recovery_status": "bounded_correction",
                    "provider_boundary_status": "transport_only",
                    "job_report_status": "projected",
                    "lifecycle_projection_status": "selected",
                }
            ]
        )

        self.assertIn("## Phase29 Runtime Support", report)
        self.assertIn("- support_rows=C35|C36|C39|C43|C44: 1", report)
        self.assertIn("- provider_boundary_status=transport_only: 1", report)

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

    def test_completion_authority_fields_classify_stale_evidence(self):
        observation = eval_failure_observation.normalize_observation(
            {
                "reason": "rc:1",
                "success": False,
                "evidence": "\n".join(
                    [
                        "- terminal_state=stale_evidence",
                        "- completion_authority_status=stale_evidence",
                        "- evidence_runner_status=executed",
                        "- completion_evidence_status=stale",
                        "- freshness_status=stale",
                        "- artifact_ledger_status=complete",
                        "- stale_evidence: kind=repo_edit target=src/lib.rs status=stale",
                    ]
                ),
            }
        )

        self.assertEqual(observation["terminal_state"], "stale_evidence")
        self.assertEqual(observation["failure_category"], "quality")
        self.assertEqual(observation["freshness_status"], "stale")
        self.assertEqual(
            observation["completion_authority_status"], "stale_evidence"
        )
        self.assertEqual(observation["stale_evidence"], "kind=repo_edit target=src/lib.rs status=stale")

    def test_recheck_reads_indented_repair_evidence(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            run_dir = root / "large-fastapi-app-modify" / "run-1"
            repairs = run_dir / "workspace" / ".commandagent" / "repairs"
            repairs.mkdir(parents=True)
            (run_dir / "stdout.txt").write_text("", encoding="utf-8")
            (run_dir / "stderr.txt").write_text(
                "ERROR: step verify-tests failed verification\n",
                encoding="utf-8",
            )
            (repairs / "repair.md").write_text(
                "\n".join(
                    [
                        "Contract correction evidence:",
                        "  - diagnostic_code: fastapi_response_mismatch",
                        "  - target_path: app/main.py",
                        "  - repair_target: app/main.py",
                        "  - active_job: source_implementation_repair",
                        "  - artifact_role: entrypoint",
                        "  - repair_action: repair_source_error",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            (run_dir / "meta.json").write_text(
                json.dumps(
                    {
                        "case_id": "large-fastapi-app-modify",
                        "run_index": 1,
                        "rc": 1,
                        "success": False,
                        "elapsed_ms": 1,
                        "success_check_reason": "rc:1",
                        "matrix_row": "large-fastapi-app-modify",
                        "proof_mode": "real_llm",
                        "dry_run": False,
                    }
                )
                + "\n",
                encoding="utf-8",
            )

            rows, _ = eval_report.recheck(
                root,
                {
                    "large-fastapi-app-modify": {
                        "required_paths": [],
                        "must_include": {},
                        "type": "semantic",
                        "matrix_row": "large-fastapi-app-modify",
                        "proof_mode": "real_llm",
                    }
                },
            )

        row = rows[0]
        self.assertEqual(row["diagnostic_code"], "fastapi_response_mismatch")
        self.assertEqual(row["target_path"], "app/main.py")
        self.assertEqual(row["terminal_state"], "verifier_command_failed")
        self.assertEqual(row["evidence_binding_status"], "bound")
        self.assertEqual(row["completion_evidence_status"], "failed")

    def test_recheck_admits_existing_profile_entrypoint_target_for_raw_rc_evidence(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            run_dir = root / "large-rust-app-new" / "run-1"
            workspace = run_dir / "workspace"
            repairs = workspace / ".commandagent" / "repairs"
            (workspace / "src").mkdir(parents=True)
            repairs.mkdir(parents=True)
            (workspace / "src" / "main.rs").write_text(
                "fn main() {}\n", encoding="utf-8"
            )
            (run_dir / "stdout.txt").write_text("", encoding="utf-8")
            (run_dir / "stderr.txt").write_text(
                "ERROR: initial turn error: minimal loop reached max iterations\n",
                encoding="utf-8",
            )
            (repairs / "repair.md").write_text(
                "\n".join(
                    [
                        "Verification failures:",
                        "- command: repair turn",
                        "- reason: turn_error",
                        "- diagnostic: tool error: bash command blocked as Unknown: "
                        "compound shell commands, pipes, redirects, and shell "
                        "substitutions are blocked",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            (run_dir / "meta.json").write_text(
                json.dumps(
                    {
                        "case_id": "large-rust-app-new",
                        "run_index": 1,
                        "rc": 1,
                        "success": False,
                        "elapsed_ms": 1,
                        "success_check_reason": "rc:1",
                        "matrix_row": "large-rust-app-new",
                        "proof_mode": "real_llm",
                        "dry_run": False,
                        "active_job": "source_implementation_repair",
                        "recovery_owner": "source",
                        "repair_action": "edit_source_for_diagnostic",
                        "profile_entrypoints": "src/main.rs|src/lib.rs",
                        "profile_integration_artifacts": "src/main.rs|src/lib.rs",
                    }
                )
                + "\n",
                encoding="utf-8",
            )

            rows, _ = eval_report.recheck(
                root,
                {
                    "large-rust-app-new": {
                        "required_paths": [],
                        "must_include": {},
                        "type": "semantic",
                        "matrix_row": "large-rust-app-new",
                        "proof_mode": "real_llm",
                    }
                },
            )

        row = rows[0]
        self.assertEqual(row["diagnostic_code"], "blocked_bash_command_policy")
        self.assertEqual(row["target_path"], "src/main.rs")
        self.assertEqual(row["selected_target"], "src/main.rs")
        self.assertEqual(row["target_admission_status"], "admitted")
        self.assertEqual(row["target_source_of_truth"], "profile_artifact_hint")
        self.assertEqual(row["target_ownership_source"], "profile_workspace_artifact")

    def test_recheck_does_not_invent_target_without_existing_profile_artifact(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            run_dir = root / "large-rust-app-new" / "run-1"
            workspace = run_dir / "workspace"
            workspace.mkdir(parents=True)
            (run_dir / "stdout.txt").write_text("", encoding="utf-8")
            (run_dir / "stderr.txt").write_text(
                "ERROR: initial turn error: minimal loop reached max iterations\n",
                encoding="utf-8",
            )
            (run_dir / "meta.json").write_text(
                json.dumps(
                    {
                        "case_id": "large-rust-app-new",
                        "run_index": 1,
                        "rc": 1,
                        "success": False,
                        "elapsed_ms": 1,
                        "success_check_reason": "rc:1",
                        "matrix_row": "large-rust-app-new",
                        "proof_mode": "real_llm",
                        "dry_run": False,
                        "active_job": "source_implementation_repair",
                        "recovery_owner": "source",
                        "repair_action": "edit_source_for_diagnostic",
                        "profile_entrypoints": "src/main.rs|src/lib.rs",
                    }
                )
                + "\n",
                encoding="utf-8",
            )

            rows, _ = eval_report.recheck(
                root,
                {
                    "large-rust-app-new": {
                        "required_paths": [],
                        "must_include": {},
                        "type": "semantic",
                        "matrix_row": "large-rust-app-new",
                        "proof_mode": "real_llm",
                    }
                },
            )

        row = rows[0]
        self.assertEqual(row["diagnostic_code"], "minimal_loop_max_iterations")
        self.assertEqual(row["target_path"], "")
        self.assertEqual(row["target_admission_status"], "unknown")

    def test_recheck_reprojects_fixture_fields_for_focused_assertions(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            run_dir = root / "focused-artifact-ledger-producers" / "run-1"
            workspace = run_dir / "workspace"
            workspace.mkdir(parents=True)
            (run_dir / "stdout.txt").write_text("", encoding="utf-8")
            (run_dir / "stderr.txt").write_text("", encoding="utf-8")
            (run_dir / "meta.json").write_text(
                json.dumps(
                    {
                        "case_id": "focused-artifact-ledger-producers",
                        "run_index": 1,
                        "rc": 0,
                        "success": True,
                        "elapsed_ms": 1,
                        "success_check_reason": "ok",
                        "matrix_row": "phase24-c07-artifact-ledger-producers",
                        "proof_mode": "deterministic_fixture",
                        "fixture_fields": {
                            "artifact_ledger_entries": "4",
                            "artifact_ledger_sources": "completion_authority:1",
                            "changed_paths": "[app/page.tsx]",
                            "workspace_scope_kind": "greenfield",
                        },
                        "dry_run": False,
                    }
                )
                + "\n",
                encoding="utf-8",
            )

            rows, _ = eval_report.recheck(
                root,
                {
                    "focused-artifact-ledger-producers": {
                        "required_paths": [],
                        "must_include": {},
                        "type": "semantic",
                        "matrix_row": "phase24-c07-artifact-ledger-producers",
                        "proof_mode": "deterministic_fixture",
                        "expected_fields": {
                            "artifact_ledger_entries": "4",
                            "artifact_ledger_sources": "completion_authority:1",
                            "changed_paths": "[app/page.tsx]",
                            "workspace_scope_kind": "greenfield",
                            "lifecycle_stage": "completed",
                            "completion_source": "runtime_success",
                        },
                    }
                },
            )

        row = rows[0]
        self.assertEqual(row["artifact_ledger_entries"], "4")
        self.assertEqual(row["artifact_ledger_sources"], "completion_authority:1")
        self.assertEqual(row["changed_paths"], "[app/page.tsx]")
        self.assertEqual(row["workspace_scope_kind"], "greenfield")
        self.assertEqual(row["expected_assertion_status"], "passed_recheck")

    def test_recheck_preserves_explicit_meta_observation_fields(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            run_dir = root / "focused-explicit-stop" / "run-1"
            (run_dir / "workspace").mkdir(parents=True)
            (run_dir / "stdout.txt").write_text("", encoding="utf-8")
            (run_dir / "stderr.txt").write_text(
                "ERROR: step verify-tests failed verification\n",
                encoding="utf-8",
            )
            (run_dir / "meta.json").write_text(
                json.dumps(
                    {
                        "case_id": "focused-explicit-stop",
                        "run_index": 1,
                        "rc": 1,
                        "success": False,
                        "elapsed_ms": 1,
                        "success_check_reason": "rc:1",
                        "matrix_row": "phase33-explicit-stop",
                        "proof_mode": "deterministic_fixture",
                        "terminal_state": "explicit_stop",
                        "active_job": "explicit_stop",
                        "recovery_owner": "explicit_stop",
                        "target_admission_status": "rejected",
                        "completion_source": "none",
                        "fixture_fields": {
                            "explicit_stop_reason": "protected_raw_input",
                            "repair_action": "stop_for_contract_conflict",
                        },
                        "dry_run": False,
                    }
                )
                + "\n",
                encoding="utf-8",
            )

            rows, _ = eval_report.recheck(
                root,
                {
                    "focused-explicit-stop": {
                        "required_paths": [],
                        "must_include": {},
                        "type": "semantic",
                        "matrix_row": "phase33-explicit-stop",
                        "proof_mode": "deterministic_fixture",
                        "expected_fields": {
                            "terminal_state": "explicit_stop",
                            "active_job": "explicit_stop",
                            "active_owner": "explicit_stop",
                            "target_admission_status": "rejected",
                            "completion_source": "none",
                            "explicit_stop_reason": "protected_raw_input",
                        },
                    }
                },
            )

        row = rows[0]
        self.assertEqual(row["terminal_state"], "explicit_stop")
        self.assertEqual(row["active_job"], "explicit_stop")
        self.assertEqual(row["active_owner"], "explicit_stop")
        self.assertEqual(row["target_admission_status"], "rejected")
        self.assertEqual(row["completion_source"], "none")
        self.assertEqual(row["explicit_stop_reason"], "protected_raw_input")
        self.assertEqual(row["expected_assertion_status"], "passed_recheck")

    def test_failure_projection_preserves_observed_evidence_states(self):
        projection = eval_runtime_job_report.large_failure_projection(
            {
                "evidence_binding_status": "failed",
                "completion_evidence_status": "stale",
                "attempt_outcome": "duplicate",
                "runtime_job_outcome": "no_progress",
            },
            success=False,
            reason="rc:1",
            terminal_state="verifier_command_failed",
            failure_category="verifier",
            diagnostic_code="rust_compile_error",
        )

        self.assertNotIn("evidence_binding_status", projection)
        self.assertNotIn("completion_evidence_status", projection)
        self.assertNotIn("attempt_outcome", projection)
        self.assertNotIn("runtime_job_outcome", projection)

    def test_target_admission_status_preserves_explicit_unknown(self):
        self.assertEqual(
            eval_runtime_job_report.target_admission_status(
                {"target_admission_status": "unknown", "target_path": "src/lib.rs"},
                "source_implementation_repair",
                False,
            ),
            "unknown",
        )

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

    def test_render_report_includes_completion_freshness_sections(self):
        report = eval_report.render_report(
            [
                {
                    "case_id": "stale",
                    "run": "1",
                    "rc": "1",
                    "elapsed_ms": "10",
                    "success": "false",
                    "reason": "rc:1",
                    "terminal_state": "stale_evidence",
                    "completion_authority_status": "stale_evidence",
                    "completion_source_of_truth": "completion_evidence_freshness",
                    "evidence_runner_status": "executed",
                    "evidence_runner_kind": "verifier",
                    "evidence_binding_kind": "file_layout",
                    "freshness_status": "stale",
                    "artifact_ledger_status": "complete",
                }
            ]
        )

        self.assertIn("- stale_evidence: 1", report)
        self.assertIn("- completion_authority_status=stale_evidence: 1", report)
        self.assertIn(
            "- completion_source_of_truth=completion_evidence_freshness: 1",
            report,
        )
        self.assertIn("- evidence_runner_kind=verifier: 1", report)
        self.assertIn("- evidence_binding_kind=file_layout: 1", report)
        self.assertIn("- freshness_status=stale: 1", report)

    def test_runtime_job_report_distinguishes_dry_run_from_runtime_success(self):
        report = eval_runtime_job_report.build_runtime_job_report(
            {
                "success": "false",
                "reason": "missing:package.json",
                "active_job": "manifest_repair",
                "recovery_owner": "manifest",
                "repair_action": "add_missing_manifest_dependency",
                "target_path": "package.json",
                "repair_brief_status": "admitted",
                "action_envelope_status": "admitted",
            },
            dry_run=True,
        )

        self.assertEqual(report["lifecycle_stage"], "dry_run_placeholder")
        self.assertEqual(report["completion_source"], "dry_run_placeholder_success")
        self.assertEqual(report["active_owner"], "manifest")
        self.assertEqual(report["target_admission_status"], "admitted")
        self.assertEqual(report["repair_action_plan_status"], "planned")

    def test_runtime_job_report_ignores_none_explicit_stop_and_passed_outcome_action(self):
        report = eval_runtime_job_report.build_runtime_job_report(
            {
                "success": "true",
                "reason": "ok",
                "active_job": "none",
                "recovery_owner": "none",
                "repair_action": "none",
                "loop_control_action": "none",
                "runtime_job_outcome": "passed",
                "explicit_stop_reason": "none",
            }
        )

        self.assertEqual(report["lifecycle_stage"], "completed")
        self.assertEqual(report["selected_action"], "none")

    def test_runtime_job_report_distinguishes_recheck_failure(self):
        report = eval_runtime_job_report.build_runtime_job_report(
            {
                "success": "false",
                "reason": "semantic_missing:README.md",
                "active_job": "documentation_repair",
                "recovery_owner": "docs",
            },
            recheck=True,
        )

        self.assertEqual(report["lifecycle_stage"], "rechecking")
        self.assertEqual(report["completion_source"], "recheck_failure")

    def test_runtime_job_report_projects_provider_timeout_ownership(self):
        report = eval_runtime_job_report.build_runtime_job_report(
            {
                "success": "false",
                "reason": "provider_transport:eval_timeout",
                "terminal_state": "provider_transport_failed",
                "failure_category": "provider_transport",
                "diagnostic_code": "provider_transport:eval_timeout",
            }
        )

        self.assertEqual(report["active_job"], "provider_transport_blocker")
        self.assertEqual(report["active_owner"], "provider_transport")
        self.assertEqual(report["recovery_owner"], "provider_transport")
        self.assertEqual(report["selected_action"], "stop_for_provider_timeout")
        self.assertEqual(report["repair_action"], "stop_for_provider_timeout")
        self.assertEqual(report["target_path"], "not_applicable")
        self.assertEqual(report["target_admission_status"], "not_applicable")
        self.assertEqual(report["evidence_binding_status"], "not_applicable")
        self.assertEqual(report["completion_evidence_status"], "not_applicable")
        self.assertEqual(report["attempt_outcome"], "blocked_external")

    def test_runtime_job_report_overrides_profile_dependency_source_fallback(self):
        report = eval_runtime_job_report.build_runtime_job_report(
            {
                "success": "false",
                "reason": "semantic_missing:components/AnalyticsPanel.tsx",
                "terminal_state": "profile_contract_failed",
                "failure_category": "profile",
                "diagnostic_code": "profile_verification:nextjs_dependency_version_conflict",
                "active_job": "source_implementation_repair",
                "recovery_owner": "source",
                "repair_action": "edit_source_for_diagnostic",
            }
        )

        self.assertEqual(report["active_job"], "manifest_repair")
        self.assertEqual(report["active_owner"], "manifest")
        self.assertEqual(report["recovery_owner"], "manifest")
        self.assertEqual(report["selected_action"], "resolve_manifest_conflict")
        self.assertEqual(report["repair_action"], "resolve_manifest_conflict")
        self.assertEqual(report["target_path"], "package.json")
        self.assertEqual(report["target_role"], "setup_manifest")
        self.assertEqual(report["target_admission_status"], "admitted")
        self.assertEqual(report["evidence_binding_status"], "bound")
        self.assertEqual(report["completion_evidence_status"], "failed")
        self.assertEqual(report["attempt_outcome"], "failed")

    def test_focused_assertions_accept_recheck_success_equivalents(self):
        result = eval_case_schema.focused_assertions(
            {
                "lifecycle_stage": "completed",
                "completion_source": "runtime_success",
            },
            {
                "lifecycle_stage": "rechecking",
                "completion_source": "recheck_success",
            },
            recheck=True,
        )

        self.assertEqual(result["expected_assertion_status"], "passed_recheck")
        self.assertEqual(result["expected_assertion_failures"], "")

    def test_focused_assertions_accept_recheck_failure_equivalents(self):
        result = eval_case_schema.focused_assertions(
            {
                "lifecycle_stage": "repairing",
                "completion_source": "none",
            },
            {
                "lifecycle_stage": "rechecking",
                "completion_source": "recheck_failure",
            },
            recheck=True,
        )

        self.assertEqual(result["expected_assertion_status"], "passed_recheck")
        self.assertEqual(result["expected_assertion_failures"], "")

    def test_focused_assertions_still_fail_recheck_opposite_outcome(self):
        result = eval_case_schema.focused_assertions(
            {"completion_source": "runtime_success"},
            {"completion_source": "recheck_failure"},
            recheck=True,
        )

        self.assertEqual(result["expected_assertion_status"], "failed_recheck")
        self.assertIn("completion_source", result["expected_assertion_failures"])

    def test_focused_assertions_accept_non_ok_contract_layer_recheck_projection(self):
        result = eval_case_schema.focused_assertions(
            {"contract_layer": "setup_bootstrap_contract"},
            {"contract_layer": "verification_contract"},
            recheck=True,
        )

        self.assertEqual(result["expected_assertion_status"], "passed_recheck")

    def test_focused_assertions_do_not_hide_ok_contract_layer_recheck_failure(self):
        result = eval_case_schema.focused_assertions(
            {"contract_layer": "ok"},
            {"contract_layer": "verification_contract"},
            recheck=True,
        )

        self.assertEqual(result["expected_assertion_status"], "failed_recheck")
        self.assertIn("contract_layer", result["expected_assertion_failures"])

    def test_render_report_includes_runtime_job_lifecycle_funnel(self):
        report = eval_report.render_report(
            [
                {
                    "case_id": "phase14",
                    "run": "1",
                    "rc": "0",
                    "elapsed_ms": "10",
                    "success": "true",
                    "reason": "ok",
                    "lifecycle_stage": "completed",
                    "active_owner": "none",
                    "selected_action": "none",
                    "target_admission_status": "not_applicable",
                    "repair_action_plan_status": "not_applicable",
                    "completion_source": "runtime_success",
                    "attempt_outcome": "passed",
                    "verifier_rerun_result": "passed",
                }
            ]
        )

        self.assertIn("## Runtime Job Lifecycle", report)
        self.assertIn("- lifecycle_stage=completed: 1", report)
        self.assertIn("- active_owner=none: 1", report)
        self.assertIn("- selected_action=none: 1", report)
        self.assertIn("- target_admission_status=not_applicable: 1", report)
        self.assertIn("- repair_action_plan_status=not_applicable: 1", report)
        self.assertIn("- completion_source=runtime_success: 1", report)
        self.assertIn("- attempt_outcome=passed: 1", report)
        self.assertIn("- verifier_rerun_result=passed: 1", report)

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
                    "allowed_change_kind": "tool_call_shape_only",
                    "allowed_tool_category": "tool_protocol",
                    "target_confidence": "tool_protocol_only",
                    "tool_protocol_status": "admitted",
                    "tool_protocol_source": "tool_argument_schema",
                    "tool_protocol_action": "emit_same_tool_with_required_fields",
                    "tool_protocol_failed_tool": "Write",
                    "tool_protocol_missing_field": "path",
                    "tool_protocol_required_fields": "path,content",
                    "tool_protocol_correction_spent": "false",
                    "tool_protocol_correction_exhausted": "false",
                    "repair_plan_rejection_reason": "none",
                    "selected_failure_cluster": "tool_protocol:tool_args_missing_required_field",
                    "semantic_failure_kind": "tool_protocol_failure",
                    "diagnostic_failure_kind": "verifier_contract_failure",
                    "semantic_cluster_source_of_truth": "verifier_contract",
                    "preferred_repair_role": "verifier_contract",
                    "observed_expected": "observed=invalid_tool_call_expected=valid_tool_call",
                    "affected_cases": "Write.path",
                    "candidate_artifacts": "src/main.rs",
                    "weak_verifier_reason": "source_grep_verifies_text_not_behavior",
                    "contract_conflict_status": "resolved",
                    "contract_conflict_authority": "test_authoritative",
                    "contract_conflict_repair_target_side": "implementation",
                    "contract_conflict_selected_action": "edit_source_for_diagnostic",
                    "contract_conflict_safe_stop_reason": "none",
                    "contract_conflict_source_of_truth": "test_contract_and_original_verifier",
                    "admitted_cluster_targets": "src/main.rs",
                    "unknown_diagnostic_count": "0",
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
        self.assertIn("## Repair Action Envelope", report)
        self.assertIn("- allowed_change_kind=tool_call_shape_only: 1", report)
        self.assertIn("- allowed_tool_category=tool_protocol: 1", report)
        self.assertIn("- target_confidence=tool_protocol_only: 1", report)
        self.assertIn("## Selected Failure Clusters", report)
        self.assertIn("## Semantic Failure Kinds", report)
        self.assertIn("- tool_protocol_failure: 1", report)
        self.assertIn("## Diagnostic Failure Kinds", report)
        self.assertIn("- verifier_contract_failure: 1", report)
        self.assertIn("## Semantic Cluster Sources", report)
        self.assertIn("- verifier_contract: 1", report)
        self.assertIn("## Preferred Repair Roles", report)
        self.assertIn("- verifier_contract: 1", report)
        self.assertIn("## Observed/Expected Pairs", report)
        self.assertIn("- observed=invalid_tool_call_expected=valid_tool_call: 1", report)
        self.assertIn("## Affected Cases", report)
        self.assertIn("- Write.path: 1", report)
        self.assertIn("## Candidate Artifacts", report)
        self.assertIn("- src/main.rs: 1", report)
        self.assertIn("## Weak Verifier Reasons", report)
        self.assertIn("- source_grep_verifies_text_not_behavior: 1", report)
        self.assertIn("## Contract Conflict Decisions", report)
        self.assertIn("- status=resolved: 1", report)
        self.assertIn("- authority=test_authoritative: 1", report)
        self.assertIn("- repair_target_side=implementation: 1", report)
        self.assertIn("- selected_action=edit_source_for_diagnostic: 1", report)
        self.assertIn("- safe_stop_reason=none: 1", report)
        self.assertIn(
            "- source_of_truth=test_contract_and_original_verifier: 1", report
        )
        self.assertIn("## Admitted Cluster Targets", report)
        self.assertIn("- src/main.rs: 1", report)
        self.assertIn("## Unknown Diagnostic Count", report)
        self.assertIn("- total: 0", report)
        self.assertIn("## Tool Protocol Recovery", report)
        self.assertIn("- status=admitted: 1", report)
        self.assertIn("- source=tool_argument_schema: 1", report)
        self.assertIn("- action=emit_same_tool_with_required_fields: 1", report)
        self.assertIn("- failed_tool=Write: 1", report)
        self.assertIn("- missing_field=path: 1", report)
        self.assertIn("- required_fields=path,content: 1", report)
        self.assertIn("- correction_spent=false: 1", report)
        self.assertIn("- correction_exhausted=false: 1", report)

    def test_render_report_includes_patch_mechanical_and_rollback_sections(self):
        report = eval_report.render_report(
            [
                {
                    "case_id": "phase12",
                    "run": "1",
                    "rc": "1",
                    "elapsed_ms": "10",
                    "success": "false",
                    "reason": "patch_validation:test_weakening",
                    "failure_category": "verifier",
                    "contract_layer": "recovery_contract",
                    "patch_validation_status": "rejected",
                    "patch_validation_source": "model_tool_edit",
                    "patch_validation_outcomes": "test_weakening",
                    "patch_validation_rejected_paths": "tests/app_test.rs",
                    "mechanical_adapter": "rust_compile_diagnostic",
                    "mechanical_adapter_status": "admitted",
                    "mechanical_adapter_action": "repair_rust_compile_error",
                    "rollback_admission_status": "rejected",
                    "rollback_reason": "safe_rollback_data_missing",
                }
            ]
        )

        self.assertIn("## Patch Validation", report)
        self.assertIn("- status=rejected: 1", report)
        self.assertIn("- source=model_tool_edit: 1", report)
        self.assertIn("- outcomes=test_weakening: 1", report)
        self.assertIn("- rejected_paths=tests/app_test.rs: 1", report)
        self.assertIn("## Mechanical Repair Adapters", report)
        self.assertIn("- adapter=rust_compile_diagnostic: 1", report)
        self.assertIn("- status=admitted: 1", report)
        self.assertIn("- action=repair_rust_compile_error: 1", report)
        self.assertIn("## Rollback Admission", report)
        self.assertIn("- status=rejected: 1", report)
        self.assertIn("- reason=safe_rollback_data_missing: 1", report)

    def test_render_report_includes_profile_parity_sections(self):
        report = eval_report.render_report(
            [
                {
                    "case_id": "phase13",
                    "run": "1",
                    "rc": "0",
                    "elapsed_ms": "10",
                    "success": "true",
                    "reason": "ok",
                    "failure_category": "ok",
                    "contract_layer": "none",
                    "profile_project_kind": "nextjs_app",
                    "profile_manifest_artifacts": "package.json",
                    "profile_entrypoints": "app/page.tsx",
                    "profile_integration_artifacts": "app/page.tsx|components/Game.tsx",
                    "profile_completion_evidence": "npm_run_build|selected_route_binding",
                    "profile_failure_mapping": "nextjs_missing_dependency->manifest_repair",
                    "profile_adapter_families": "node_next_type|nextjs_route_integration",
                    "profile_capability_status": "project:ok|manifest:ok|adapter:ok",
                }
            ]
        )

        self.assertIn("## Profile Parity", report)
        self.assertIn("- project_kind=nextjs_app: 1", report)
        self.assertIn("- manifest_artifacts=package.json: 1", report)
        self.assertIn("- entrypoints=app/page.tsx: 1", report)
        self.assertIn(
            "- integration_artifacts=app/page.tsx|components/Game.tsx: 1",
            report,
        )
        self.assertIn(
            "- completion_evidence=npm_run_build|selected_route_binding: 1",
            report,
        )
        self.assertIn(
            "- failure_mapping=nextjs_missing_dependency->manifest_repair: 1",
            report,
        )
        self.assertIn(
            "- adapter_families=node_next_type|nextjs_route_integration: 1",
            report,
        )
        self.assertIn("- capability_status=project:ok|manifest:ok|adapter:ok: 1", report)

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
                    "task_contract_lifecycle": "projected",
                    "task_contract_request_signals": "explicit_intent:new:intent",
                    "task_contract_constraints": "profile:profile:nextjs",
                    "task_contract_completion_evidence": "artifact_exists:required_artifact:package.json",
                    "behavior_obligation_codes": "nextjs_dependencies_required",
                    "behavior_obligation_status": "projected",
                    "behavior_obligation_owners": "nextjs_dependencies_required:setup",
                    "behavior_obligation_paths": "nextjs_dependencies_required:package.json",
                    "artifact_role_projection_status": "projected",
                }
            ]
        )

        self.assertIn("## Task Contract", report)
        self.assertIn("- kind=new: 1", report)
        self.assertIn("- status=admitted: 1", report)
        self.assertIn("- lifecycle=projected: 1", report)
        self.assertIn("- request_signals=explicit_intent:new:intent: 1", report)
        self.assertIn("- constraints=profile:profile:nextjs: 1", report)
        self.assertIn(
            "- completion_evidence=artifact_exists:required_artifact:package.json: 1",
            report,
        )
        self.assertIn("## Behavior Obligations", report)
        self.assertIn("- status=projected: 1", report)
        self.assertIn("- owners=nextjs_dependencies_required:setup: 1", report)
        self.assertIn("- paths=nextjs_dependencies_required:package.json: 1", report)
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

    def test_explicit_stop_with_reason_is_not_unknown_contract_defect(self):
        report = eval_report.render_report(
            [
                {
                    "case_id": "explicit",
                    "run": "1",
                    "rc": "1",
                    "elapsed_ms": "10",
                    "success": "false",
                    "reason": "explicit_stop",
                    "terminal_state": "explicit_stop",
                    "explicit_stop_reason": "contract_conflict",
                }
            ]
        )

        self.assertIn("## Unknown/Raw Failure Coverage Defects", report)
        self.assertIn("- none", report)

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
                        "expected_active_job_lifecycle: not_applicable",
                        "expected_diagnostic_failure_kind: assertion_mismatch",
                        "expected_unknown_diagnostic_count: 0",
                        "expected_lifecycle_stage: completed",
                        "expected_completion_source: runtime_success",
                        "matrix_row: docs-literal",
                        "proof_mode: deterministic_fixture",
                        "",
                    ]
                ),
                encoding="utf-8",
            )

            cases = eval_report.read_cases(root)

        self.assertIn("focused-assertion", cases)
        self.assertEqual(cases["focused-assertion"]["expected_fields"]["terminal_state"], "ok")
        self.assertEqual(cases["focused-assertion"]["expected_fields"]["active_job"], "none")
        self.assertEqual(
            cases["focused-assertion"]["expected_fields"]["active_job_lifecycle"],
            "not_applicable",
        )
        self.assertEqual(
            cases["focused-assertion"]["expected_fields"]["diagnostic_failure_kind"],
            "assertion_mismatch",
        )
        self.assertEqual(
            cases["focused-assertion"]["expected_fields"]["unknown_diagnostic_count"],
            "0",
        )
        self.assertEqual(
            cases["focused-assertion"]["expected_fields"]["lifecycle_stage"],
            "completed",
        )
        self.assertEqual(
            cases["focused-assertion"]["expected_fields"]["completion_source"],
            "runtime_success",
        )
        self.assertEqual(cases["focused-assertion"]["matrix_row"], "docs-literal")
        self.assertEqual(
            cases["focused-assertion"]["proof_mode"], "deterministic_fixture"
        )

    def test_read_eval_case_parses_fixture_fields(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = pathlib.Path(tmp) / "case.yaml"
            path.write_text(
                "\n".join(
                    [
                        "id: fixture-case",
                        "profile: docs",
                        "style: default",
                        "prompt: \"Create README.md\"",
                        "proof_mode: deterministic_fixture",
                        "fixture_reason: tool_args_missing_required_field:path",
                        "fixture_success: false",
                        "fixture_rc: 1",
                        "fixture_fields:",
                        "  active_job: tool_protocol_correction",
                        "  tool_protocol_missing_field: path",
                        "",
                    ]
                ),
                encoding="utf-8",
            )

            case = eval_case_schema.read_eval_case(path)

        self.assertEqual(case["proof_mode"], "deterministic_fixture")
        self.assertEqual(
            case["fixture_reason"], "tool_args_missing_required_field:path"
        )
        self.assertEqual(
            case["fixture_fields"]["active_job"], "tool_protocol_correction"
        )
        self.assertEqual(case["fixture_fields"]["tool_protocol_missing_field"], "path")

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

    def test_render_report_includes_focused_matrix_section(self):
        report = eval_report.render_report(
            [
                {
                    "case_id": "matrix",
                    "run": "1",
                    "rc": "1",
                    "elapsed_ms": "10",
                    "success": "false",
                    "reason": "tool_args_missing_required_field:path",
                    "matrix_row": "tool-protocol-missing-field",
                    "proof_mode": "deterministic_fixture",
                    "expected_assertion_status": "passed",
                }
            ]
        )

        self.assertIn("## Focused Matrix", report)
        self.assertIn("- proof_mode=deterministic_fixture: 1", report)
        self.assertIn("- matrix_row=tool-protocol-missing-field: 1", report)


if __name__ == "__main__":
    unittest.main()
