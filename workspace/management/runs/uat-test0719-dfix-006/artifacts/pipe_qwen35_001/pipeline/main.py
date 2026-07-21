#!/usr/bin/env python3
"""
pipeline/main.py — 月次×地域売上集計パイプライン

data/sales.csv を読み込み、月次×地域の売上集計と全体合計を計算し、
無効な行は理由別に除外して件数を明記した上で、要約レポートを作成する。
"""

import csv
import json
import os
import sys
from datetime import datetime
from collections import defaultdict

# ---------- configuration ----------
BASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DATA_CSV = os.path.join(BASE_DIR, "data", "sales.csv")
OUTPUT_DIR = os.path.join(BASE_DIR, "output")
RESULTS_JSON = os.path.join(OUTPUT_DIR, "results.json")
REPORT_MD = os.path.join(OUTPUT_DIR, "report.md")

# ---------- helpers ----------

def read_csv(path):
    """Read CSV and return list of dicts (one per data row)."""
    rows = []
    with open(path, newline="", encoding="utf-8") as f:
        reader = csv.DictReader(f)
        for row in reader:
            rows.append(dict(row))
    return rows


def validate_row(row):
    """
    Return (is_valid, reason_or_None).
    - Missing date: date field is empty or whitespace.
    - Invalid date: date field is non-empty but not a valid YYYY-MM-DD date.
    """
    date_str = row.get("date", "").strip()
    if not date_str:
        return False, "missing_date"
    try:
        datetime.strptime(date_str, "%Y-%m-%d")
    except ValueError:
        return False, "invalid_date"
    return True, None


def parse_amount(val):
    """Parse amount string to int."""
    return int(val.strip())


# ---------- main pipeline ----------

def run():
    os.makedirs(OUTPUT_DIR, exist_ok=True)

    # 1. Read data
    raw_rows = read_csv(DATA_CSV)
    input_rows = len(raw_rows)

    # 2. Validate rows
    valid_rows = []
    exclusion_counts = defaultdict(int)
    for row in raw_rows:
        is_valid, reason = validate_row(row)
        if is_valid:
            valid_rows.append(row)
        else:
            exclusion_counts[reason] += 1

    used_rows = len(valid_rows)
    excluded_list = sorted(
        [{"reason": r, "rows": c} for r, c in exclusion_counts.items()],
        key=lambda x: x["reason"],
    )

    # 3. Aggregate by month × region
    #    month: 1-6, region: 東京/大阪/名古屋
    agg = defaultdict(lambda: defaultdict(int))  # month -> region -> sum
    for row in valid_rows:
        date_str = row["date"].strip()
        dt = datetime.strptime(date_str, "%Y-%m-%d")
        month = dt.month
        region = row["region"].strip()
        amount = parse_amount(row["amount"])
        agg[month][region] += amount

    # 4. Compute overall totals
    total_by_month = {}
    for m in sorted(agg.keys()):
        total_by_month[m] = sum(agg[m].values())

    total_all = sum(total_by_month.values())

    # 5. Build claim keys and values
    #    claim keys: "total", "month_X", "month_X_region"
    values = {}
    values["total"] = total_all

    regions_sorted = sorted(set(r for m in agg for r in agg[m]))
    for m in sorted(agg.keys()):
        values[f"month_{m}"] = total_by_month[m]
        for region in regions_sorted:
            if region in agg[m]:
                values[f"month_{m}_{region}"] = agg[m][region]

    # 6. Write results.json
    results = {
        "reconciliation": {
            "input_rows": input_rows,
            "used_rows": used_rows,
            "excluded": excluded_list,
        },
        "values": values,
    }
    with open(RESULTS_JSON, "w", encoding="utf-8") as f:
        json.dump(results, f, ensure_ascii=False, indent=2)
        f.write("\n")

    # 7. Write report.md
    lines = []
    lines.append("# 売上集計レポート")
    lines.append("")
    lines.append("## 概要")
    lines.append("")
    lines.append(f"- 入力行数: {input_rows}")
    lines.append(f"- 使用行数: {used_rows}")
    lines.append(f"- 除外行数: {input_rows - used_rows}")
    lines.append("")
    lines.append("## 除外理由")
    lines.append("")
    for ex in excluded_list:
        lines.append(f"- {ex['reason']}: {ex['rows']} 行")
    lines.append("")
    lines.append("## 月次×地域別売上")
    lines.append("")
    lines.append("| 月 | 地域 | 売上 |")
    lines.append("|---:|:-----|---:|")
    for m in sorted(agg.keys()):
        for region in regions_sorted:
            if region in agg[m]:
                lines.append(f"| {m}月 | {region} | {agg[m][region]} |")
    lines.append("")
    lines.append("## 月次合計")
    lines.append("")
    lines.append("| 月 | 合計 |")
    lines.append("|---:|---:|")
    for m in sorted(total_by_month.keys()):
        lines.append(f"| {m}月 | {total_by_month[m]} |")
    lines.append("")
    lines.append("## 全体合計")
    lines.append("")
    lines.append(f"**{total_all}**")
    lines.append("")

    with open(REPORT_MD, "w", encoding="utf-8") as f:
        f.write("\n".join(lines))

    print(f"Done. input_rows={input_rows}, used_rows={used_rows}, total={total_all}")
    print(f"Results written to {RESULTS_JSON}")
    print(f"Report written to {REPORT_MD}")


if __name__ == "__main__":
    run()
