#!/usr/bin/env python3
"""
pipeline/main.py - Sales data pipeline

Reads data/sales.csv, validates rows, aggregates sales by month and region,
calculates totals, and outputs:
  - output/inspection.json
  - output/results.json
  - output/report.md

Uses only Python 3 standard library (csv, json, datetime, statistics, os, collections).
Deterministic: fixed ordering, no time-dependent logic.
"""

import csv
import json
import os
from datetime import datetime
from collections import defaultdict

# Paths
BASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DATA_FILE = os.path.join(BASE_DIR, "data", "sales.csv")
OUTPUT_DIR = os.path.join(BASE_DIR, "output")
INSPECTION_FILE = os.path.join(OUTPUT_DIR, "inspection.json")
RESULTS_FILE = os.path.join(OUTPUT_DIR, "results.json")
REPORT_FILE = os.path.join(OUTPUT_DIR, "report.md")

VALID_REGIONS = {"東京", "大阪", "名古屋"}


def validate_row(row):
    """
    Validate a single row from sales.csv.
    Returns (is_valid, reason_or_none, date_str, region, amount_float).
    """
    date_str = row.get("date", "").strip()
    region = row.get("region", "").strip()
    amount_str = row.get("amount", "").strip()

    # Check empty date
    if not date_str:
        return False, "empty_date", None, None, None

    # Check valid date format
    try:
        dt = datetime.strptime(date_str, "%Y-%m-%d")
    except ValueError:
        return False, "invalid_date", None, None, None

    # Check region
    if region not in VALID_REGIONS:
        return False, "invalid_region", None, None, None

    # Check numeric amount
    try:
        amount = float(amount_str)
    except (ValueError, TypeError):
        return False, "non_numeric_amount", None, None, None

    return True, None, date_str, region, amount


def run_pipeline():
    """Main pipeline execution."""
    os.makedirs(OUTPUT_DIR, exist_ok=True)

    # Read and validate all rows
    all_rows = []
    valid_rows = []
    invalid_rows = []
    exclusion_reasons = defaultdict(int)

    with open(DATA_FILE, "r", encoding="utf-8") as f:
        reader = csv.DictReader(f)
        for row in reader:
            all_rows.append(row)
            is_valid, reason, date_str, region, amount = validate_row(row)
            if is_valid:
                valid_rows.append({
                    "date": date_str,
                    "region": region,
                    "amount": amount,
                })
            else:
                invalid_rows.append(row)
                exclusion_reasons[reason] += 1

    total_rows = len(all_rows)
    valid_count = len(valid_rows)
    invalid_count = len(invalid_rows)

    # Aggregate by month and region
    # Key: (year_month, region) -> total amount
    monthly_region = defaultdict(float)
    monthly_total = defaultdict(float)
    region_total = defaultdict(float)
    grand_total = 0.0

    for row in valid_rows:
        date_str = row["date"]
        region = row["region"]
        amount = row["amount"]

        # Extract year-month (YYYY-MM)
        year_month = date_str[:7]  # e.g., "2026-01"
        key = (year_month, region)
        monthly_region[key] += amount
        monthly_total[year_month] += amount
        region_total[region] += amount
        grand_total += amount

    # Build inspection.json
    inspection = {
        "total_rows": total_rows,
        "valid_rows": valid_count,
        "invalid_rows": invalid_count,
        "excluded_reasons": dict(sorted(exclusion_reasons.items())),
        "valid_regions": sorted(VALID_REGIONS),
    }

    with open(INSPECTION_FILE, "w", encoding="utf-8") as f:
        json.dump(inspection, f, indent=2, ensure_ascii=False)

    # Build results.json
    # Sort keys deterministically
    sorted_monthly_region_keys = sorted(monthly_region.keys())
    sorted_month_keys = sorted(monthly_total.keys())
    sorted_region_keys = sorted(region_total.keys())

    values = {}
    for year_month, region in sorted_monthly_region_keys:
        key = f"monthly_{year_month}_{region}"
        values[key] = monthly_region[(year_month, region)]

    for year_month in sorted_month_keys:
        key = f"month_total_{year_month}"
        values[key] = monthly_total[year_month]

    for region in sorted_region_keys:
        key = f"region_total_{region}"
        values[key] = region_total[region]

    values["grand_total"] = grand_total

    results = {
        "reconciliation": {
            "input_rows": total_rows,
            "used_rows": valid_count,
            "excluded": sorted(
                [{"reason": reason, "rows": count} for reason, count in exclusion_reasons.items()],
                key=lambda x: x["reason"],
            ),
        },
        "values": values,
    }

    with open(RESULTS_FILE, "w", encoding="utf-8") as f:
        json.dump(results, f, indent=2, ensure_ascii=False)

    # Build report.md
    lines = []
    lines.append("# 売上集計レポート")
    lines.append("")
    lines.append("## 概要")
    lines.append(f"- 入力行数: {total_rows}")
    lines.append(f"- 使用行数: {valid_count}")
    lines.append(f"- 除外行数: {invalid_count}")
    lines.append("")
    lines.append("## 除外理由")
    lines.append("| 理由 | 件数 |")
    lines.append("|------|------|")
    for reason, count in sorted(exclusion_reasons.items()):
        lines.append(f"| {reason} | {count} |")
    lines.append("")
    lines.append("## 月次×地域別売上")
    lines.append("| 月 | 地域 | 売上 |")
    lines.append("|------|------|------|")
    for year_month, region in sorted_monthly_region_keys:
        lines.append(f"| {year_month} | {region} | {monthly_region[(year_month, region)]} |")
    lines.append("")
    lines.append("## 月次合計")
    lines.append("| 月 | 合計 |")
    lines.append("|------|------|")
    for year_month in sorted_month_keys:
        lines.append(f"| {year_month} | {monthly_total[year_month]} |")
    lines.append("")
    lines.append("## 地域別合計")
    lines.append("| 地域 | 合計 |")
    lines.append("|------|------|")
    for region in sorted_region_keys:
        lines.append(f"| {region} | {region_total[region]} |")
    lines.append("")
    lines.append("## 全体合計")
    lines.append(f"全体合計: {grand_total}")
    lines.append("")

    with open(REPORT_FILE, "w", encoding="utf-8") as f:
        f.write("\n".join(lines))

    print(f"Done. {valid_count}/{total_rows} rows used. Grand total: {grand_total}")


if __name__ == "__main__":
    run_pipeline()
