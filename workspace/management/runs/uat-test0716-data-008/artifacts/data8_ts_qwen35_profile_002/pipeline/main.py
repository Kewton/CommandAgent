#!/usr/bin/env python3
"""
Sales data pipeline: monthly totals, MoM %, 3-month moving average.
Uses only Python 3 standard library (csv, json, statistics, datetime).
Deterministic: fixed seed, stable ordering.
"""

import csv
import json
import os
import sys
from datetime import datetime
from collections import OrderedDict
from statistics import mean

BASE_DIR = os.path.dirname(os.path.abspath(__file__))
DATA_FILE = os.path.join(BASE_DIR, "data", "sales.csv")
OUTPUT_DIR = os.path.join(BASE_DIR, "output")
INSPECTION_FILE = os.path.join(OUTPUT_DIR, "inspection.json")
RESULTS_FILE = os.path.join(OUTPUT_DIR, "results.json")
REPORT_FILE = os.path.join(OUTPUT_DIR, "report.md")


def load_inspection():
    """Load inspection.json for validation rules."""
    with open(INSPECTION_FILE, "r", encoding="utf-8") as f:
        return json.load(f)


def read_sales_data(filepath):
    """Read CSV and return header + rows."""
    rows = []
    with open(filepath, "r", encoding="utf-8") as f:
        reader = csv.reader(f)
        header = next(reader)
        for row in reader:
            rows.append(row)
    return header, rows


def validate_row(row, header):
    """
    Validate a single row.
    Returns (is_valid, reason) tuple.
    """
    if len(row) < len(header):
        return False, "incomplete_row"
    
    date_str = row[0].strip()
    region = row[1].strip()
    amount_str = row[2].strip()
    
    # Check date is not empty
    if not date_str:
        return False, "missing_date"
    
    # Check date is valid
    try:
        dt = datetime.strptime(date_str, "%Y-%m-%d")
    except ValueError:
        return False, "invalid_date"
    
    # Check region is not empty
    if not region:
        return False, "missing_region"
    
    # Check amount is valid number
    if not amount_str:
        return False, "missing_amount"
    
    try:
        amount = float(amount_str)
        if amount < 0:
            return False, "negative_amount"
    except ValueError:
        return False, "invalid_amount"
    
    return True, None


def extract_month_key(date_str):
    """Extract YYYY-MM from date string."""
    dt = datetime.strptime(date_str.strip(), "%Y-%m-%d")
    return dt.strftime("%Y-%m")


def compute_monthly_totals(valid_rows):
    """Compute monthly totals from valid rows."""
    monthly = {}
    for row in valid_rows:
        date_str = row[0].strip()
        amount = float(row[2].strip())
        month_key = extract_month_key(date_str)
        
        if month_key not in monthly:
            monthly[month_key] = 0.0
        monthly[month_key] += amount
    
    # Sort by month key for deterministic ordering
    sorted_months = sorted(monthly.keys())
    return sorted_months, monthly


def compute_mom_percentages(monthly_totals):
    """Compute month-over-month percentage changes."""
    keys = sorted(monthly_totals.keys())
    mom = {}
    for i, key in enumerate(keys):
        if i == 0:
            mom[key] = None  # No previous month
        else:
            prev = monthly_totals[keys[i-1]]
            curr = monthly_totals[key]
            if prev == 0:
                mom[key] = None
            else:
                mom[key] = round((curr - prev) / prev * 100, 2)
    return mom


def compute_moving_averages(monthly_totals, window=3):
    """Compute N-month moving averages."""
    keys = sorted(monthly_totals.keys())
    ma = {}
    for i, key in enumerate(keys):
        if i < window - 1:
            ma[key] = None
        else:
            values = [monthly_totals[keys[j]] for j in range(i - window + 1, i + 1)]
            ma[key] = round(mean(values), 2)
    return ma


def build_results(valid_rows, invalid_rows, sorted_months, monthly_totals, mom, ma):
    """Build the results.json structure."""
    # Reconciliation
    input_rows = len(valid_rows) + len(invalid_rows)
    used_rows = len(valid_rows)
    excluded = []
    
    # Group invalid rows by reason
    reason_counts = {}
    for _, reason in invalid_rows:
        reason_counts[reason] = reason_counts.get(reason, 0) + 1
    
    for reason, count in sorted(reason_counts.items()):
        excluded.append({"reason": reason, "rows": count})
    
    reconciliation = {
        "input_rows": input_rows,
        "used_rows": used_rows,
        "excluded": excluded
    }
    
    # Values - monthly totals, MoM, moving averages
    values = OrderedDict()
    for month in sorted_months:
        values[f"{month}_total"] = round(monthly_totals[month], 2)
        if mom[month] is not None:
            values[f"{month}_mom_pct"] = mom[month]
        if ma[month] is not None:
            values[f"{month}_ma3"] = ma[month]
    
    # Summary statistics
    all_totals = [monthly_totals[m] for m in sorted_months]
    values["overall_total"] = round(sum(all_totals), 2)
    values["avg_monthly"] = round(mean(all_totals), 2)
    values["max_month"] = max(all_totals, key=lambda x: x)
    values["min_month"] = min(all_totals, key=lambda x: x)
    
    return {
        "reconciliation": reconciliation,
        "values": values
    }


def generate_report(results, sorted_months, monthly_totals, mom, ma):
    """Generate report.md content."""
    lines = []
    lines.append("# Sales Analysis Report")
    lines.append("")
    lines.append("## Data Reconciliation")
    lines.append("")
    rec = results["reconciliation"]
    lines.append(f"- Input rows: {rec['input_rows']}")
    lines.append(f"- Used rows: {rec['used_rows']}")
    lines.append(f"- Excluded rows: {rec['input_rows'] - rec['used_rows']}")
    lines.append("")
    lines.append("### Excluded Rows by Reason")
    lines.append("")
    lines.append("| Reason | Count |")
    lines.append("|--------|-------|")
    for ex in rec["excluded"]:
        lines.append(f"| {ex['reason']} | {ex['rows']} |")
    lines.append("")
    
    lines.append("## Monthly Sales Summary")
    lines.append("")
    lines.append("| Month | Total | MoM % | 3-Month MA |")
    lines.append("|-------|-------|-------|------------|")
    for month in sorted_months:
        total = monthly_totals[month]
        mom_val = mom[month]
        ma_val = ma[month]
        mom_str = f"{mom_val:.2f}%" if mom_val is not None else "N/A"
        ma_str = f"{ma_val:.2f}" if ma_val is not None else "N/A"
        lines.append(f"| {month} | {total:.2f} | {mom_str} | {ma_str} |")
    lines.append("")
    
    lines.append("## Summary Statistics")
    lines.append("")
    vals = results["values"]
    lines.append(f"- **Overall Total**: {vals['overall_total']:.2f}")
    lines.append(f"- **Average Monthly**: {vals['avg_monthly']:.2f}")
    lines.append(f"- **Maximum Monthly**: {vals['max_month']:.2f}")
    lines.append(f"- **Minimum Monthly**: {vals['min_month']:.2f}")
    lines.append("")
    
    return "\n".join(lines)


def main():
    """Main pipeline entry point."""
    os.makedirs(OUTPUT_DIR, exist_ok=True)
    
    # Read data
    header, rows = read_sales_data(DATA_FILE)
    
    # Validate rows
    valid_rows = []
    invalid_rows = []
    for row in rows:
        is_valid, reason = validate_row(row, header)
        if is_valid:
            valid_rows.append(row)
        else:
            invalid_rows.append((row, reason))
    
    # Compute monthly aggregations
    sorted_months, monthly_totals = compute_monthly_totals(valid_rows)
    mom = compute_mom_percentages(monthly_totals)
    ma = compute_moving_averages(monthly_totals)
    
    # Build results
    results = build_results(valid_rows, invalid_rows, sorted_months, monthly_totals, mom, ma)
    
    # Write results.json
    with open(RESULTS_FILE, "w", encoding="utf-8") as f:
        json.dump(results, f, indent=2, ensure_ascii=False)
    
    # Generate and write report
    report = generate_report(results, sorted_months, monthly_totals, mom, ma)
    with open(REPORT_FILE, "w", encoding="utf-8") as f:
        f.write(report)
    
    print(f"Pipeline complete. Valid rows: {len(valid_rows)}, Invalid rows: {len(invalid_rows)}")
    print(f"Results written to {RESULTS_FILE}")
    print(f"Report written to {REPORT_FILE}")
    
    return 0


if __name__ == "__main__":
    sys.exit(main())
