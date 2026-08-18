import unittest

import cm3b_matrix


class Cm3bMatrixTests(unittest.TestCase):
    def test_newcombe_equal_rates_contains_zero(self) -> None:
        lower, upper = cm3b_matrix.newcombe_difference(7, 12, 7, 12)
        self.assertLess(lower, 0.0)
        self.assertGreater(upper, 0.0)

    def test_d_prime_decision_requires_both_predeclared_lines(self) -> None:
        passing = {"full_rate": 11 / 12, "duration_secs": {"p50": 30.0}}
        self.assertTrue(cm3b_matrix.d_prime_decision(passing)["established"])
        slow = {"full_rate": 11 / 12, "duration_secs": {"p50": 30.1}}
        self.assertFalse(cm3b_matrix.d_prime_decision(slow)["established"])
        weak = {"full_rate": 10 / 12, "duration_secs": {"p50": 20.0}}
        self.assertFalse(cm3b_matrix.d_prime_decision(weak)["established"])

    def test_terminal_classification_preserves_model_signatures(self) -> None:
        self.assertEqual(
            cm3b_matrix.classify_terminal(
                "path does not exist: schema/app-spec.schema.sha256sums", 1
            ),
            "community_schema_pin_path_invented",
        )
        self.assertEqual(
            cm3b_matrix.classify_terminal("path does not exist: package.json", 1),
            "community_package_artifact_missing",
        )
        self.assertIsNone(cm3b_matrix.classify_terminal("completed", 0))


if __name__ == "__main__":
    unittest.main()
