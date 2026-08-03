from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

import bon_select

REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
SUITE = (
    REPOSITORY_ROOT
    / "workspace"
    / "management"
    / "bench"
    / "suites"
    / "cli-filter-bon0.toml"
)
BASELINE = (
    REPOSITORY_ROOT
    / "workspace"
    / "management"
    / "bench"
    / "suites"
    / "cli-create-luna.toml"
)


def write_campaign(
    root: Path,
    vectors: list[tuple[str, str, str, str]],
    full_indices: set[int],
    *,
    returned_model: str = "gpt-5.6-luna",
) -> Path:
    campaign = root / "cli-filter-bon0-fixture"
    campaign.mkdir()
    metadata_runs = []
    for index, states in enumerate(vectors, start=1):
        name = f"filter_bon0_{index:03d}"
        metadata_runs.append(
            {
                "name": name,
                "goal": "filter",
                "executor": "gpt-5.6-luna",
                "status": "completed",
                "duration_seconds": 70 - index,
                "input_sha256_expected": {},
                "input_sha256_observed": {},
            }
        )
        artifact = campaign / "artifacts" / name
        events_dir = artifact / ".anvil" / "runs" / f"run-{index}"
        evidence_dir = artifact / "evidence"
        events_dir.mkdir(parents=True)
        evidence_dir.mkdir(parents=True)
        events = [
            {
                "event": "provider_turn_duration",
                "caller_scope": "executor",
                "provider": "openai",
                "model": "gpt-5.6-luna",
                "provider_model_id": returned_model,
                "provider_service_tier": "default",
                "system_fingerprint": None,
                "prompt_eval_count": 1000 + index,
                "provider_cached_input_tokens": 800,
                "eval_count": 100,
                "provider_reasoning_tokens": 10,
                "provider_response_id": f"resp-{index}-1",
            },
            {
                "event": "provider_response",
                "provider": "openai",
                "model": "gpt-5.6-luna",
                "tool_calls": 1,
            },
            {
                "event": "run_stop",
                "final_acceptance_status": (
                    "full_success" if index in full_indices else "failed"
                ),
                "assurance_level": "full" if index in full_indices else "failed",
                "ok": index in full_indices,
            },
        ]
        (events_dir / "events.jsonl").write_text(
            "".join(json.dumps(event) + "\n" for event in events), encoding="utf-8"
        )
        checks = dict(zip(bon_select.REGISTERED_ATOMS, states, strict=True))
        (evidence_dir / "cli-assurance.json").write_text(
            json.dumps({"evidence": {"checks": checks}}), encoding="utf-8"
        )
        (artifact / "acceptance-sheet.md").write_text("fixture\n", encoding="utf-8")
    metadata = {
        "schema_version": "1",
        "campaign_id": campaign.name,
        "suite": {
            "id": "cli-filter-bon0",
            "sha256": bon_select.sha256_file(SUITE),
            "bon_series": "f-bon-v-cli-luna",
            "profile": "cli",
            "intent": "create",
            "planner_model": "qwen3.6:27b-coding-nvfp4",
            "provider": "openai",
        },
        "preflight": {
            "binary_sha256": {"built": "a" * 64, "installed": "a" * 64},
            "bon_series_pin": {
                "schema_version": bon_select.BON_PREDECLARATION_SCHEMA_VERSION,
                "series_id": "f-bon-v-cli-luna",
                "execution_revision_expected": "b" * 40,
                "execution_revision_observed": "b" * 40,
                "suite_sha256_expected": bon_select.sha256_file(SUITE),
                "binary_sha256_expected": "a" * 64,
                "binary_sha256_observed": "a" * 64,
                "binary_sha256_matches": True,
                "baseline_rate": {
                    "full_count": 4,
                    "trial_count": 42,
                },
                "predictive_distribution": {"model": "beta_binomial"},
            },
        },
        "runs": metadata_runs,
    }
    (campaign / "uat-meta.json").write_text(json.dumps(metadata), encoding="utf-8")
    return campaign


class BonSelectionTests(unittest.TestCase):
    def test_suite_is_one_pinned_filter_goal_repeated_six_times(self) -> None:
        document, baseline, reasons = bon_select.validate_suite(SUITE, BASELINE)

        self.assertEqual(reasons, [])
        self.assertEqual(set(document["goals"]), {"filter"})
        self.assertEqual(len(document["runs"]), 6)
        self.assertEqual(document["goals"]["filter"], baseline["goals"]["filter"])
        self.assertEqual(
            bon_select.sha256_file(BASELINE),
            "e4c04d2374aa2c0d45eaeb6cdc51a2ea3fd0692533fd66070e0df505b01da895",
        )
        self.assertEqual(
            bon_select.sha256_bytes(document["goals"]["filter"].encode()),
            "87977bbb84c010dc158eba0246c3f04794899f65303c3585292070d9b2ef4070",
        )

    def test_earned_full_precedes_a_higher_speed_non_full(self) -> None:
        vectors = [
            ("failed", "pass", "pass", "pass"),
            ("pass", "pass", "pass", "pass"),
            *([("failed", "pass", "claims_absent", "pass")] * 4),
        ]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            campaign = write_campaign(root, vectors, {2})
            result = bon_select.build_selection(
                campaign, SUITE, BASELINE, root / "calibration"
            )

        self.assertTrue(result["valid_measurement"])
        self.assertEqual(result["selection"]["kind"], "adopted_full")
        self.assertEqual(result["selection"]["run"], "filter_bon0_002")
        self.assertFalse(result["selection"]["repair_connected"])
        self.assertEqual(result["summary"]["earned_full"], 1)
        self.assertEqual(result["summary"]["full_count"], 1)
        self.assertTrue(result["identity"]["sampling"]["trial_specific"])
        self.assertEqual(result["retention"]["nonselected_evidence_preserved"], 5)

    def test_multiple_fulls_use_duration_then_cost_then_name(self) -> None:
        vectors = [("pass", "pass", "pass", "pass")] * 6
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            campaign = write_campaign(root, vectors, {1, 2})
            result = bon_select.build_selection(
                campaign, SUITE, BASELINE, root / "calibration"
            )

        self.assertEqual(result["selection"]["run"], "filter_bon0_002")

    def test_all_fail_identifies_highest_score_loser_without_repair(self) -> None:
        vectors = [
            ("failed", "pass", "pass", "pass"),
            ("pass", "pass", "claims_absent", "pass"),
            *([("failed", "pass", "claims_absent", "pass")] * 4),
        ]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            campaign = write_campaign(root, vectors, set())
            result = bon_select.build_selection(
                campaign, SUITE, BASELINE, root / "calibration"
            )

        self.assertEqual(result["selection"]["kind"], "most_promising_loser")
        self.assertEqual(result["selection"]["run"], "filter_bon0_002")
        self.assertFalse(result["definition"]["prediction"])
        self.assertFalse(result["definition"]["pruning"])
        self.assertFalse(result["definition"]["repair_connected"])

    def test_returned_model_drift_invalidates_measurement(self) -> None:
        vectors = [("pass", "pass", "pass", "pass")] * 6
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            campaign = write_campaign(
                root, vectors, {1}, returned_model="gpt-5.6-luna-drift"
            )
            result = bon_select.build_selection(
                campaign, SUITE, BASELINE, root / "calibration"
            )

        self.assertFalse(result["valid_measurement"])
        self.assertIn("model_metadata_mismatch", result["invalid_reasons"])
        self.assertEqual(result["selection"]["kind"], "invalid_measurement")
        self.assertIsNone(result["selection"]["run"])

    def test_series_binary_pin_mismatch_invalidates_measurement(self) -> None:
        vectors = [("pass", "pass", "pass", "pass")] * 6
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            campaign = write_campaign(root, vectors, {1})
            metadata_path = campaign / "uat-meta.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["preflight"]["bon_series_pin"]["binary_sha256_expected"] = (
                "c" * 64
            )
            metadata_path.write_text(json.dumps(metadata), encoding="utf-8")

            result = bon_select.build_selection(
                campaign, SUITE, BASELINE, root / "calibration"
            )

        self.assertFalse(result["valid_measurement"])
        self.assertIn("bon_series_pin_mismatch", result["invalid_reasons"])
        self.assertEqual(result["selection"]["kind"], "invalid_measurement")

    def test_cross_campaign_binomial_dispersion_uses_predeclared_ratio(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            paths = []
            for index, full_count in enumerate((0, 1, 2, 1), start=1):
                path = root / f"campaign-{index}.json"
                path.write_text(
                    json.dumps(
                        {
                            "schema_version": bon_select.SCHEMA_VERSION,
                            "campaign_id": f"campaign-{index}",
                            "valid_measurement": True,
                            "summary": {
                                "runs": 6,
                                "earned_full": full_count,
                                "full_count": full_count,
                            },
                        }
                    ),
                    encoding="utf-8",
                )
                paths.append(path)

            result = bon_select.build_independence_check(paths)

        self.assertEqual(result["observed"]["full_counts"], [0, 1, 2, 1])
        self.assertEqual(result["observed"]["full_count_total"], 4)
        self.assertAlmostEqual(
            result["cross_check"]["binomial_expected_variance"], 0.8466
        )
        self.assertAlmostEqual(result["cross_check"]["variance_ratio"], 0.78746358)
        self.assertEqual(result["cross_check"]["decision"], "binomial_consistent")
        self.assertEqual(
            result["predeclared_test"]["dispersion_ratio_thresholds"],
            {"underdispersed_below": 0.5, "overdispersed_above": 1.5},
        )

    def test_cross_campaign_binomial_dispersion_rejects_duplicate_campaign(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / "campaign.json"
            path.write_text(
                json.dumps(
                    {
                        "schema_version": bon_select.SCHEMA_VERSION,
                        "campaign_id": "same",
                        "valid_measurement": True,
                        "summary": {"runs": 6, "full_count": 1},
                    }
                ),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(
                bon_select.SelectionError, "campaign ids must be present and distinct"
            ):
                bon_select.build_independence_check([path, path])


if __name__ == "__main__":
    unittest.main()
