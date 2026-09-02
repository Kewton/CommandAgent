"""Validation for the preregistered conditional-effect experiment design."""
from __future__ import annotations

from typing import Any

REQUIRED_PROFILES = ("generic", "data", "nextjs")


def materialize_pair_ids(seed: int, pairs_per_profile: int = 30) -> list[str]:
    """Return the frozen pair ordering for a confirmatory campaign."""
    if pairs_per_profile < 1:
        raise ValueError("pairs_per_profile must be positive")
    # Seed is part of the preregistration surface even though ordering is fixed;
    # rejecting non-integer seeds prevents accidental implicit randomness.
    if not isinstance(seed, int) or isinstance(seed, bool):
        raise TypeError("seed must be an integer")
    return [
        f"{profile}--pair-{index:02d}"
        for profile in REQUIRED_PROFILES
        for index in range(1, pairs_per_profile + 1)
    ]


def design_errors(design: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if design.get("profiles") != list(REQUIRED_PROFILES):
        errors.append("profiles_invalid")
    if design.get("pairs_per_profile") != 30:
        errors.append("pairs_per_profile_must_be_30")
    if design.get("total_pairs") != 90:
        errors.append("total_pairs_must_be_90")
    if design.get("arm_order") != ["control", "treatment"]:
        errors.append("arm_order_invalid")
    if design.get("recovery_auto_runs") != {"control": 0, "treatment": 1}:
        errors.append("arm_assignment_invalid")
    for field in ("fresh_workspace_per_arm", "same_input_snapshot", "same_failure_boundary"):
        if design.get(field) is not True:
            errors.append(f"{field}_required")
    if design.get("optional_stopping") is not False:
        errors.append("optional_stopping_must_be_false")
    if design.get("replacement_or_rerun") is not False:
        errors.append("replacement_or_rerun_must_be_false")
    if design.get("bootstrap_samples") != 2000:
        errors.append("bootstrap_samples_must_be_2000")
    if design.get("confidence_level") != 0.95:
        errors.append("confidence_level_must_be_0.95")
    if design.get("effect_threshold") != 0.20:
        errors.append("effect_threshold_must_be_0.20")
    for field in ("regression_zero", "artifact_harm_zero", "discarded_valid_treatment_zero"):
        if design.get(field) is not True:
            errors.append(f"{field}_required")
    if design.get("a29_pairs_pooled") is not False:
        errors.append("a29_pairs_must_not_be_pooled")
    return errors


def validate_design(design: dict[str, Any]) -> dict[str, Any]:
    errors = design_errors(design)
    return {"valid": not errors, "errors": errors}
