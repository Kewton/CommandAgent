import importlib.util
import pathlib
import sys
import tempfile
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "eval_signoff", ROOT / "scripts" / "eval_signoff.py"
)
eval_signoff = importlib.util.module_from_spec(SPEC)
sys.modules["eval_signoff"] = eval_signoff
SPEC.loader.exec_module(eval_signoff)


def write_summary(root: pathlib.Path, name: str, rows: list[dict[str, str]]) -> None:
    root.mkdir(parents=True, exist_ok=True)
    fieldnames = sorted({key for row in rows for key in row})
    lines = ["\t".join(fieldnames)]
    for row in rows:
        lines.append("\t".join(row.get(key, "") for key in fieldnames))
    (root / name).write_text("\n".join(lines) + "\n", encoding="utf-8")


def case_rows(prefix: str, count: int) -> list[dict[str, str]]:
    return [
        {
            "case_id": f"{prefix}-{index:03d}",
            "run": "1",
            "success": "true",
            "reason": "ok",
            "terminal_state": "ok",
            "contract_layer": "ok",
            "diagnostic_code": "ok",
        }
        for index in range(count)
    ]


def focused_rows(count: int) -> list[dict[str, str]]:
    return [
        {
            "case_id": f"focused-{index:03d}",
            "run": "1",
            "success": "true",
            "reason": "ok",
            "terminal_state": "ok",
            "contract_layer": "ok",
            "diagnostic_code": "ok",
            "expected_assertion_status": "passed_recheck",
        }
        for index in range(count)
    ]


def write_root(root: pathlib.Path, rows: list[dict[str, str]]) -> None:
    write_summary(root, "summary.tsv", rows)
    write_summary(root, "recheck_summary.tsv", rows)


def current_root_specs(base: pathlib.Path) -> list[object]:
    smoke = base / "smoke" / "run"
    focused = base / "focused-control-recovery" / "run"
    large = base / "large" / "run"
    write_root(smoke, case_rows("smoke", 3))
    write_root(focused, focused_rows(82))
    write_root(large, case_rows("large", 6))
    return [
        eval_signoff.RootSpec("smoke", smoke),
        eval_signoff.RootSpec("focused", focused),
        eval_signoff.RootSpec("large", large),
    ]


class EvalSignoffTests(unittest.TestCase):
    def test_admits_current_root_bundle(self):
        with tempfile.TemporaryDirectory() as tmp:
            specs = current_root_specs(pathlib.Path(tmp))

            admission = eval_signoff.admit_roots(
                specs,
                require_recheck=True,
                summary_name=None,
            )

        self.assertEqual(admission.findings, [])
        self.assertEqual(admission.current_case_coverage, 91)
        self.assertEqual(
            admission.family_case_counts,
            {"focused": 82, "large": 6, "small": 0, "smoke": 3},
        )

    def test_rejects_duplicate_root_label(self):
        with tempfile.TemporaryDirectory() as tmp:
            specs = current_root_specs(pathlib.Path(tmp))
            specs.append(
                eval_signoff.RootSpec(
                    "focused",
                    pathlib.Path(tmp) / "focused-control-recovery-copy" / "run",
                )
            )
            write_root(specs[-1].path, focused_rows(82))

            admission = eval_signoff.admit_roots(
                specs,
                require_recheck=True,
                summary_name=None,
            )

        self.assertIn("duplicate_root_label", [item.code for item in admission.findings])

    def test_rejects_duplicate_root_path_under_different_labels(self):
        with tempfile.TemporaryDirectory() as tmp:
            specs = current_root_specs(pathlib.Path(tmp))
            focused_path = specs[1].path
            specs.append(eval_signoff.RootSpec("focused-fixture", focused_path))

            admission = eval_signoff.admit_roots(
                specs,
                require_recheck=True,
                summary_name=None,
            )

        self.assertIn("duplicate_root_path", [item.code for item in admission.findings])

    def test_rejects_missing_required_family(self):
        with tempfile.TemporaryDirectory() as tmp:
            specs = [
                spec
                for spec in current_root_specs(pathlib.Path(tmp))
                if spec.family != "large"
            ]

            admission = eval_signoff.admit_roots(
                specs,
                require_recheck=True,
                summary_name=None,
            )

        codes = [item.code for item in admission.findings]
        self.assertIn("missing_required_root", codes)
        self.assertIn("current_case_coverage_mismatch", codes)

    def test_rejects_historical_smaller_focused_root(self):
        with tempfile.TemporaryDirectory() as tmp:
            specs = current_root_specs(pathlib.Path(tmp))
            write_root(specs[1].path, focused_rows(47))

            admission = eval_signoff.admit_roots(
                specs,
                require_recheck=True,
                summary_name=None,
            )

        codes = [item.code for item in admission.findings]
        self.assertIn("root_case_count_mismatch", codes)
        self.assertIn("current_case_coverage_mismatch", codes)

    def test_allows_absent_small_when_expected_zero(self):
        with tempfile.TemporaryDirectory() as tmp:
            specs = current_root_specs(pathlib.Path(tmp))

            admission = eval_signoff.admit_roots(
                specs,
                require_recheck=True,
                summary_name=None,
            )

        self.assertNotIn("missing_required_root", [item.code for item in admission.findings])
        self.assertEqual(admission.family_case_counts["small"], 0)

    def test_admission_requires_recheck_summary_when_requested(self):
        with tempfile.TemporaryDirectory() as tmp:
            specs = current_root_specs(pathlib.Path(tmp))
            (specs[0].path / "recheck_summary.tsv").unlink()

            admission = eval_signoff.admit_roots(
                specs,
                require_recheck=True,
                summary_name=None,
            )

        self.assertIn("missing_recheck_summary", [item.code for item in admission.findings])

    def test_passes_owned_large_failure(self):
        rows = [
            {
                "case_id": "large-rust",
                "run": "1",
                "success": "false",
                "reason": "command_failed:1",
                "terminal_state": "verifier_command_failed",
                "failure_category": "verifier",
                "contract_layer": "verification_contract",
                "diagnostic_code": "rust_compile_error",
                "active_job": "source_implementation_repair",
                "recovery_owner": "source",
                "repair_action": "edit_source_for_diagnostic",
                "target_path": "src/main.rs",
                "evidence_binding_status": "bound",
                "completion_evidence_status": "failed",
                "attempt_outcome": "failed",
                "large_disposition": "closed_owned_failure",
                "large_disposition_reason": "owned_verifier",
                "large_disposition_owner_action_status": "consistent",
                "large_disposition_evidence": "owner=source;target=src/main.rs",
            }
        ]
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            write_summary(root, "summary.tsv", rows)
            spec = eval_signoff.RootSpec("large", root)

            self.assertEqual(eval_signoff.classify(spec, rows), [])

    def test_flags_raw_smoke_failure(self):
        rows = [
            {
                "case_id": "smoke",
                "run": "1",
                "success": "false",
                "reason": "rc:1",
                "terminal_state": "verifier_command_failed",
                "contract_layer": "verification_contract",
                "diagnostic_code": "rc_1",
            }
        ]
        spec = eval_signoff.RootSpec("smoke", pathlib.Path("root"))

        findings = eval_signoff.classify(spec, rows)

        self.assertEqual([item.code for item in findings], ["raw_undiagnostic_rc"])

    def test_accepts_classified_rc_failure_with_admitted_target(self):
        rows = [
            {
                "case_id": "large-rust-app-new",
                "run": "1",
                "success": "false",
                "reason": "rc:1",
                "terminal_state": "verifier_command_failed",
                "failure_category": "verifier",
                "contract_layer": "verification_contract",
                "diagnostic_code": "blocked_bash_command_policy",
                "active_job": "source_implementation_repair",
                "recovery_owner": "source",
                "repair_action": "edit_source_for_diagnostic",
                "target_path": "src/main.rs",
                "target_admission_status": "admitted",
                "evidence_binding_status": "bound",
                "completion_evidence_status": "failed",
                "attempt_outcome": "failed",
                "large_disposition": "closed_owned_failure",
                "large_disposition_reason": "owned_tool_policy_failure",
                "large_disposition_owner_action_status": "consistent",
                "large_disposition_evidence": "owner=source;target=src/main.rs",
            }
        ]
        spec = eval_signoff.RootSpec("large", pathlib.Path("root"))

        self.assertEqual(eval_signoff.classify(spec, rows), [])

    def test_flags_focused_assertion_failure(self):
        rows = [
            {
                "case_id": "focused",
                "run": "1",
                "success": "true",
                "expected_assertion_status": "failed",
                "expected_assertion_failures": "active_job mismatch",
            }
        ]
        spec = eval_signoff.RootSpec("focused", pathlib.Path("root"))

        findings = eval_signoff.classify(spec, rows)

        self.assertEqual([item.code for item in findings], ["focused_assertion_failed"])
        self.assertIn("active_job mismatch", findings[0].detail)

    def test_flags_generic_source_fallback_for_setup_failure(self):
        rows = [
            {
                "case_id": "large-nextjs",
                "run": "1",
                "success": "false",
                "reason": "dependency_missing",
                "terminal_state": "dependency_missing",
                "failure_category": "setup",
                "contract_layer": "setup_contract",
                "diagnostic_code": "dependency_missing",
                "active_job": "source_implementation_repair",
                "recovery_owner": "source",
                "repair_action": "edit_source_for_diagnostic",
                "target_path": "app/page.tsx",
                "evidence_binding_status": "bound",
                "completion_evidence_status": "failed",
                "attempt_outcome": "failed",
            }
        ]
        spec = eval_signoff.RootSpec("large", pathlib.Path("root"))

        findings = eval_signoff.classify(spec, rows)

        self.assertIn("generic_source_fallback", [item.code for item in findings])

    def test_accepts_provider_timeout_not_applicable_evidence_when_owned(self):
        rows = [
            {
                "case_id": "large-python",
                "run": "1",
                "success": "false",
                "reason": "provider_transport:eval_timeout",
                "terminal_state": "provider_transport_failed",
                "failure_category": "provider_transport",
                "contract_layer": "execution_contract",
                "diagnostic_code": "provider_transport:eval_timeout",
                "active_job": "provider_transport_blocker",
                "recovery_owner": "provider_transport",
                "repair_action": "stop_for_provider_timeout",
                "target_path": "not_applicable",
                "evidence_binding_status": "not_applicable",
                "completion_evidence_status": "not_applicable",
                "attempt_outcome": "blocked_external",
                "large_disposition": "accepted_external_limitation",
                "large_disposition_reason": "provider_transport_timeout",
                "large_disposition_owner_action_status": "consistent",
                "large_disposition_evidence": "owner=provider_transport",
            }
        ]
        spec = eval_signoff.RootSpec("large", pathlib.Path("root"))

        self.assertEqual(eval_signoff.classify(spec, rows), [])

    def test_rejects_provider_timeout_not_applicable_without_owner_action(self):
        rows = [
            {
                "case_id": "large-python",
                "run": "1",
                "success": "false",
                "reason": "provider_transport:eval_timeout",
                "terminal_state": "provider_transport_failed",
                "failure_category": "provider_transport",
                "contract_layer": "execution_contract",
                "diagnostic_code": "provider_transport:eval_timeout",
                "target_path": "not_applicable",
                "evidence_binding_status": "not_applicable",
                "completion_evidence_status": "not_applicable",
                "attempt_outcome": "blocked_external",
                "large_disposition": "accepted_external_limitation",
                "large_disposition_reason": "provider_transport_timeout",
                "large_disposition_owner_action_status": "consistent",
                "large_disposition_evidence": "owner=provider_transport",
            }
        ]
        spec = eval_signoff.RootSpec("large", pathlib.Path("root"))

        findings = eval_signoff.classify(spec, rows)

        self.assertIn("missing_active_job", [item.code for item in findings])
        self.assertIn("missing_owner", [item.code for item in findings])
        self.assertIn("missing_action", [item.code for item in findings])
        self.assertIn("missing_evidence_binding", [item.code for item in findings])
        self.assertIn("missing_completion_evidence", [item.code for item in findings])

    def test_accepts_profile_manifest_dependency_target_projection(self):
        rows = [
            {
                "case_id": "large-nextjs-app-modify",
                "run": "1",
                "success": "false",
                "reason": "semantic_missing:components/AnalyticsPanel.tsx",
                "terminal_state": "profile_contract_failed",
                "failure_category": "profile",
                "contract_layer": "profile_contract",
                "diagnostic_code": "profile_verification:nextjs_dependency_version_conflict",
                "active_job": "manifest_repair",
                "recovery_owner": "manifest",
                "repair_action": "add_missing_manifest_dependency",
                "target_path": "package.json",
                "target_role": "setup_manifest",
                "evidence_binding_status": "bound",
                "completion_evidence_status": "failed",
                "attempt_outcome": "failed",
                "large_disposition": "closed_owned_failure",
                "large_disposition_reason": "owned_manifest_conflict",
                "large_disposition_owner_action_status": "consistent",
                "large_disposition_evidence": "owner=manifest;target=package.json",
            }
        ]
        spec = eval_signoff.RootSpec("large", pathlib.Path("root"))

        self.assertEqual(eval_signoff.classify(spec, rows), [])

    def test_rejects_profile_contract_not_applicable_evidence(self):
        rows = [
            {
                "case_id": "large-nextjs-app-modify",
                "run": "1",
                "success": "false",
                "reason": "semantic_missing:components/AnalyticsPanel.tsx",
                "terminal_state": "profile_contract_failed",
                "failure_category": "profile",
                "contract_layer": "profile_contract",
                "diagnostic_code": "profile_verification:nextjs_dependency_version_conflict",
                "active_job": "manifest_repair",
                "recovery_owner": "manifest",
                "repair_action": "add_missing_manifest_dependency",
                "target_path": "package.json",
                "target_role": "setup_manifest",
                "evidence_binding_status": "not_applicable",
                "completion_evidence_status": "not_applicable",
                "attempt_outcome": "failed",
                "large_disposition": "closed_owned_failure",
                "large_disposition_reason": "owned_manifest_conflict",
                "large_disposition_owner_action_status": "consistent",
                "large_disposition_evidence": "owner=manifest;target=package.json",
            }
        ]
        spec = eval_signoff.RootSpec("large", pathlib.Path("root"))

        findings = eval_signoff.classify(spec, rows)

        self.assertIn("missing_evidence_binding", [item.code for item in findings])
        self.assertIn("missing_completion_evidence", [item.code for item in findings])

    def test_requires_large_disposition_for_failed_large_rows(self):
        rows = [
            {
                "case_id": "large-rust",
                "run": "1",
                "success": "false",
                "reason": "command_failed:1",
                "terminal_state": "verifier_command_failed",
                "failure_category": "verifier",
                "contract_layer": "verification_contract",
                "diagnostic_code": "rust_compile_error",
                "active_job": "source_implementation_repair",
                "recovery_owner": "source",
                "repair_action": "edit_source_for_diagnostic",
                "target_path": "src/main.rs",
                "evidence_binding_status": "bound",
                "completion_evidence_status": "failed",
                "attempt_outcome": "failed",
            }
        ]
        spec = eval_signoff.RootSpec("large", pathlib.Path("root"))

        findings = eval_signoff.classify(spec, rows)

        self.assertIn("missing_large_disposition", [item.code for item in findings])

    def test_rejects_large_external_limitation_without_provider_boundary(self):
        rows = [
            {
                "case_id": "large-rust",
                "run": "1",
                "success": "false",
                "reason": "rc:1",
                "terminal_state": "verifier_command_failed",
                "failure_category": "verifier",
                "contract_layer": "verification_contract",
                "diagnostic_code": "rust_compile_error",
                "active_job": "source_implementation_repair",
                "recovery_owner": "source",
                "repair_action": "edit_source_for_diagnostic",
                "target_path": "src/main.rs",
                "evidence_binding_status": "bound",
                "completion_evidence_status": "failed",
                "attempt_outcome": "failed",
                "large_disposition": "accepted_external_limitation",
                "large_disposition_reason": "model_quality_limit",
                "large_disposition_owner_action_status": "consistent",
                "large_disposition_evidence": "owner=source;target=src/main.rs",
            }
        ]
        spec = eval_signoff.RootSpec("large", pathlib.Path("root"))

        findings = eval_signoff.classify(spec, rows)

        self.assertIn("invalid_external_limitation", [item.code for item in findings])

    def test_rejects_large_owner_action_mismatch(self):
        rows = [
            {
                "case_id": "large-nextjs",
                "run": "1",
                "success": "false",
                "reason": "rc:1",
                "terminal_state": "verifier_command_failed",
                "failure_category": "verifier",
                "contract_layer": "verification_contract",
                "diagnostic_code": "edit_target_not_found",
                "active_job": "tool_protocol_correction",
                "recovery_owner": "source",
                "repair_action": "correct_tool_protocol",
                "target_path": "app/page.tsx",
                "evidence_binding_status": "bound",
                "completion_evidence_status": "failed",
                "attempt_outcome": "failed",
                "large_disposition": "closed_owned_failure",
                "large_disposition_reason": "owned_tool_protocol_failure",
                "large_disposition_owner_action_status": (
                    "inconsistent_source_tool_protocol_action"
                ),
                "large_disposition_evidence": "owner=source;target=app/page.tsx",
            }
        ]
        spec = eval_signoff.RootSpec("large", pathlib.Path("root"))

        findings = eval_signoff.classify(spec, rows)

        self.assertIn("inconsistent_large_owner_action", [item.code for item in findings])

    def test_requires_recheck_summary_when_requested(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            write_summary(root, "summary.tsv", [{"case_id": "smoke", "run": "1"}])
            spec = eval_signoff.RootSpec("smoke", root)

            findings, selected = eval_signoff.root_summary_path(
                spec,
                require_recheck=True,
                summary_name=None,
            )

        self.assertIsNone(selected)
        self.assertEqual([item.code for item in findings], ["missing_recheck_summary"])

    def test_require_recheck_uses_recheck_as_authoritative_focused_row(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            write_summary(
                root,
                "summary.tsv",
                [
                    {
                        "case_id": "focused",
                        "run": "1",
                        "success": "true",
                        "expected_assertion_status": "failed",
                        "expected_assertion_failures": "terminal_state mismatch",
                    }
                ],
            )
            write_summary(
                root,
                "recheck_summary.tsv",
                [
                    {
                        "case_id": "focused",
                        "run": "1",
                        "success": "true",
                        "expected_assertion_status": "passed_recheck",
                    }
                ],
            )
            spec = eval_signoff.RootSpec("focused", root)

            findings = eval_signoff.focused_summary_findings(
                spec,
                eval_signoff.read_rows(root / "summary.tsv"),
                authoritative_recheck_rows=eval_signoff.read_rows(
                    root / "recheck_summary.tsv"
                ),
            )

        self.assertEqual(findings, [])

    def test_require_recheck_keeps_unrechecked_original_focused_assertions(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            write_summary(
                root,
                "summary.tsv",
                [
                    {
                        "case_id": "focused",
                        "run": "1",
                        "success": "true",
                        "expected_assertion_status": "failed",
                        "expected_assertion_failures": "terminal_state mismatch",
                    }
                ],
            )
            write_summary(
                root,
                "recheck_summary.tsv",
                [
                    {
                        "case_id": "other-focused",
                        "run": "1",
                        "success": "true",
                        "expected_assertion_status": "passed_recheck",
                    }
                ],
            )
            spec = eval_signoff.RootSpec("focused", root)

            findings = eval_signoff.focused_summary_findings(
                spec,
                eval_signoff.read_rows(root / "summary.tsv"),
                authoritative_recheck_rows=eval_signoff.read_rows(
                    root / "recheck_summary.tsv"
                ),
            )

        self.assertEqual([item.code for item in findings], ["focused_assertion_failed"])


if __name__ == "__main__":
    unittest.main()
