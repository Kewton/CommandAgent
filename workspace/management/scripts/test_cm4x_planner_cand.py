from __future__ import annotations

import unittest

import cm4x_planner_cand


class Cm4xPlannerCandidateTests(unittest.TestCase):
    def test_terminal_signatures_are_closed_and_specific(self) -> None:
        cases = {
            "path does not exist: app.spec.yaml": "community_spec_artifact_missing",
            "stdin is not a TTY; pass --prompt": "community_verify_instruction_not_executable",
            "dangerous command blocked": "community_dangerous_command_blocked",
            "community_package_missing": "community_package_missing",
            "community_computed_unregistered:luggageItem": "community_computed_unregistered",
            "path does not exist: .bench-product-stdout.md": "community_workspace_path_invented",
            "path does not exist: core.yaml": "community_workspace_path_invented",
        }
        for reason, expected in cases.items():
            with self.subTest(reason=reason):
                self.assertEqual(
                    cm4x_planner_cand.classify_terminal(reason, 1), expected
                )
        self.assertIsNone(cm4x_planner_cand.classify_terminal("completed", 0))


if __name__ == "__main__":
    unittest.main()
