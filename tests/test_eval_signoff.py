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


class EvalSignoffTests(unittest.TestCase):
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
            }
        ]
        spec = eval_signoff.RootSpec("large", pathlib.Path("root"))

        findings = eval_signoff.classify(spec, rows)

        self.assertIn("missing_evidence_binding", [item.code for item in findings])
        self.assertIn("missing_completion_evidence", [item.code for item in findings])

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

    def test_require_recheck_still_checks_original_focused_assertions(self):
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
            )

        self.assertEqual([item.code for item in findings], ["focused_assertion_failed"])


if __name__ == "__main__":
    unittest.main()
