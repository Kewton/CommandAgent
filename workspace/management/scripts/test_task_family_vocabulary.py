import unittest

import band_aggregate as band
from task_family_vocabulary import (
    DATA_CREATE_FAMILIES,
    DATA_INVESTIGATE_FAMILIES,
    NEXTJS_CREATE_FAMILIES,
    NEXTJS_FIX_FAMILIES,
    TASK_FAMILY_VOCABULARY,
)


class TaskFamilyVocabularyTests(unittest.TestCase):
    def test_formal_classifiers_emit_only_declared_families(self) -> None:
        observed = {
            band.normalize_scenario("Quiz"),
            band.normalize_scenario("Breakout"),
            band.normalize_scenario("Space"),
            band.classify_data_family("月次×地域の集計"),
            band.classify_data_family("3ヶ月移動平均"),
            band.classify_fix_family("compile", ""),
            band.classify_fix_family("hook", ""),
            band.classify_investigation_family("run_pipe_001"),
            band.classify_investigation_family("run_schema_001"),
        }
        self.assertLessEqual(observed, set(TASK_FAMILY_VOCABULARY))

    def test_band_family_sets_are_the_declared_vocabulary_slices(self) -> None:
        self.assertEqual(band.DATA_FAMILIES, DATA_CREATE_FAMILIES)
        self.assertEqual(band.FIX_FAMILIES, NEXTJS_FIX_FAMILIES)
        self.assertEqual(band.INVESTIGATION_FAMILIES, DATA_INVESTIGATE_FAMILIES)
        self.assertEqual(NEXTJS_CREATE_FAMILIES, ("Quiz", "Breakout", "Space"))
