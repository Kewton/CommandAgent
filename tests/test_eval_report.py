import importlib.util
import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "eval_report", ROOT / "scripts" / "eval_report.py"
)
eval_report = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(eval_report)


class EvalReportCategorizeTests(unittest.TestCase):
    def test_layer_categories(self):
        cases = {
            "plan_lint.profile_obligations:missing literal": "planning",
            "Gemini JSON parse failed: tool call is missing a tool name": "provider_transport",
            "tool_args_missing_required_field": "tool_protocol",
            "read_only_step_mutation": "step_policy",
            "profile_verification:nextjs_route_not_integrated": "profile",
            "dependency_missing": "setup",
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


if __name__ == "__main__":
    unittest.main()
