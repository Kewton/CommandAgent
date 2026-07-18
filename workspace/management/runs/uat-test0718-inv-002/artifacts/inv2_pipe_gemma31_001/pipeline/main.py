#!/usr/bin/env python3
"""
sales_pipeline.py - Monthly × Regional Sales Aggregation Pipeline

Reads data/sales.csv, validates rows, aggregates sales by month and region,
computes total sales, and writes:
  - output/inspection.json
  - output/results.json
  - output/report.md
"""

import csv
import json
import os
import calendar
from collections import defaultdict

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------
BASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DATA_FILE = os.path.join(BASE_DIR, "data", "sales.csv")
OUTPUT_DIR = os.path.join(BASE_DIR, "output")

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def is_valid_date(date_str: str) -> bool:
    """Return True if date_str is a valid YYYY-MM-DD date."""
    try:
        parts = date_str.strip().split("-")
        if len(parts) != 3:
            return False
        year, month, day = int(parts[0]), int(parts[1]), int(parts[2])
        if month < 1 or month > 12:
            return False
        max_day = calendar.monthrange(year, month)[1]
        return 1 <= day <= max_day
    except (ValueError, TypeError):
        return False


def extract_month(date_str: str) -> str:
    """Extract YYYY-MM from a valid date string."""
    return date_str.strip()[:7]


# ---------------------------------------------------------------------------
# Main pipeline
# ---------------------------------------------------------------------------

def run():
    os.makedirs(OUTPUT_DIR, exist_ok=True)

    # ---- 1. Read & Validate ------------------------------------------------
    valid_rows = []
    excluded_reasons: dict[str, int] = defaultdict(int)
    input_row_count = 0

    with open(DATA_FILE, newline="", encoding="utf-8") as fh:
        reader = csv.DictReader(fh)
        for row in reader:
            input_row_count += 1
            date_val = row.get("date", "").strip()
            region_val = row.get("region", "").strip()
            amount_val = row.get("amount", "").strip()

            # Validation
            if not date_val:
                excluded_reasons["missing_date"] += 1
                continue
            if not is_valid_date(date_val):
                excluded_reasons["invalid_date"] += 1
                continue
            if not region_val:
                excluded_reasons["missing_region"] += 1
                continue
            if not amount_val:
                excluded_reasons["missing_amount"] += 1
                continue
            try:
                amount = float(amount_val)
            except ValueError:
                excluded_reasons["invalid_amount"] += 1
                continue

            valid_rows.append({
                "date": date_val,
                "month": extract_month(date_val),
                "region": region_val,
                "amount": amount,
            })

    used_row_count = len(valid_rows)

    # ---- 2. Aggregate by month × region ------------------------------------
    # Key: (month, region)  Value: sum of amounts
    agg: dict[tuple[str, str], float] = defaultdict(float)
    for v in valid_rows:
        agg[(v["month"], v["region"])] += v["amount"]

    # Deterministic ordering: sort by (month, region)
    sorted_agg = sorted(agg.items(), key=lambda x: (x[0][0], x[0][1]))

    # ---- 3. Build results.json ---------------------------------------------
    excluded_list = sorted(
        [{"reason": k, "rows": v} for k, v in excluded_reasons.items()],
        key=lambda x: x["reason"],
    )

    # Build values dict with deterministic key ordering
    values: dict[str, float] = {}
    for (month, region), total in sorted_agg:
        key = f"{month}_{region}"
        values[key] = round(total, 2)

    # Total sales claim
    total_sales = sum(v["amount"] for v in valid_rows)
    values["total_sales"] = round(total_sales, 2)

    results = {
        "reconciliation": {
            "input_rows": input_row_count,
            "used_rows": used_row_count,
            "excluded": excluded_list,
        },
        "values": values,
    }

    # ---- 4. Write output/inspection.json -----------------------------------
    inspection = {
        "columns": ["date", "region", "amount"],
        "total_rows": input_row_count,
        "valid_rows": used_row_count,
        "invalid_rows": input_row_count - used_row_count,
        "invalid_reasons": dict(excluded_reasons),
        "valid_sample": valid_rows[:5] if valid_rows else [],
    }

    with open(os.path.join(OUTPUT_DIR, "inspection.json"), "w", encoding="utf-8") as f:
        json.dump(inspection, f, indent=2, ensure_ascii=False)

    # ---- 5. Write output/results.json --------------------------------------
    with open(os.path.join(OUTPUT_DIR, "results.json"), "w", encoding="utf-8") as f:
        json.dump(results, f, indent=2, ensure_ascii=False)

    # ---- 6. Write output/report.md -----------------------------------------
    report_lines = [
        "# 売上集計レポート",
        "",
        "## 集計概要",
        "",
        f"- 入力行数: {input_row_count}",
        f"- 使用行数: {used_row_count}",
        f"- 除外行数: {input_row_count - used_row_count}",
        "",
        "## 除外理由",
        "",
    ]
    for item in excluded_list:
        report_lines.append(f"- {item['reason']}: {item['rows']} 件")
    report_lines.append("")
    report_lines.append("## 月次×地域別売上", "")
    report_lines.append("| 月 | 地域 | 売上 |")
    report_lines.append("|---|------|------|")
    for (month, region), total in sorted_agg:
        report_lines.append(f"| {month} | {region} | {total:.2f} |")
    report_lines.append("")
    report_lines.append(f"### 全体合計: {total_sales:.2f}", "")

    with open(os.path.join(OUTPUT_DIR, "report.md"), "w", encoding="utf-8") as f:
        f.write("\n".join(report_lines) + "\n")

    print(f"Pipeline complete: {used_row_count}/{input_row_count} rows used")
    print(f"Total sales: {total_sales:.2f}")
    print(f"Excluded reasons: {dict(excluded_reasons)}")


if __name__ == "__main__":
    run()
