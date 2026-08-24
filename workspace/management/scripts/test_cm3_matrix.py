#!/usr/bin/env python3
from __future__ import annotations

import unittest

import cm3_matrix


class Cm3MatrixTests(unittest.TestCase):
    def test_wilson_clamps_zero_success_roundoff(self) -> None:
        lower, upper = cm3_matrix.wilson(0, 12)
        self.assertEqual(lower, 0.0)
        self.assertAlmostEqual(upper, 0.24249400665524076)

    def test_percentile_matches_declared_linear_interpolation(self) -> None:
        self.assertEqual(cm3_matrix.percentile([1, 2, 3, 4], 0.5), 2.5)
        self.assertAlmostEqual(cm3_matrix.percentile([1, 2, 3, 4], 0.95), 3.85)

    def test_stop_class_prefers_product_vocabulary(self) -> None:
        record = {"product_exit": 1, "terminal_reason": "phase failed"}
        events = [
            {
                "event": "run_stop",
                "ok": False,
                "stop_reason": "community_profile_violation:community_package_missing",
            }
        ]
        self.assertEqual(
            cm3_matrix.classify_stop(record, events), "community_package_missing"
        )


if __name__ == "__main__":
    unittest.main()
