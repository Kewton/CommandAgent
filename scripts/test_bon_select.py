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
            "profile": "cli",
            "intent": "create",
            "planner_model": "qwen3.6:27b-coding-nvfp4",
            "provider": "openai",
        },
        "preflight": {"binary_sha256": {"built": "a" * 64, "installed": "a" * 64}},
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


if __name__ == "__main__":
    unittest.main()
