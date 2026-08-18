#!/usr/bin/env python3
from __future__ import annotations

import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

import cm4_planner_cand


class Cm4PlannerCandidateTests(unittest.TestCase):
    def test_classification_uses_existing_community_vocabulary(self) -> None:
        self.assertEqual(
            cm4_planner_cand.classify_terminal(
                "error: community_profile_violation:community_computed_unregistered:items",
                1,
            ),
            "community_computed_unregistered",
        )
        self.assertEqual(
            cm4_planner_cand.classify_terminal("error: stdin is not a TTY", 1),
            "community_verify_instruction_not_executable",
        )

    def test_planner_empty_response_has_a_separate_signature(self) -> None:
        self.assertEqual(
            cm4_planner_cand.classify_terminal(
                "planner_empty_response: planner returned empty content", 1
            ),
            "community_planner_empty_response",
        )

    def test_success_has_no_stop_class(self) -> None:
        self.assertIsNone(cm4_planner_cand.classify_terminal("ignored", 0))

    def test_sha256_file_uses_file_bytes(self) -> None:
        with TemporaryDirectory() as directory:
            path = Path(directory) / "evidence"
            path.write_bytes(b"cm4 evidence\n")
            self.assertEqual(
                cm4_planner_cand.sha256_file(path),
                "0d73ecb151f2932510a7db279727a51d1dfe7a859b665ec10f74fccde443a176",
            )


if __name__ == "__main__":
    unittest.main()
