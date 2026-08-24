#!/usr/bin/env python3
"""
Sales data pipeline: reads data/sales.csv, aggregates by month x region,
excludes invalid rows, and produces output/results.json and output/report.md.
Uses only Python 3 standard library (csv, json, datetime, statistics).
Deterministic: fixed seed, stable ordering, no time-dependent logic.
"""

import csv
import json
import os
import sys
from datetime import datetime
from collections import defaultdict

# ── Paths ──────────────────────────────────────────────────────────────
BASE_DIR = os.path.dirname(os.path.abspath(__file__))
ROOT_DIR = os.path.dirname(BASE_DIR)
DATA_PATH = os.path.join(ROOT_DIR, "data", "sales.csv")
OUTPUT_DIR = os.path.join(ROOT_DIR, "output")
RESULTS_PATH = os.path.join(OUTPUT_DIR, "results.json")
REPORT_PATH = os.path.join(OUTPUT_DIR, "report.md")

# ── Known valid regions (from inspection) ──────────────────────────────
VALID_REGIONS = {"東京", "大阪", "名古屋"}


def parse_date(date_str):
    """Return a datetime.date if valid YYYY-MM-DD, else None."""
    date_str = date_str.strip()
    if not date_str:
        return None
    try:
        return datetime.strptime(date_str, "%Y-%m-%d").date()
    except ValueError:
        return None


def extract_month(date_obj):
    """Return 'YYYY-MM' string from a date object."""
    return date_obj.strftime("%Y-%m")


def main():
    os.makedirs(OUTPUT_DIR, exist_ok=True)

    # ── Read CSV ───────────────────────────────────────────────────────
    rows_raw = []
    with open(DATA_PATH, "r", encoding="utf-8") as f:
        reader = csv.DictReader(f)
        for row in reader:
            rows_raw.append(row)

    input_rows = len(rows_raw)

    # ── Validate & separate ────────────────────────────────────────────
    valid_rows = []
    exclusion_reasons = defaultdict(int)

    for row in rows_raw:
        date_str = row.get("date", "").strip()
        region = row.get("region", "").strip()
        amount_str = row.get("amount", "").strip()

        # Check date
        if not date_str:
            exclusion_reasons["missing_date"] += 1
            continue
        date_obj = parse_date(date_str)
        if date_obj is None:
            exclusion_reasons["invalid_date"] += 1
            continue

        # Check region
        if region not in VALID_REGIONS:
            exclusion_reasons["invalid_region"] += 1
            continue

        # Check amount
        try:
            amount = float(amount_str)
        except (ValueError, TypeError):
            exclusion_reasons["invalid_amount"] += 1
            continue

        valid_rows.append({
            "date": date_obj,
            "month": extract_month(date_obj),
            "region": region,
            "amount": amount,
        })

    used_rows = len(valid_rows)
    excluded_list = [
        {"reason": reason, "rows": count}
        for reason, count in sorted(exclusion_reasons.items())
    ]

    # ── Aggregate by month x region ────────────────────────────────────
    agg = defaultdict(float)
    for vr in valid_rows:
        key = f"{vr['month']}_{vr['region']}"
        agg[key] += vr["amount"]

    # Sort keys deterministically
    sorted_keys = sorted(agg.keys())

    # ── Build values dict ──────────────────────────────────────────────
    values = {}
    for key in sorted_keys:
        values[f"monthly_region_{key}"] = round(agg[key], 2)

    # Overall total
    overall_total = round(sum(agg.values()), 2)
    values["overall_total"] = overall_total

    # Per-region totals
    region_totals = defaultdict(float)
    for vr in valid_rows:
        region_totals[vr["region"]] += vr["amount"]
    for region in sorted(region_totals.keys()):
        values[f"region_total_{region}"] = round(region_totals[region], 2)

    # Per-month totals
    month_totals = defaultdict(float)
    for vr in valid_rows:
        month_totals[vr["month"]] += vr["amount"]
    for month in sorted(month_totals.keys()):
        values[f"month_total_{month}"] = round(month_totals[month], 2)

    # ── Write results.json ─────────────────────────────────────────────
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

    # ── Write report.md ────────────────────────────────────────────────
    lines = []
    lines.append("# 売上レポート")
    lines.append("")
    lines.append("## 集計概要")
    lines.append("")
    lines.append(f"- 入力行数: {input_rows}")
    lines.append(f"- 使用行数: {used_rows}")
    lines.append(f"- 除外行数: {input_rows - used_rows}")
    lines.append("")
    lines.append("## 除外理由")
    lines.append("")
    if excluded_list:
        for ex in excluded_list:
            lines.append(f"- {ex['reason']}: {ex['rows']} 件")
    else:
        lines.append("- なし")
    lines.append("")
    lines.append("## 月次×地域別売上")
    lines.append("")
    lines.append("| 月 | 地域 | 売上 |")
    lines.append("|-----|------|------|")
    for key in sorted_keys:
        parts = key.split("_")
        month = "_".join(parts[:-1])
        region = parts[-1]
        total = values[f"monthly_region_{key}"]
        lines.append(f"| {month} | {region} | {total} |")
    lines.append("")
    lines.append("## 全体合計")
    lines.append("")
    lines.append(f"全体の売上合計: {overall_total}")
    lines.append("")
    lines.append("## 地域別合計")
    lines.append("")
    for region in sorted(region_totals.keys()):
        lines.append(f"- {region}: {values[f'region_total_{region}']}")
    lines.append("")
    lines.append("## 月別合計")
    lines.append("")
    for month in sorted(month_totals.keys()):
        lines.append(f"- {month}: {values[f'month_total_{month}']}")
    lines.append("")

    with open(REPORT_PATH, "w", encoding="utf-8") as f:
        f.write("\n".join(lines))

    print(f"Pipeline complete: {input_rows} input rows, {used_rows} used, {input_rows - used_rows} excluded")
    print(f"Results written to {RESULTS_PATH}")
    print(f"Report written to {REPORT_PATH}")


if __name__ == "__main__":
    main()
