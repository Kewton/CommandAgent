"""Validation for the preregistered conditional-effect experiment design."""
from __future__ import annotations

import random
from typing import Any

REQUIRED_PROFILES = ("generic", "data", "nextjs")


def materialize_pair_ids(seed: int, pairs_per_profile: int = 10) -> list[str]:
    """Return a deterministic randomized order over distinct registered tasks."""
    if pairs_per_profile < 1:
        raise ValueError("pairs_per_profile must be positive")
    # Seed is part of the preregistration surface even though ordering is fixed;
    # rejecting non-integer seeds prevents accidental implicit randomness.
    if not isinstance(seed, int) or isinstance(seed, bool):
        raise TypeError("seed must be an integer")
    pair_ids = [
        f"{profile}--pair-{index:02d}"
        for profile in REQUIRED_PROFILES
        for index in range(1, pairs_per_profile + 1)
    ]
    random.Random(seed).shuffle(pair_ids)
    return pair_ids


def paired_effect_ci(
    control: list[int],
    treatment: list[int],
    *,
    seed: int,
    samples: int = 2000,
    confidence: float = 0.95,
) -> dict[str, float | int | bool]:
    """Estimate treatment-control paired endpoint contrast with percentile bootstrap."""
    if len(control) != len(treatment) or not control:
        raise ValueError("paired observations must be non-empty and equal length")
    if any(value not in (0, 1) for value in control + treatment):
        raise ValueError("endpoints must be binary")
    if samples < 1 or not 0 < confidence < 1:
        raise ValueError("invalid bootstrap configuration")
    differences = [t - c for c, t in zip(control, treatment)]
    rng = random.Random(seed)
    estimates = [
        sum(differences[rng.randrange(len(differences))] for _ in differences)
        / len(differences)
        for _ in range(samples)
    ]
    estimates.sort()
    alpha = (1 - confidence) / 2
    lower = estimates[int(alpha * (samples - 1))]
    upper = estimates[int((1 - alpha) * (samples - 1))]
    estimate = sum(differences) / len(differences)
    return {"estimate": estimate, "lower": lower, "upper": upper, "samples": samples}


def design_errors(design: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if design.get("profiles") != list(REQUIRED_PROFILES):
        errors.append("profiles_invalid")
    if design.get("pairs_per_profile") != 10:
        errors.append("pairs_per_profile_must_be_10")
    if design.get("total_pairs") != 30:
        errors.append("total_pairs_must_be_30")
    if design.get("task_ids") != [f"{index:02d}" for index in range(1, 11)]:
        errors.append("task_ids_invalid")
    seed = design.get("allocation_seed")
    if not isinstance(seed, int) or isinstance(seed, bool):
        errors.append("allocation_seed_must_be_integer")
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


def analyze_pair_results(
    design: dict[str, Any], rows: list[dict[str, Any]]
) -> dict[str, Any]:
    """Apply integrity and safety gates before reporting conditional effects."""
    errors = design_errors(design)
    if errors:
        return {"status": "INVALID", "errors": errors, "effects": {}}

    expected = materialize_pair_ids(
        design["allocation_seed"], design["pairs_per_profile"]
    )
    observed = [row.get("pair_id") for row in rows]
    duplicates = sorted(
        pair_id for pair_id in set(observed) if observed.count(pair_id) > 1
    )
    missing = sorted(set(expected) - set(observed))
    unexpected = sorted(set(observed) - set(expected), key=str)
    if len(rows) != len(expected) or duplicates or missing or unexpected:
        return {
            "status": "INVALID",
            "errors": ["pair_denominator_invalid"],
            "duplicates": duplicates,
            "missing": missing,
            "unexpected": unexpected,
            "effects": {},
        }

    safety_fields = (
        "regression_count",
        "artifact_harm_count",
        "discarded_valid_treatment_count",
        "stale_failure_kind_count",
    )
    malformed = [
        row["pair_id"]
        for row in rows
        if row.get("profile") not in REQUIRED_PROFILES
        or row.get("profile") != row["pair_id"].split("--", 1)[0]
        or not isinstance(row.get("control_endpoint"), bool)
        or not isinstance(row.get("treatment_endpoint"), bool)
        or not isinstance(row.get("pair_valid"), bool)
        or any(
            not isinstance(row.get(field), int)
            or isinstance(row.get(field), bool)
            or row[field] < 0
            for field in safety_fields
        )
    ]
    if malformed:
        return {
            "status": "INVALID",
            "errors": ["pair_record_invalid"],
            "malformed": sorted(malformed),
            "effects": {},
        }

    effects: dict[str, Any] = {}
    ordered_rows = sorted(rows, key=lambda row: expected.index(row["pair_id"]))
    for profile in (*REQUIRED_PROFILES, "pooled"):
        selected = (
            ordered_rows
            if profile == "pooled"
            else [row for row in ordered_rows if row["profile"] == profile]
        )
        effects[profile] = paired_effect_ci(
            [int(row["control_endpoint"]) for row in selected],
            [int(row["treatment_endpoint"]) for row in selected],
            seed=design["allocation_seed"],
            samples=design["bootstrap_samples"],
            confidence=design["confidence_level"],
        )

    safety_totals = {
        field: sum(row[field] for row in rows) for field in safety_fields
    }
    gates = {
        "all_pairs_valid": all(row["pair_valid"] for row in rows),
        "all_effect_lower_bounds_met": all(
            effect["lower"] >= design["effect_threshold"]
            for effect in effects.values()
        ),
        "regression_zero": safety_totals["regression_count"] == 0,
        "artifact_harm_zero": safety_totals["artifact_harm_count"] == 0,
        "discarded_valid_treatment_zero": safety_totals[
            "discarded_valid_treatment_count"
        ]
        == 0,
        "stale_failure_kind_zero": safety_totals["stale_failure_kind_count"] == 0,
    }
    return {
        "status": "GO" if all(gates.values()) else "NO-GO",
        "errors": [],
        "pair_count": len(rows),
        "effects": effects,
        "safety_totals": safety_totals,
        "gates": gates,
    }
