import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_recovery_confirmatory_design import validate_design


def _design():
    return {
        "profiles": ["generic", "data", "nextjs"],
        "pairs_per_profile": 30,
        "total_pairs": 90,
        "arm_order": ["control", "treatment"],
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
    assert "pairs_per_profile_must_be_30" in result["errors"]
    assert "discarded_valid_treatment_zero_required" in result["errors"]
