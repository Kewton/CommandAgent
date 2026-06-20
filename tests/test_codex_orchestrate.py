import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = REPO_ROOT / "scripts" / "codex_orchestrate.py"
FIXTURE_PATH = REPO_ROOT / "tests" / "fixtures" / "codex_orchestrate" / "issues.json"


def load_module():
    spec = importlib.util.spec_from_file_location("codex_orchestrate", SCRIPT_PATH)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class CodexOrchestrateTests(unittest.TestCase):
    def setUp(self):
        self.mod = load_module()

    def test_slugify_normalizes_titles(self):
        self.assertEqual(self.mod.slugify("Add Plan Schema Contract"), "add-plan-schema-contract")
        self.assertEqual(self.mod.slugify("!!!"), "task")

    def test_fixture_loading_preserves_requested_order(self):
        issues = self.mod.load_issues([103, 101], FIXTURE_PATH, "Kewton/CommandAgent")
        self.assertEqual([issue.number for issue in issues], [103, 101])
        self.assertEqual(issues[0].labels, ("bug",))

    def test_analysis_uses_commandagent_worktree_names(self):
        issue = self.mod.load_issues([101], FIXTURE_PATH, "Kewton/CommandAgent")[0]
        analysis = self.mod.analyze_issue(issue, "CommandAgent", skip_enhance=False)
        self.assertEqual(analysis.branch_name, "feature/issue-101-add-plan-schema-contract")
        self.assertEqual(
            analysis.worktree_path,
            "../CommandAgent-issue-101-add-plan-schema-contract",
        )
        self.assertIn("src/agent/step_runner/plan.rs", analysis.suspected_files)
        self.assertIn("cargo test", analysis.test_expectations)

    def test_storage_issue_depends_on_contract_issue(self):
        issues = self.mod.load_issues([101, 102, 103], FIXTURE_PATH, "Kewton/CommandAgent")
        analyses = [
            self.mod.analyze_issue(issue, "CommandAgent", skip_enhance=False)
            for issue in issues
        ]
        by_number = {analysis.issue.number: analysis for analysis in analyses}
        deps = self.mod.direct_dependencies(by_number[102], analyses)
        self.assertEqual([dep.issue.number for dep in deps], [101])
        self.assertEqual(self.mod.classify_issue(by_number[102], analyses), "strong-dependency")

    def test_dry_run_writes_artifacts_without_network(self):
        with tempfile.TemporaryDirectory() as tmp:
            runs_dir = Path(tmp) / "runs"
            result = self.mod.main(
                [
                    "101",
                    "102",
                    "--dry-run",
                    "--issue-json",
                    str(FIXTURE_PATH),
                    "--run-id",
                    "smoke",
                    "--runs-dir",
                    str(runs_dir),
                ]
            )
            self.assertEqual(result, 0)
            run_dir = runs_dir / "smoke"
            manifest = (run_dir / "manifest.md").read_text(encoding="utf-8")
            dependency = (run_dir / "dependency-plan.md").read_text(encoding="utf-8")
            analysis = (run_dir / "issue-analysis.md").read_text(encoding="utf-8")

        self.assertIn("Repository: `Kewton/CommandAgent`", manifest)
        self.assertIn("Base: `origin/develop`", manifest)
        self.assertIn("CommandAgent-issue-101-add-plan-schema-contract", manifest)
        self.assertIn("#102: strong-dependency", dependency)
        self.assertIn("src/session/store.rs", analysis)

    def test_mutating_flag_without_dry_run_is_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            result = self.mod.main(
                [
                    "101",
                    "--issue-json",
                    str(FIXTURE_PATH),
                    "--runs-dir",
                    tmp,
                    "--create-worktrees",
                ]
            )
        self.assertEqual(result, 2)


if __name__ == "__main__":
    unittest.main()
