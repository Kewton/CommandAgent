"""Curate the same-instrument CM-4x E extension and n=36 aggregate."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

import cm3_matrix
import cm3b_matrix
import cm4_planner_cand

EXECUTION_REVISION = "f2072b570b5eddde06215e8025cce859335c7916"
BINARY_SHA256 = "b9f9818602d34c1b383a1910bcaf0c8737d596bcf0d792f5b3e0399d330c13fa"


def classify_terminal(reason: str, product_exit: int | None) -> str | None:
    if product_exit == 0:
        return None
    signatures = (
        ("path does not exist: app.spec.yaml", "community_spec_artifact_missing"),
        ("stdin is not a TTY", "community_verify_instruction_not_executable"),
        ("dangerous command blocked", "community_dangerous_command_blocked"),
        ("community_package_missing", "community_package_missing"),
        ("community_computed_unregistered", "community_computed_unregistered"),
        ("path does not exist: .bench-product-stdout.md", "community_workspace_path_invented"),
        ("path does not exist: core.yaml", "community_workspace_path_invented"),
    )
    for token, class_id in signatures:
        if token in reason:
            return class_id
    return "unclassified_terminal"


def extension_rows(meta_path: Path, pricing: Path) -> list[dict[str, Any]]:
    metadata = json.loads(meta_path.read_text(encoding="utf-8"))
    preflight = metadata["preflight"]
    if preflight["head_sha"] != EXECUTION_REVISION:
        raise ValueError("extension execution revision drifted")
    if preflight["binary_sha256"]["installed"] != BINARY_SHA256:
        raise ValueError("extension binary SHA-256 drifted")
    if metadata["suite"].get("think") != "medium":
        raise ValueError("extension think declaration drifted")
    records = {record["name"]: record for record in metadata["runs"]}
    rows = cm3_matrix.live_observations("E", meta_path, pricing)
    for row in rows:
        record = records[row["id"]]
        if "--think=medium" not in record["command_argv"]:
            raise ValueError(f"{row['id']} does not declare --think=medium")
        row["source"] = "cm4-planner-cand-002 same-instrument extension"
        row["think"] = "medium"
        row["stop_class"] = classify_terminal(
            str(record.get("terminal_reason") or ""), record.get("product_exit")
        )
        row["outcome_signature"] = row["stop_class"] or "full"
    return rows


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--candidate-001", type=Path, required=True)
    parser.add_argument("--extension-meta", type=Path, required=True)
    parser.add_argument("--pricing", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    previous = json.loads(args.candidate_001.read_text(encoding="utf-8"))
    existing_rows = [row for row in previous["runs"] if row["arm"] == "E"]
    if len(existing_rows) != 12:
        raise ValueError("candidate-001 E denominator must remain 12")
    new_rows = extension_rows(args.extension_meta, args.pricing)
    if len(new_rows) != 24:
        raise ValueError("candidate-002 extension denominator must be 24")
    combined_rows = existing_rows + new_rows
    baseline = previous["baseline"]
    existing_summary = cm3_matrix.summarize_arm(existing_rows)
    extension_summary = cm3_matrix.summarize_arm(new_rows)
    combined_summary = cm3_matrix.summarize_arm(combined_rows)
    document = {
        "schema_version": "commandagent.cm4x-planner-candidate-summary/v1",
        "series_id": "cm4-planner-cand-002",
        "denominator": 36,
        "execution_revision": EXECUTION_REVISION,
        "binary_sha256": BINARY_SHA256,
        "baseline": {"label": "A", **baseline},
        "existing_e_12": existing_summary,
        "extension_e_24": extension_summary,
        "combined_e_36": combined_summary,
        "comparisons": {
            "A_to_E36": cm3b_matrix.comparison(baseline, combined_summary),
            "E12_to_extension24_descriptive": {
                "full_rate_delta": (
                    extension_summary["full_rate"] - existing_summary["full_rate"]
                ),
                "one_shot_rate_delta": (
                    extension_summary["one_shot_full_rate"]
                    - existing_summary["one_shot_full_rate"]
                ),
                "note": "independent-arm CI is not applied to the overlapping E36 aggregate",
            },
        },
        "campaign_window": cm4_planner_cand.campaign_window(args.extension_meta),
        "evidence_hashes": cm4_planner_cand.evidence_hashes(
            "E-extension", args.extension_meta
        ),
        "extension_cost_usd": extension_summary["cost_usd"]["total"],
        "combined_cost_usd": combined_summary["cost_usd"]["total"],
        "local_planner_cost_usd": 0,
        "adoption_decision": "owner_adjudication_pending",
        "runs": combined_rows,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(document, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )


if __name__ == "__main__":
    main()
