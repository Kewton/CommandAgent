import tempfile
import unittest
from pathlib import Path

import sys

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO / "scripts"))

from eval_case_schema import read_eval_case  # noqa: E402
from eval_trace import (  # noqa: E402
    artifact_changes,
    causal_summary,
    file_snapshot,
    plan_quality_from_workspace,
)


class EvalTraceTests(unittest.TestCase):
    def test_workspace_delta_records_created_and_modified_files(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "README.md").write_text("before\n", encoding="utf-8")
            before = file_snapshot(root)

            (root / "README.md").write_text("after\n", encoding="utf-8")
            (root / "notes.txt").write_text("new\n", encoding="utf-8")
            after = file_snapshot(root)

        changes = artifact_changes(before, after)
        by_path = {change["path"]: change["status"] for change in changes}

        self.assertEqual(by_path["README.md"], "modified")
        self.assertEqual(by_path["notes.txt"], "created")

    def test_causal_summary_classifies_blocked_verifier_bash(self):
        events = [
            {
                "event_id": "evt_1",
                "event_type": "tool_call.finished",
                "step_id": "repair-rust-main",
                "payload": {
                    "error": "tool_policy_violation: Bash command is not read-only for this step: command=cargo test 2>&1"
                },
            }
        ]

        summary = causal_summary(events, [], False, "rc:1")

        self.assertEqual(
            summary["first_actionable_divergence"],
            "verifier_requested_before_mutation",
        )
        self.assertEqual(summary["first_divergence_event_id"], "evt_1")
        self.assertEqual(summary["first_divergence_step_id"], "repair-rust-main")

    def test_minimal_mode_defaults_component(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "case.yaml"
            path.write_text(
                "\n".join(
                    [
                        "id: direct",
                        "profile: docs",
                        "style: default",
                        "mode: minimal",
                        'prompt: "Create README.md"',
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            case = read_eval_case(path)

        self.assertEqual(case["mode"], "minimal")
        self.assertEqual(case["component"], "minimal_loop")

    def test_plan_only_defaults_planner_component(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "case.yaml"
            path.write_text(
                "\n".join(
                    [
                        "id: planner",
                        "profile: docs",
                        "style: default",
                        "mode: plan-only",
                        'prompt: "Plan README.md"',
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            case = read_eval_case(path)

        self.assertEqual(case["mode"], "plan-only")
        self.assertEqual(case["component"], "planner")

    def test_plan_quality_scores_owned_expected_paths(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            plan_dir = root / ".commandagent" / "plans"
            plan_dir.mkdir(parents=True)
            (plan_dir / "plan.yaml").write_text(
                "\n".join(
                    [
                        'goal: "Create docs"',
                        'profile: "docs"',
                        'style: "default"',
                        "required_artifacts:",
                        "  - README.md",
                        "steps:",
                        "  - id: create-readme",
                        "    kind: create",
                        '    instruction: "Create README.md with usage notes."',
                        "    expected_paths:",
                        "      - README.md",
                        "    verify:",
                        "      - test -f README.md",
                        "  - id: verify-readme",
                        "    kind: verify",
                        '    instruction: "Verify README.md exists."',
                        "    expected_paths: []",
                        "    verify:",
                        "      - test -f README.md",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            quality = plan_quality_from_workspace(root, "plan-run", ["README.md"])

        self.assertEqual(quality["plan_quality_status"], "pass")
        self.assertEqual(quality["plan_quality_owned_required_artifact_count"], "1")
        self.assertEqual(quality["plan_quality_missing_owner_count"], "0")
        self.assertEqual(quality["plan_quality_responsibility_score"], "100")

    def test_plan_quality_flags_missing_ultra_phase_owner(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            plan_dir = root / ".commandagent" / "plans"
            plan_dir.mkdir(parents=True)
            (plan_dir / "ultra.yaml").write_text(
                "\n".join(
                    [
                        'goal: "Create Rust CLI"',
                        'profile: "rust"',
                        'style: "default"',
                        'intent: "new"',
                        "required_artifacts:",
                        "  - Cargo.toml",
                        "  - src/main.rs",
                        "phases:",
                        "  - id: scaffold",
                        '    goal: "Create manifest."',
                        "    owned_artifacts:",
                        "      - Cargo.toml",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            quality = plan_quality_from_workspace(
                root, "ultra-plan-run", ["Cargo.toml", "src/main.rs"]
            )

        self.assertEqual(quality["plan_quality_status"], "fail")
        self.assertEqual(quality["plan_quality_owned_required_artifact_count"], "1")
        self.assertEqual(quality["plan_quality_missing_owner_count"], "1")
        self.assertIn("src/main.rs", quality["plan_quality_notes"])


if __name__ == "__main__":
    unittest.main()
