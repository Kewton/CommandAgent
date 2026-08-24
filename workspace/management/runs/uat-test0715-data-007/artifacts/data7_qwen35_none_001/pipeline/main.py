#!/usr/bin/env python3
"""
Tabular-data pipeline: read data/sales.csv, validate rows, aggregate by month×region,
compute overall totals, and produce output/inspection.json, output/results.json,
and output/report.md.

Uses only Python 3 standard library (csv, json, datetime, os, statistics).
Deterministic: fixed ordering, no time-dependent logic, no randomness.
"""

import csv
import json
import os
import sys
from datetime import datetime
from collections import defaultdict

BASE_DIR = os.path.dirname(os.path.abspath(__file__))
PROJECT_DIR = os.path.dirname(BASE_DIR)
DATA_PATH = os.path.join(PROJECT_DIR, "data", "sales.csv")
OUTPUT_DIR = os.path.join(PROJECT_DIR, "output")
INSPECTION_PATH = os.path.join(OUTPUT_DIR, "inspection.json")
RESULTS_PATH = os.path.join(OUTPUT_DIR, "results.json")
REPORT_PATH = os.path.join(OUTPUT_DIR, "report.md")


def validate_row(row, row_index):
    """Validate a single row. Returns (is_valid, reason) tuple."""
    date_str = row.get("date", "").strip()
    region = row.get("region", "").strip()
    amount_str = row.get("amount", "").strip()

    # Check missing date
    if not date_str:
        return False, "missing_date"

    # Check invalid date format / unparseable date
    try:
        dt = datetime.strptime(date_str, "%Y-%m-%d")
        # Verify the parsed date matches the input (catches things like Feb 30)
        if dt.strftime("%Y-%m-%d") != date_str:
            return False, "invalid_date"
    except ValueError:
        return False, "invalid_date"

    # Check non-numeric amount
    try:
        amount = float(amount_str)
    except (ValueError, TypeError):
        return False, "invalid_amount"

    return True, None


def main():
    os.makedirs(OUTPUT_DIR, exist_ok=True)

    # --- Read CSV ---
    rows = []
    with open(DATA_PATH, "r", newline="", encoding="utf-8") as f:
        reader = csv.DictReader(f)
        for row in reader:
            rows.append(row)

    input_rows = len(rows)

    # --- Validate rows ---
    valid_rows = []
    exclusion_counts = defaultdict(int)

    for i, row in enumerate(rows):
        is_valid, reason = validate_row(row, i)
        if is_valid:
            valid_rows.append(row)
        else:
            exclusion_counts[reason] += 1

    used_rows = len(valid_rows)

    # --- Column observations for inspection.json ---
    regions_observed = sorted(set(r["region"].strip() for r in valid_rows))
    amounts = []
    for r in valid_rows:
        try:
            amounts.append(float(r["amount"].strip()))
        except (ValueError, TypeError):
            pass

    amount_stats = {}
    if amounts:
        amount_stats = {
            "min": min(amounts),
            "max": max(amounts),
            "sum": sum(amounts),
            "count": len(amounts),
        }

    date_samples = sorted(set(r["date"].strip() for r in valid_rows))[:5]

    inspection = {
        "column_observations": {
            "columns": ["date", "region", "amount"],
            "total_rows": input_rows,
            "regions_observed": regions_observed,
            "amount_stats": amount_stats,
            "date_samples": date_samples,
        },
        "validation_summary": {
            "input_rows": input_rows,
            "used_rows": used_rows,
            "excluded": sorted(
                [{"reason": k, "rows": v} for k, v in exclusion_counts.items()],
                key=lambda x: x["reason"],
            ),
        },
    }

    with open(INSPECTION_PATH, "w", encoding="utf-8") as f:
        json.dump(inspection, f, indent=2, ensure_ascii=False)
        f.write("\n")

    # --- Aggregate by month × region ---
    # Extract month from date and aggregate
    monthly_region = defaultdict(float)
    region_totals = defaultdict(float)
    month_totals = defaultdict(float)

    for row in valid_rows:
        date_str = row["date"].strip()
        region = row["region"].strip()
        amount = float(row["amount"].strip())
        dt = datetime.strptime(date_str, "%Y-%m-%d")
        month_key = dt.strftime("%Y-%m")

        key = f"{month_key}_{region}"
        monthly_region[key] += amount
        region_totals[region] += amount
        month_totals[month_key] += amount

    # Sort keys deterministically
    sorted_monthly = sorted(monthly_region.keys())
    sorted_regions = sorted(region_totals.keys())
    sorted_months = sorted(month_totals.keys())

    # Build values dict
    values = {}
    for key in sorted_monthly:
        values[key] = round(monthly_region[key], 2)

    grand_total = round(sum(monthly_region.values()), 2)
    values["grand_total"] = grand_total

    for region in sorted_regions:
        values[f"total_{region}"] = round(region_totals[region], 2)

    for month in sorted_months:
        values[f"total_{month}"] = round(month_totals[month], 2)

    # --- Build results.json ---
    excluded_list = sorted(
        [{"reason": k, "rows": v} for k, v in exclusion_counts.items()],
        key=lambda x: x["reason"],
    )

    results = {
        "reconciliation": {
            "input_rows": input_rows,
            "used_rows": used_rows,
            "excluded": excluded_list,
        },
        "values": values,
    }

    with open(RESULTS_PATH, "w", encoding="utf-8") as f:
        json.dump(results, f, indent=2, ensure_ascii=False)
        f.write("\n")

    # --- Validate reconciliation ---
    sum_excluded = sum(e["rows"] for e in excluded_list)
    assert input_rows == used_rows + sum_excluded, (
        f"Reconciliation failed: {input_rows} != {used_rows} + {sum_excluded}"
    )

    # --- Build report.md ---
    lines = []
    lines.append("# Sales Data Pipeline Report")
    lines.append("")
    lines.append("## Validation Summary")
    lines.append(f"- Input rows: {input_rows}")
    lines.append(f"- Used rows: {used_rows}")
    lines.append(f"- Excluded rows: {sum_excluded}")
    lines.append("")
    lines.append("| Reason | Count |")
    lines.append("|--------|-------|")
    for e in excluded_list:
        lines.append(f"| {e['reason']} | {e['rows']} |")
    lines.append("")
    lines.append("## Monthly × Regional Sales")
    lines.append("")
    lines.append("| Month | Region | Amount |")
    lines.append("|-------|--------|--------|")
    for key in sorted_monthly:
        month, region = key.rsplit("_", 1)
        lines.append(f"| {month} | {region} | {int(values[key])} |")
    lines.append("")
    lines.append("## Totals")
    lines.append(f"- Grand total: {int(grand_total)}")
    lines.append("")
    lines.append("| Region | Total |")
    lines.append("|--------|-------|")
    for region in sorted_regions:
        lines.append(f"| {region} | {int(values[f'total_{region}'])} |")
    lines.append("")
    lines.append("| Month | Total |")
    lines.append("|-------|-------|")
    for month in sorted_months:
        lines.append(f"| {month} | {int(values[f'total_{month}'])} |")
    lines.append("")

    with open(REPORT_PATH, "w", encoding="utf-8") as f:
        f.write("\n".join(lines))

    print(f"Pipeline complete: {input_rows} input rows, {used_rows} used, {sum_excluded} excluded")
    return 0


if __name__ == "__main__":
    sys.exit(main())
