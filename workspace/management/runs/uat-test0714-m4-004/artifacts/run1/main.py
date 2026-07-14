#!/usr/bin/env python3
"""
Sales data aggregation pipeline.
Reads data/sales.csv, validates rows, computes monthly × regional totals,
and writes output/inspection.json, output/results.json, output/report.md.
"""

import csv
import json
import os
import calendar
from collections import defaultdict

# ---------------------------------------------------------------------------
# Paths
# ---------------------------------------------------------------------------
BASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DATA_CSV = os.path.join(BASE_DIR, "data", "sales.csv")
OUTPUT_DIR = os.path.join(BASE_DIR, "output")
INSPECTION_JSON = os.path.join(OUTPUT_DIR, "inspection.json")
RESULTS_JSON = os.path.join(OUTPUT_DIR, "results.json")
REPORT_MD = os.path.join(OUTPUT_DIR, "report.md")

os.makedirs(OUTPUT_DIR, exist_ok=True)

# ---------------------------------------------------------------------------
# 1. Read & validate
# ---------------------------------------------------------------------------
valid_rows = []
excluded = []  # list of {"reason": str, "rows": int}

with open(DATA_CSV, newline="", encoding="utf-8") as f:
    reader = csv.DictReader(f)
    for row in reader:
        date_str = row.get("date", "").strip()
        region = row.get("region", "").strip()
        amount_str = row.get("amount", "").strip()

        # Validate date
        if not date_str:
            excluded.append({"reason": "missing_date", "rows": 1})
            continue
        try:
            year, month, day = map(int, date_str.split("-"))
            # Check if the date is actually valid (e.g. Feb 30)
            if not (1 <= month <= 12 and 1 <= day <= calendar.monthrange(year, month)[1]):
                excluded.append({"reason": "invalid_date", "rows": 1})
                continue
        except (ValueError, TypeError):
            excluded.append({"reason": "invalid_date", "rows": 1})
            continue

        # Validate region
        if not region:
            excluded.append({"reason": "empty_region", "rows": 1})
            continue

        # Validate amount
        if not amount_str:
            excluded.append({"reason": "missing_amount", "rows": 1})
            continue
        try:
            amount = float(amount_str)
        except (ValueError, TypeError):
            excluded.append({"reason": "invalid_amount", "rows": 1})
            continue

        valid_rows.append({
            "date": date_str,
            "year": year,
            "month": month,
            "region": region,
            "amount": amount,
        })

input_rows = len(valid_rows) + sum(e["rows"] for e in excluded)
used_rows = len(valid_rows)

# ---------------------------------------------------------------------------
# 2. Build inspection.json
# ---------------------------------------------------------------------------
inspection = {
    "total_input_rows": input_rows,
    "valid_rows": used_rows,
    "excluded": excluded,
    "regions": sorted(set(r["region"] for r in valid_rows)),
    "months": sorted(set((r["year"], r["month"]) for r in valid_rows)),
}

with open(INSPECTION_JSON, "w", encoding="utf-8") as f:
    json.dump(inspection, f, indent=2, ensure_ascii=False)

# ---------------------------------------------------------------------------
# 3. Aggregate: monthly × regional totals
# ---------------------------------------------------------------------------
# key: (year, month, region) -> total
agg = defaultdict(float)
region_totals = defaultdict(float)
month_totals = defaultdict(float)
grand_total = 0.0

for r in valid_rows:
    key = (r["year"], r["month"], r["region"])
    agg[key] += r["amount"]
    region_totals[r["region"]] += r["amount"]
    month_totals[(r["year"], r["month"])] += r["amount"]
    grand_total += r["amount"]

# Build monthly × regional matrix
months_sorted = sorted(set((r["year"], r["month"]) for r in valid_rows))
regions_sorted = sorted(set(r["region"] for r in valid_rows))

matrix = {}
for ym in months_sorted:
    for reg in regions_sorted:
        matrix[f"{ym[0]:04d}-{ym[1]:02d}_{reg}"] = agg.get((ym[0], ym[1], reg), 0.0)

# ---------------------------------------------------------------------------
# 4. Build results.json
# ---------------------------------------------------------------------------
# Claims: grand total, per-region totals, per-month totals
claims = {}
claims["grand_total"] = grand_total
for reg in regions_sorted:
    claims[f"region_{reg}"] = region_totals[reg]
for ym in months_sorted:
    claims[f"month_{ym[0]:04d}-{ym[1]:02d}"] = month_totals[ym]

results = {
    "reconciliation": {
        "input_rows": input_rows,
        "used_rows": used_rows,
        "excluded": excluded,
    },
    "values": claims,
}

with open(RESULTS_JSON, "w", encoding="utf-8") as f:
    json.dump(results, f, indent=2, ensure_ascii=False)

# ---------------------------------------------------------------------------
# 5. Build report.md
# ---------------------------------------------------------------------------
lines = []
lines.append("# 売上サマリーレポート")
lines.append("")
lines.append("## 集計概要")
lines.append("")
lines.append(f"- 入力行数: {input_rows}")
lines.append(f"- 使用行数: {used_rows}")
lines.append(f"- 除外行数: {input_rows - used_rows}")
lines.append("")
lines.append("### 除外理由")
lines.append("")
for e in excluded:
    lines.append(f"- {e['reason']}: {e['rows']} 行")
lines.append("")

lines.append("### 月次 × 地域別売上")
lines.append("")
# Header
header = "| 月 | " + " | ".join(regions_sorted) + " | 月合計 |"
sep = "| --- | " + " | ".join(["---" for _ in regions_sorted]) + " | --- |"
lines.append(header)
lines.append(sep)

for ym in months_sorted:
    row_vals = []
    total = 0.0
    for reg in regions_sorted:
        val = agg.get((ym[0], ym[1], reg), 0.0)
        row_vals.append(f"{val:,.2f}")
        total += val
    row_vals.append(f"{total:,.2f}")
    lines.append(f"| {ym[0]:04d}-{ym[1]:02d} | {' | '.join(row_vals)} |")
lines.append("")

lines.append("### 地域別合計")
lines.append("")
for reg in regions_sorted:
    lines.append(f"- {reg}: {region_totals[reg]:,.2f}")
lines.append("")

lines.append(f"**全体合計: {grand_total:,.2f}**")
lines.append("")

with open(REPORT_MD, "w", encoding="utf-8") as f:
    f.write("\n".join(lines))

print("Pipeline completed successfully.")
print(f"  inspection.json: {INSPECTION_JSON}")
print(f"  results.json:    {RESULTS_JSON}")
print(f"  report.md:       {REPORT_MD}")
