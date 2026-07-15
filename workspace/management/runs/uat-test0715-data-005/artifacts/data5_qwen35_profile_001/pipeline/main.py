#!/usr/bin/env python3
"""
Pipeline: Read data/sales.csv, compute monthly × regional sales aggregation
and overall totals, exclude invalid rows by reason with counts, and produce
output/results.json and output/report.md.

Uses only Python 3 standard library (csv, json, datetime, statistics).
Deterministic: no randomness, stable iteration order.
"""

import csv
import json
import os
from collections import OrderedDict
from datetime import datetime

# Paths
BASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DATA_FILE = os.path.join(BASE_DIR, "data", "sales.csv")
OUTPUT_DIR = os.path.join(BASE_DIR, "output")
RESULTS_FILE = os.path.join(OUTPUT_DIR, "results.json")
REPORT_FILE = os.path.join(OUTPUT_DIR, "report.md")


def validate_date(date_str):
    """Return True if date_str is a valid YYYY-MM-DD date."""
    try:
        datetime.strptime(date_str, "%Y-%m-%d")
        return True
    except ValueError:
        return False


def main():
    os.makedirs(OUTPUT_DIR, exist_ok=True)

    # Read all rows
    all_rows = []
    with open(DATA_FILE, "r", encoding="utf-8") as f:
        reader = csv.DictReader(f)
        for row in reader:
            all_rows.append(row)

    input_row_count = len(all_rows)

    # Validate rows
    valid_rows = []
    excluded_reasons = {}  # reason -> count

    for row in all_rows:
        date_str = row.get("date", "").strip()
        region = row.get("region", "").strip()
        amount_str = row.get("amount", "").strip()

        issues = []

        # Check date validity
        if not date_str or not validate_date(date_str):
            issues.append("invalid_date")

        # Check region is not empty
        if not region:
            issues.append("missing_region")

        # Check amount is a valid number
        if not amount_str:
            issues.append("missing_amount")
        else:
            try:
                float(amount_str)
            except ValueError:
                issues.append("invalid_amount")

        if issues:
            reason = ", ".join(sorted(set(issues)))
            excluded_reasons[reason] = excluded_reasons.get(reason, 0) + 1
        else:
            valid_rows.append(row)

    used_row_count = len(valid_rows)

    # Aggregate by month and region
    # Extract month from date (YYYY-MM)
    monthly_region = {}  # (month, region) -> total amount
    monthly_totals = {}  # month -> total amount
    region_totals = {}   # region -> total amount
    overall_total = 0.0

    for row in valid_rows:
        date_str = row["date"].strip()
        region = row["region"].strip()
        amount = float(row["amount"].strip())

        month = date_str[:7]  # YYYY-MM

        key = (month, region)
        monthly_region[key] = monthly_region.get(key, 0.0) + amount

        monthly_totals[month] = monthly_totals.get(month, 0.0) + amount
        region_totals[region] = region_totals.get(region, 0.0) + amount
        overall_total += amount

    # Build ordered results for determinism
    # Sort months and regions
    sorted_months = sorted(monthly_totals.keys())
    sorted_regions = sorted(region_totals.keys())

    # Build values dict with ordered keys
    values = OrderedDict()
    for month in sorted_months:
        for region in sorted_regions:
            key = f"{month}_{region}"
            if (month, region) in monthly_region:
                values[key] = round(monthly_region[(month, region)], 2)
            else:
                values[key] = 0.0

    # Add overall total
    values["overall_total"] = round(overall_total, 2)

    # Build excluded list
    excluded_list = []
    for reason in sorted(excluded_reasons.keys()):
        excluded_list.append({"reason": reason, "rows": excluded_reasons[reason]})

    # Build results
    results = {
        "reconciliation": {
            "input_rows": input_row_count,
            "used_rows": used_row_count,
            "excluded": excluded_list
        },
        "values": values
    }

    # Write results.json
    with open(RESULTS_FILE, "w", encoding="utf-8") as f:
        json.dump(results, f, ensure_ascii=False, indent=2)
        f.write("\n")

    # Generate report.md
    report_lines = []
    report_lines.append("# 売上集計レポート")
    report_lines.append("")
    report_lines.append("## データ整合性 (Reconciliation)")
    report_lines.append("")
    report_lines.append(f"- 入力行数: {input_row_count}")
    report_lines.append(f"- 使用行数: {used_row_count}")
    report_lines.append(f"- 除外行数: {input_row_count - used_row_count}")
    report_lines.append("")
    report_lines.append("| 除外理由 | 行数 |")
    report_lines.append("|----------|------|")
    for item in excluded_list:
        report_lines.append(f"| {item['reason']} | {item['rows']} |")
    report_lines.append("")
    report_lines.append("## 月次×地域別売上 (Monthly × Regional Sales)")
    report_lines.append("")
    report_lines.append("| 月 | 東京 | 大阪 | 名古屋 |")
    report_lines.append("|----|------|------|--------|")
    for month in sorted_months:
        tokyo = monthly_region.get((month, "東京"), 0.0)
        osaka = monthly_region.get((month, "大阪"), 0.0)
        nagoya = monthly_region.get((month, "名古屋"), 0.0)
        total = monthly_totals[month]
        report_lines.append(
            f"| {month} | {tokyo:.2f} | {osaka:.2f} | {nagoya:.2f} |"
        )
    report_lines.append("")
    report_lines.append("## 地域別合計 (Regional Totals)")
    report_lines.append("")
    report_lines.append("| 地域 | 合計 |")
    report_lines.append("|------|------|")
    for region in sorted_regions:
        report_lines.append(f"| {region} | {region_totals[region]:.2f} |")
    report_lines.append("")
    report_lines.append("## 全体合計 (Overall Total)")
    report_lines.append("")
    report_lines.append(f"**{overall_total:.2f}**")
    report_lines.append("")

    with open(REPORT_FILE, "w", encoding="utf-8") as f:
        f.write("\n".join(report_lines))

    print(f"Results written to {RESULTS_FILE}")
    print(f"Report written to {REPORT_FILE}")


if __name__ == "__main__":
    main()
