import unittest

import bench
from id_vocabulary import INTERRUPTED_ENVIRONMENT, PYTHON_PRODUCED_IDS


class IdVocabularyTests(unittest.TestCase):
    def test_cross_language_stop_id_is_declared_once_and_used_by_bench(self):
        expected = "interrupted" + "(environment)"

        self.assertEqual(PYTHON_PRODUCED_IDS, (expected,))
        self.assertEqual(INTERRUPTED_ENVIRONMENT, expected)
        self.assertIn(INTERRUPTED_ENVIRONMENT, bench.TERMINAL_RUN_STATUSES)
