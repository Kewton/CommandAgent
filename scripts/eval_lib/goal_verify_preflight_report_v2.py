from __future__ import annotations

from collections import Counter
from typing import Any


def build_preflight_report(
    *,
    records: list[dict[str, Any]],
    contract: dict[str, Any],
    adapters: list[dict[str, Any]],
) -> dict[str, Any]:
    expected_pairs = contract["acceptance"]["schema_compliance"]["denominator_pairs"]
    pair_ids = [record.get("pair_id") for record in records]
    integrity_errors = []
    if len(records) != expected_pairs:
        integrity_errors.append(
            f"record_count:{len(records)}:expected:{expected_pairs}"
        )
    if len(pair_ids) != len(set(pair_ids)):
        integrity_errors.append("duplicate_pair_id")

    schema_passes = sum(
        1 for record in records if record.get("validation", {}).get("valid") is True
    )
    error_counts: Counter[str] = Counter()
    for record in records:
        for error in record.get("validation", {}).get("errors", []):
            error_counts[str(error)] += 1
    systematic_unknown = {
        error: count
        for error, count in error_counts.items()
        if "unknown variant" in error
        and count
        > contract["acceptance"]["systematic_unknown_variant"][
            "maximum_occurrences_per_variant"
        ]
    }
    host_error_fragments = ("start_byte", "end_byte", "binding_sha256", "oracle id")
    host_owned_errors = sum(
        count
        for error, count in error_counts.items()
        if any(fragment in error.lower() for fragment in host_error_fragments)
    )

    command_adapter_ids = {
        adapter["adapter_id"]
        for adapter in adapters
        if adapter["executor"]["kind"] == "sandbox_command"
    }
    command_rows = [
        evaluation
        for record in records
        for evaluation in record.get("oracle_evaluations", [])
        if evaluation.get("adapter_id") in command_adapter_ids
    ]
    command_passes = sum(1 for row in command_rows if row.get("result") == "pass")
    command_rate = command_passes / len(command_rows) if command_rows else 0.0

    fix_adapter_ids = {
        adapter["adapter_id"]
        for adapter in adapters
        if adapter["case_id"].startswith("fix-")
    }
    fix_rows = [
        evaluation
        for record in records
        for evaluation in record.get("oracle_evaluations", [])
        if evaluation.get("adapter_id") in fix_adapter_ids
    ]
    fix_contract_matches = sum(
        1
        for row in fix_rows
        if row.get("reason") != "candidate_oracle_contract_not_matched"
    )
    fix_integrity = fix_contract_matches / len(fix_rows) if fix_rows else 0.0

    wrong_arm_rows = [
        row
        for record in records
        for field, expected_arm in (
            ("oracle_evaluations", "candidate"),
            ("baseline_oracle_evaluations", "baseline"),
        )
        for row in record.get(field, [])
        if row.get("arm") != expected_arm
    ]
    checks = {
        "record_integrity": not integrity_errors,
        "schema_compliance": schema_passes
        >= contract["acceptance"]["schema_compliance"]["minimum_passes"],
        "systematic_unknown_variant": not systematic_unknown,
        "host_owned_validator_errors": host_owned_errors
        <= contract["acceptance"]["host_owned_validator_errors"],
        "command_oracle_success": command_rate
        >= contract["acceptance"]["command_oracle_success"]["minimum_rate"],
        "fix_registry_reference_integrity": fix_integrity
        >= contract["acceptance"]["fix_registry_reference_integrity"],
        "baseline_observation_copy": not wrong_arm_rows,
    }
    return {
        "schema_version": "commandagent.goal_verify.preflight_report.v2",
        "interpretation": "contract-integration readiness only; never a product GO decision",
        "ready_for_full_experiment_design": all(checks.values()),
        "record_count": len(records),
        "schema_passes": schema_passes,
        "command_oracle": {
            "passes": command_passes,
            "denominator": len(command_rows),
            "success_rate": command_rate,
        },
        "fix_adapter_contract": {
            "matches": fix_contract_matches,
            "denominator": len(fix_rows),
            "integrity_rate": fix_integrity,
        },
        "systematic_unknown_variants": systematic_unknown,
        "host_owned_validator_error_count": host_owned_errors,
        "arm_binding_error_count": len(wrong_arm_rows),
        "integrity_errors": integrity_errors,
        "checks": checks,
    }
