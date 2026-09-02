from __future__ import annotations

import hashlib
from pathlib import Path
from typing import Any

from eval_lib.goal_verify_recovery_a15_report import (
    build_recovery_a15_smoke_report,
)


def build_recovery_a23_pilot_report(
    *,
    records: list[dict[str, Any]],
    contract: dict[str, Any],
    oracle_executability_preflight: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Evaluate pilot validity separately from the next-design exposure gate."""
    base = build_recovery_a15_smoke_report(
        records=records,
        contract=contract,
        oracle_executability_preflight=oracle_executability_preflight,
    )
    design = contract["pilot_design"]
    threshold = design["natural_exposure_confirmation_threshold"]
    required_profiles = contract["smoke"]["required_real_profiles"]
    profile_readiness = base.get("profile_readiness", {})
    minimum_clusters = int(threshold["minimum_executed_recovery_clusters_per_profile"])
    profile_exposure = {
        profile: {
            "executed_recovery_clusters": int(
                profile_readiness.get(profile, {}).get("executed_recovery_clusters", 0)
            ),
            "minimum_required": minimum_clusters,
            "threshold_met": (
                int(
                    profile_readiness.get(profile, {}).get(
                        "executed_recovery_clusters", 0
                    )
                )
                >= minimum_clusters
            ),
        }
        for profile in required_profiles
    }
    profiles_meeting = sum(
        row["threshold_met"] is True for row in profile_exposure.values()
    )
    diagnostics = base.get("diagnostics", {})
    instrumentation_unusable = sorted(
        {
            str(pair_id)
            for pair_id in diagnostics.get("instrumentation_unusable_pair_ids", [])
        }
    )
    base_checks = base.get("checks", {})
    safety_check_names = threshold["safety_check_names"]
    failed_safety_checks = [
        name for name in safety_check_names if base_checks.get(name) is not True
    ]
    pilot_instrument_ready = base.get("instrument_ready") is True
    threshold_checks = {
        "pilot_instrument_ready": pilot_instrument_ready,
        "all_required_profiles_reported": set(profile_readiness)
        == set(required_profiles),
        "minimum_profiles_meeting_exposure_threshold": (
            profiles_meeting >= int(threshold["minimum_profiles_meeting_threshold"])
        ),
        "instrumentation_unusable_pairs_within_limit": (
            len(instrumentation_unusable)
            <= int(threshold["maximum_instrumentation_unusable_pairs"])
        ),
        "safety_violations_within_limit": (
            len(failed_safety_checks) <= int(threshold["maximum_safety_violations"])
        ),
    }
    natural_exposure_ready = all(threshold_checks.values())
    if not pilot_instrument_ready:
        decision = "pilot_invalid_requires_diagnosis"
        threshold_status = "INVALID"
    elif natural_exposure_ready:
        decision = "preregister_natural_exposure_confirmatory_experiment"
        threshold_status = "MET"
    else:
        decision = "preregister_deterministic_fault_boundary_experiment"
        threshold_status = "NOT_MET"
    return {
        **base,
        "schema_version": (
            "commandagent.goal_verify.recovery_natural_exposure_pilot_report.v1"
        ),
        "inference_role": design["inference_role"],
        "effect_claim_allowed": False,
        "effect_claim_ready": False,
        "pilot_instrument_ready": pilot_instrument_ready,
        "pilot_go_no_go": "GO" if pilot_instrument_ready else "NO-GO",
        "natural_exposure_confirmation_ready": natural_exposure_ready,
        "natural_exposure_threshold_status": threshold_status,
        "next_design_decision": decision,
        "natural_exposure_threshold_checks": threshold_checks,
        "profile_exposure": profile_exposure,
        "profiles_meeting_exposure_threshold": profiles_meeting,
        "instrumentation_unusable_pair_ids": instrumentation_unusable,
        "failed_safety_check_names": failed_safety_checks,
    }


def authoritative_report_source_errors(
    *, root: Path, contract: dict[str, Any]
) -> list[str]:
    expected = contract.get("pilot_design", {}).get(
        "authoritative_report_source_sha256", {}
    )
    if not isinstance(expected, dict) or not expected:
        return ["authoritative_report_source_sha256_missing"]
    errors = []
    for relative, expected_sha in expected.items():
        path = root / relative
        if not path.is_file():
            errors.append(f"authoritative_report_source_missing:{relative}")
            continue
        actual = hashlib.sha256(path.read_bytes()).hexdigest()
        if actual != expected_sha:
            errors.append(f"authoritative_report_source_mismatch:{relative}")
    return errors
