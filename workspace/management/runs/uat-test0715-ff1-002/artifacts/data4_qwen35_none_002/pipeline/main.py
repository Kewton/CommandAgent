#!/usr/bin/env python3
"""
Pipeline: Read data/sales.csv, validate rows, compute monthly x regional
sales aggregates, write inspection.json, results.json, and report.md.
Uses only Python 3 standard library. Deterministic and reproducible.
"""

import csv
import json
import os
import sys
from collections import OrderedDict
from datetime import datetime

# ---------------------------------------------------------------------------
# Paths
# ---------------------------------------------------------------------------
BASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DATA_CSV = os.path.join(BASE_DIR, "data", "sales.csv")
OUTPUT_DIR = os.path.join(BASE_DIR, "output")
INSPECTION_PATH = os.path.join(OUTPUT_DIR, "inspection.json")
RESULTS_PATH = os.path.join(OUTPUT_DIR, "results.json")
REPORT_PATH = os.path.join(OUTPUT_DIR, "report.md")

os.makedirs(OUTPUT_DIR, exist_ok=True)

# ---------------------------------------------------------------------------
# 1. Read CSV
# ---------------------------------------------------------------------------
rows_raw = []
with open(DATA_CSV, newline="", encoding="utf-8") as fh:
    reader = csv.DictReader(fh)
    for row in reader:
        rows_raw.append(row)

input_rows = len(rows_raw)

# ---------------------------------------------------------------------------
# 2. Validate rows
# ---------------------------------------------------------------------------
# Validation rules (derived from data inspection):
#   - date must be a valid ISO date (YYYY-MM-DD)
#   - region must be non-empty
#   - amount must be a valid positive number

valid_rows = []
excluded_reasons: dict[str, int] = {}
inspection_notes: list[str] = []

for row in rows_raw:
    date_str = row.get("date", "").strip()
    region = row.get("region", "").strip()
    amount_str = row.get("amount", "").strip()

    # Check date validity
    if date_str == "":
        reason = "missing_date"
        excluded_reasons[reason] = excluded_reasons.get(reason, 0) + 1
        continue
    try:
        datetime.strptime(date_str, "%Y-%m-%d")
    except ValueError:
        reason = "invalid_date"
        excluded_reasons[reason] = excluded_reasons.get(reason, 0) + 1
        continue

    # Check region
    if region == "":
        reason = "missing_region"
        excluded_reasons[reason] = excluded_reasons.get(reason, 0) + 1
        continue

    # Check amount
    if amount_str == "":
        reason = "missing_amount"
        excluded_reasons[reason] = excluded_reasons.get(reason, 0) + 1
        continue
    try:
        amount = float(amount_str)
    except ValueError:
        reason = "invalid_amount"
        excluded_reasons[reason] = excluded_reasons.get(reason, 0) + 1
        continue
    if amount <= 0:
        reason = "non_positive_amount"
        excluded_reasons[reason] = excluded_reasons.get(reason, 0) + 1
        continue

    valid_rows.append({
        "date": date_str,
        "region": region,
        "amount": amount,
    })

used_rows = len(valid_rows)

# Build inspection notes
for reason, count in sorted(excluded_reasons.items()):
    inspection_notes.append(f"Excluded {count} row(s) due to {reason}")

inspection = {
    "total_input_rows": input_rows,
    "valid_rows": used_rows,
    "excluded_count": sum(excluded_reasons.values()),
    "excluded_reasons": dict(sorted(excluded_reasons.items())),
    "notes": inspection_notes,
}

# ---------------------------------------------------------------------------
# 3. Compute aggregates
# ---------------------------------------------------------------------------
# Monthly totals (key: "YYYY-MM")
monthly_totals: dict[str, float] = {}
# Regional totals (key: region name)
regional_totals: dict[str, float] = {}

for row in valid_rows:
    dt = datetime.strptime(row["date"], "%Y-%m-%d")
    month_key = dt.strftime("%Y-%m")
    monthly_totals[month_key] = monthly_totals.get(month_key, 0.0) + row["amount"]
    regional_totals[row["region"]] = regional_totals.get(row["region"], 0.0) + row["amount"]

total_sales = sum(monthly_totals.values())

# Build ordered values dict for deterministic output
# Sort months chronologically, regions alphabetically
ordered_monthly = OrderedDict(sorted(monthly_totals.items()))
ordered_regional = OrderedDict(sorted(regional_totals.items()))

values: OrderedDict = OrderedDict()
values["total_sales"] = round(total_sales, 2)
for mk, mv in ordered_monthly.items():
    values[f"monthly_{mk}"] = round(mv, 2)
for rk, rv in ordered_regional.items():
    values[f"regional_{rk}"] = round(rv, 2)

# ---------------------------------------------------------------------------
# 4. Write outputs
# ---------------------------------------------------------------------------
# results.json
results = OrderedDict()
results["reconciliation"] = OrderedDict([
    ("input_rows", input_rows),
    ("used_rows", used_rows),
    ("excluded", [
        OrderedDict([("reason", r), ("rows", c)])
        for r, c in sorted(excluded_reasons.items())
    ]),
])
results["values"] = values

with open(RESULTS_PATH, "w", encoding="utf-8") as fh:
    json.dump(results, fh, indent=2, ensure_ascii=False)
    fh.write("\n")

# inspection.json
with open(INSPECTION_PATH, "w", encoding="utf-8") as fh:
    json.dump(inspection, fh, indent=2, ensure_ascii=False)
    fh.write("\n")

# report.md
lines = [
    "# Sales Summary Report",
    "",
    "## Reconciliation",
    "",
    f"- Input rows: {input_rows}",
    f"- Used rows: {used_rows}",
    f"- Excluded rows: {sum(excluded_reasons.values())}",
    "",
]
if excluded_reasons:
    lines.append("### Excluded Row Reasons")
    lines.append("")
    for r, c in sorted(excluded_reasons.items()):
        lines.append(f"- **{r}**: {c} row(s)")
    lines.append("")
lines.append("## Sales Summary")
lines.append("")
lines.append(f"### Total Sales: **{total_sales:.2f}**")
lines.append("")
lines.append("### Monthly Breakdown")
lines.append("")
lines.append("| Month | Total |")
lines.append("|-------|-------|")
for mk, mv in ordered_monthly.items():
    lines.append(f"| {mk} | {mv:.2f} |")
lines.append("")
lines.append("### Regional Breakdown")
lines.append("")
lines.append("| Region | Total |")
lines.append("|--------|-------|")
for rk, rv in ordered_regional.items():
    lines.append(f"| {rk} | {rv:.2f} |")
lines.append("")

with open(REPORT_PATH, "w", encoding="utf-8") as fh:
    fh.write("\n".join(lines))

print(f"Pipeline complete: {used_rows}/{input_rows} rows used.")
print(f"Total sales: {total_sales:.2f}")
print(f"Results written to {RESULTS_PATH}")
print(f"Inspection written to {INSPECTION_PATH}")
print(f"Report written to {REPORT_PATH}")
