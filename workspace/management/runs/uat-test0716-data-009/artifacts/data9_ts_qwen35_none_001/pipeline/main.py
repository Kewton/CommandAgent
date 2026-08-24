#!/usr/bin/env python3
"""
Sales data pipeline: loads data/sales.csv, validates rows,
computes monthly totals, MoM %, and 3-month moving averages,
then writes output/inspection.json, output/results.json,
and output/report.md.

Uses only Python 3 standard-library modules.
"""

import csv
import json
import os
import statistics
from collections import OrderedDict
from datetime import datetime
from pathlib import Path

# --- Configuration ---
BASE_DIR = Path(__file__).resolve().parent.parent
INPUT_CSV = BASE_DIR / "data" / "sales.csv"
OUTPUT_DIR = BASE_DIR / "output"
OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

# --- Load data ---
raw_rows = []
with open(INPUT_CSV, newline="", encoding="utf-8") as f:
    reader = csv.DictReader(f)
    for row in reader:
        raw_rows.append(row)

input_rows = len(raw_rows)

# --- Validate each row ---
valid_rows = []
exclusions = []

for row in raw_rows:
    date_str = row.get("date", "").strip()
    region = row.get("region", "").strip()
    amount_str = row.get("amount", "").strip()

    # Check for missing date
    if not date_str:
        exclusions.append({"reason": "missing_date", "row": row})
        continue

    # Check for invalid date
    try:
        dt = datetime.strptime(date_str, "%Y-%m-%d")
    except ValueError:
        exclusions.append({"reason": "invalid_date", "row": row})
        continue

    # Check for non-numeric amount
    try:
        amount = float(amount_str)
    except (ValueError, TypeError):
        exclusions.append({"reason": "non_numeric_amount", "row": row})
        continue

    # Check for negative amount
    if amount < 0:
        exclusions.append({"reason": "negative_amount", "row": row})
        continue

    valid_rows.append({
        "date": dt,
        "region": region,
        "amount": amount,
    })

used_rows = len(valid_rows)

# --- Compute monthly totals ---
monthly_data = {}
for v in valid_rows:
    key = v["date"].strftime("%Y-%m")
    if key not in monthly_data:
        monthly_data[key] = 0.0
    monthly_data[key] += v["amount"]

# Sort months
sorted_months = sorted(monthly_data.keys())

# Compute MoM % and 3-month moving average
monthly_results = []
for i, month in enumerate(sorted_months):
    total = monthly_data[month]
    # MoM %
    if i > 0:
        prev_total = monthly_data[sorted_months[i - 1]]
        mom_pct = ((total - prev_total) / prev_total) * 100 if prev_total != 0 else 0.0
    else:
        mom_pct = None  # No prior month

    # 3-month moving average
    if i >= 2:
        window = [monthly_data[sorted_months[j]] for j in range(i - 2, i + 1)]
        ma3 = statistics.mean(window)
    elif i == 1:
        # Only 2 months available, compute average of those 2
        window = [monthly_data[sorted_months[j]] for j in range(0, i + 1)]
        ma3 = statistics.mean(window)
    else:
        # Only 1 month
        ma3 = total

    monthly_results.append({
        "month": month,
        "total": total,
        "mom_pct": round(mom_pct, 2) if mom_pct is not None else None,
        "ma3": round(ma3, 2),
    })

# --- Build inspection.json ---
exclusion_reasons = {}
for ex in exclusions:
    reason = ex["reason"]
    exclusion_reasons[reason] = exclusion_reasons.get(reason, 0) + 1

# Collect observations
date_values = [v["date"] for v in valid_rows]
amount_values = [v["amount"] for v in valid_rows]

date_min = min(date_values).strftime("%Y-%m-%d") if date_values else None
date_max = max(date_values).strftime("%Y-%m-%d") if date_values else None

amt_stats = {}
if amount_values:
    amt_stats = {
        "min": round(min(amount_values), 2),
        "max": round(max(amount_values), 2),
        "mean": round(statistics.mean(amount_values), 2),
        "stdev": round(statistics.pstdev(amount_values), 2) if len(amount_values) > 1 else 0.0,
    }

inspection = OrderedDict([
    ("input_rows", input_rows),
    ("used_rows", used_rows),
    ("excluded_rows", len(exclusions)),
    ("exclusion_reasons", OrderedDict(sorted(exclusion_reasons.items()))),
    ("date_min", date_min),
    ("date_max", date_max),
    ("amount_stats", OrderedDict(amt_stats)),
    ("monthly_totals", monthly_results),
])

# Write inspection.json
with open(OUTPUT_DIR / "inspection.json", "w", encoding="utf-8") as f:
    json.dump(inspection, f, indent=2, ensure_ascii=False)

# --- Build results.json ---
# Compute regional totals for claims binding
regional_totals = {}
for v in valid_rows:
    reg = v["region"]
    regional_totals[reg] = regional_totals.get(reg, 0.0) + v["amount"]

# Build values dict with all claims
values = OrderedDict()
values["total_sales"] = round(sum(monthly_data.values()), 2)
values["avg_monthly_sales"] = round(statistics.mean(monthly_data.values()), 2) if monthly_data else 0.0

for month in sorted_months:
    values[f"monthly_{month}"] = round(monthly_data[month], 2)

for region in sorted(regional_totals.keys()):
    values[f"regional_{region}"] = round(regional_totals[region], 2)

# Add summary statistics
values["valid_rows"] = used_rows
values["excluded_rows"] = len(exclusions)

# Add MoM claims for each month with data
for i, mr in enumerate(monthly_results):
    if mr["mom_pct"] is not None:
        values[f"mom_{mr['month']}"] = mr["mom_pct"]

# Add 3-month MA claims
for mr in monthly_results:
    values[f"ma3_{mr['month']}"] = mr["ma3"]

results = OrderedDict([
    ("reconciliation", OrderedDict([
        ("input_rows", input_rows),
        ("used_rows", used_rows),
        ("excluded", [OrderedDict([("reason", ex["reason"]), ("rows", 1)]) for ex in exclusions]),
    ])),
    ("values", values),
])

# Write results.json
with open(OUTPUT_DIR / "results.json", "w", encoding="utf-8") as f:
    json.dump(results, f, indent=2, ensure_ascii=False)

# --- Build report.md ---
lines = []
lines.append("# Sales Data Pipeline Report")
lines.append("")
lines.append("## Reconciliation")
lines.append("")
lines.append(f"- **Input rows**: {input_rows}")
lines.append(f"- **Used rows**: {used_rows}")
lines.append(f"- **Excluded rows**: {len(exclusions)}")
lines.append("")
lines.append("### Exclusion Reasons")
lines.append("")
if exclusion_reasons:
    for reason, count in sorted(exclusion_reasons.items()):
        lines.append(f"- **{reason}**: {count} rows")
else:
    lines.append("- No exclusions")
lines.append("")
lines.append("## Monthly Summary")
lines.append("")
lines.append("| Month | Total Sales | MoM % | 3-Month MA |")
lines.append("|-------|------------|-------|------------|")
for mr in monthly_results:
    mom_str = f"{mr['mom_pct']:.2f}%" if mr["mom_pct"] is not None else "N/A"
    lines.append(f"| {mr['month']} | {mr['total']:.2f} | {mom_str} | {mr['ma3']:.2f} |")
lines.append("")
lines.append("## Regional Summary")
lines.append("")
for region in sorted(regional_totals.keys()):
    lines.append(f"- **{region}**: {regional_totals[region]:.2f}")
lines.append("")
lines.append("## Key Metrics")
lines.append("")
lines.append(f"- Total Sales: {values['total_sales']:.2f}")
lines.append(f"- Average Monthly Sales: {values['avg_monthly_sales']:.2f}")
lines.append(f"- Valid Rows: {values['valid_rows']}")
lines.append(f"- Excluded Rows: {values['excluded_rows']}")
lines.append("")

report_md = "\n".join(lines)

with open(OUTPUT_DIR / "report.md", "w", encoding="utf-8") as f:
    f.write(report_md)

print(f"Pipeline complete: {input_rows} input rows, {used_rows} used, {len(exclusions)} excluded")
print(f"Output written to {OUTPUT_DIR}")
