import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_recovery_confirmatory_design import (
    analyze_pair_results,
    materialize_pair_ids,
    paired_effect_ci,
    validate_design,
)


def _design():
    return {
        "profiles": ["generic", "data", "nextjs"],
        "pairs_per_profile": 10,
        "total_pairs": 30,
        "task_ids": [f"{index:02d}" for index in range(1, 11)],
        "allocation_seed": 39320260902,
        "arm_order": ["control", "treatment"],
        "concurrency": 1,
        "recovery_auto_runs": {"control": 0, "treatment": 1},
        "fresh_workspace_per_arm": True,
        "same_input_snapshot": True,
        "same_failure_boundary": True,
        "optional_stopping": False,
        "replacement_or_rerun": False,
        "bootstrap_samples": 2000,
        "confidence_level": 0.95,
        "effect_threshold": 0.20,
        "regression_zero": True,
        "artifact_harm_zero": True,
        "discarded_valid_treatment_zero": True,
        "a29_pairs_pooled": False,
    }


def test_valid_design():
    assert validate_design(_design()) == {"valid": True, "errors": []}


def test_registered_fixture_is_valid():
    fixture = ROOT / "tests/fixtures/goal_verify_recovery_confirmatory/design-v1.json"
    assert validate_design(json.loads(fixture.read_text(encoding="utf-8")))["valid"]


def test_rejects_sample_and_safety_changes():
    design = _design()
    design["pairs_per_profile"] = 3
    design["discarded_valid_treatment_zero"] = False
    result = validate_design(design)
    assert result["valid"] is False
    assert "pairs_per_profile_must_be_10" in result["errors"]
    assert "discarded_valid_treatment_zero_required" in result["errors"]


def test_materializes_frozen_pair_order():
    pair_ids = materialize_pair_ids(39320260902)
    assert len(pair_ids) == 30
    assert len(set(pair_ids)) == 30
    assert {pair_id.split("--", 1)[0] for pair_id in pair_ids} == {
        "generic",
        "data",
        "nextjs",
    }
    assert pair_ids == materialize_pair_ids(39320260902)


def test_paired_effect_ci_is_reproducible():
    result = paired_effect_ci([0, 0, 1, 0], [1, 1, 1, 0], seed=7, samples=100)
    assert result["estimate"] == 0.5
    assert result["samples"] == 100
    assert result == paired_effect_ci([0, 0, 1, 0], [1, 1, 1, 0], seed=7, samples=100)


def _perfect_rows(design):
    return [
        {
            "pair_id": pair_id,
            "profile": pair_id.split("--", 1)[0],
            "control_endpoint": False,
            "treatment_endpoint": True,
            "pair_valid": True,
            "regression_count": 0,
            "artifact_harm_count": 0,
            "discarded_valid_treatment_count": 0,
            "stale_failure_kind_count": 0,
        }
        for pair_id in materialize_pair_ids(
            design["allocation_seed"], design["pairs_per_profile"]
        )
    ]


def test_analysis_go_requires_complete_safe_denominator():
    design = _design()
    report = analyze_pair_results(design, _perfect_rows(design))
    assert report["status"] == "GO"
    assert report["pair_count"] == 30
    assert report["effects"]["pooled"]["estimate"] == 1.0
    assert all(report["gates"].values())


def test_analysis_rejects_missing_pair():
    design = _design()
    report = analyze_pair_results(design, _perfect_rows(design)[:-1])
    assert report["status"] == "INVALID"
    assert report["errors"] == ["pair_denominator_invalid"]


def test_analysis_safety_violation_is_no_go():
    design = _design()
    rows = _perfect_rows(design)
    rows[0]["discarded_valid_treatment_count"] = 1
    report = analyze_pair_results(design, rows)
    assert report["status"] == "NO-GO"
    assert report["gates"]["discarded_valid_treatment_zero"] is False
