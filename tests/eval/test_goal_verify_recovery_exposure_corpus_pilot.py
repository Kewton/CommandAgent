from __future__ import annotations

import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_recovery_exposure_corpus_pilot import (
    CASE_SPECS,
    _extract_target_text,
    build_report,
)


def ready_case(case_id: str, profile: str) -> dict:
    return {
        "case_id": case_id,
        "profile": profile,
        "corpus_ready": True,
        "checks": {
            "before_reproducer_failed_with_registered_exit": True,
            "reference_reproducer_passed_with_registered_exit": True,
            "frozen_regressions_pass_before_and_reference": True,
            "immutable_inputs_unchanged": True,
            "route_render_before_fail_reference_pass": True,
        },
    }


class RecoveryExposureCorpusPilotTest(unittest.TestCase):
    def test_extract_target_text_handles_nested_route_markup(self):
        html = '<main><p id="result-08"><span>ready</span>-08</p></main>'
        self.assertEqual(_extract_target_text(html, "result-08"), "ready-08")
        self.assertIsNone(_extract_target_text(html, "result-09"))

    def test_report_is_go_only_when_all_profiles_have_distinct_polarity(self):
        cases = [ready_case(spec["case_id"], spec["profile"]) for spec in CASE_SPECS]
        report = build_report(
            cases=cases,
            task_registry_sha256="a" * 64,
            workspace_registry_sha256="b" * 64,
            provisioning_sha256="c" * 64,
        )

        self.assertEqual(report["go_no_go"], "GO")
        self.assertTrue(report["corpus_ready_for_preregistration"])
        self.assertFalse(report["effect_claim_allowed"])
        self.assertFalse(report["full_effect_execution_authorized"])
        self.assertEqual(
            report["profile_case_counts"], {"generic": 3, "data": 3, "nextjs": 3}
        )
        self.assertFalse(report["selection_policy"]["runtime_case_exclusion_allowed"])

        cases[1]["checks"]["before_reproducer_failed_with_registered_exit"] = False
        cases[1]["corpus_ready"] = False
        rejected = build_report(
            cases=cases,
            task_registry_sha256="a" * 64,
            workspace_registry_sha256="b" * 64,
            provisioning_sha256="c" * 64,
        )
        self.assertEqual(rejected["go_no_go"], "NO-GO")
        self.assertFalse(rejected["corpus_ready_for_preregistration"])

    def test_report_rejects_post_hoc_case_exclusion(self):
        cases = [ready_case(spec["case_id"], spec["profile"]) for spec in CASE_SPECS]

        report = build_report(
            cases=cases[:-1],
            task_registry_sha256="a" * 64,
            workspace_registry_sha256="b" * 64,
            provisioning_sha256="c" * 64,
        )

        self.assertEqual(report["go_no_go"], "NO-GO")
        self.assertFalse(report["checks"]["all_preselected_case_ids_present_once"])
        self.assertFalse(report["checks"]["exactly_three_cases_per_target_profile"])


if __name__ == "__main__":
    unittest.main()
