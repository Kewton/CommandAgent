#!/usr/bin/env python3
"""main.py — Monthly sales pipeline: totals, MoM %, 3-month moving avg."""
import csv
import json
import os
from datetime import datetime
from collections import defaultdict

BASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DATA_FILE = os.path.join(BASE_DIR, "data", "sales.csv")
OUTPUT_DIR = os.path.join(BASE_DIR, "output")
RESULTS_FILE = os.path.join(OUTPUT_DIR, "results.json")
REPORT_FILE = os.path.join(OUTPUT_DIR, "report.md")


def parse_date(date_str):
    """Return a datetime.date or None if invalid."""
    date_str = date_str.strip()
    if not date_str:
        return None
    try:
        return datetime.strptime(date_str, "%Y-%m-%d").date()
    except ValueError:
        return None


def main():
    os.makedirs(OUTPUT_DIR, exist_ok=True)

    # --- Read and validate ---
    valid_rows = []
    exclusion_reasons = defaultdict(int)
    input_row_count = 0

    with open(DATA_FILE, newline="", encoding="utf-8") as f:
        reader = csv.DictReader(f)
        for row in reader:
            input_row_count += 1
            date_raw = row.get("date", "").strip()
            region = row.get("region", "").strip()
            amount_raw = row.get("amount", "").strip()

            # Check date validity
            if not date_raw:
                exclusion_reasons["empty_date"] += 1
                continue
            parsed_date = parse_date(date_raw)
            if parsed_date is None:
                exclusion_reasons["invalid_date"] += 1
                continue

            # Check amount validity
            if not amount_raw:
                exclusion_reasons["empty_amount"] += 1
                continue
            try:
                amount = float(amount_raw)
            except ValueError:
                exclusion_reasons["invalid_amount"] += 1
                continue

            valid_rows.append({
                "date": parsed_date,
                "region": region,
                "amount": amount,
            })

    used_row_count = len(valid_rows)

    # --- Group by month ---
    monthly = defaultdict(lambda: {"total": 0.0, "regions": defaultdict(float)})
    for r in valid_rows:
        month_key = r["date"].strftime("%Y-%m")
        monthly[month_key]["total"] += r["amount"]
        monthly[month_key]["regions"][r["region"]] += r["amount"]

    sorted_months = sorted(monthly.keys())

    # --- Compute monthly totals ---
    monthly_totals = {}
    for m in sorted_months:
        monthly_totals[m] = round(monthly[m]["total"], 2)

    # --- Compute MoM % ---
    mom_percent = {}
    for i, m in enumerate(sorted_months):
        if i == 0:
            mom_percent[m] = None
        else:
            prev = monthly_totals[sorted_months[i - 1]]
            curr = monthly_totals[m]
            if prev == 0:
                mom_percent[m] = None
            else:
                mom_percent[m] = round(((curr - prev) / prev) * 100, 2)

    # --- Compute 3-month moving average ---
    moving_avg = {}
    for i, m in enumerate(sorted_months):
        window = []
        for j in range(max(0, i - 2), i + 1):
            window.append(monthly_totals[sorted_months[j]])
        moving_avg[m] = round(sum(window) / len(window), 2)

    # --- Region-level monthly totals ---
    region_monthly = defaultdict(lambda: defaultdict(float))
    for r in valid_rows:
        month_key = r["date"].strftime("%Y-%m")
        region_monthly[r["region"]][month_key] += r["amount"]

    region_monthly_totals = {}
    for region in sorted(region_monthly.keys()):
        region_monthly_totals[region] = {}
        for m in sorted_months:
            region_monthly_totals[region][m] = round(region_monthly[region][m], 2)

    # --- Build results.json ---
    results = {
        "reconciliation": {
            "input_rows": input_row_count,
            "used_rows": used_row_count,
            "excluded": [
                {"reason": reason, "rows": count}
                for reason, count in sorted(exclusion_reasons.items())
            ],
        },
        "values": {
            "monthly_totals": monthly_totals,
            "mom_percent": mom_percent,
            "moving_avg": moving_avg,
            "region_monthly_totals": region_monthly_totals,
            "total_valid_amount": round(sum(monthly_totals.values()), 2),
            "avg_monthly_total": round(
                sum(monthly_totals.values()) / len(monthly_totals), 2
            )
            if monthly_totals
            else 0,
            "max_monthly_total": round(max(monthly_totals.values()), 2)
            if monthly_totals
            else 0,
            "min_monthly_total": round(min(monthly_totals.values()), 2)
            if monthly_totals
            else 0,
            "num_months": len(monthly_totals),
            "num_regions": len(region_monthly_totals),
        },
    }

    with open(RESULTS_FILE, "w", encoding="utf-8") as f:
        json.dump(results, f, indent=2, ensure_ascii=False)

    # --- Build report.md ---
    lines = []
    lines.append("# 月次売上レポート")
    lines.append("")
    lines.append("## 集計概要")
    lines.append("")
    lines.append(f"- 入力行数: {input_row_count}")
    lines.append(f"- 使用行数: {used_row_count}")
    lines.append(f"- 除外行数: {input_row_count - used_row_count}")
    lines.append("")
    lines.append("### 除外理由")
    lines.append("")
    for ex in results["reconciliation"]["excluded"]:
        lines.append(f"- {ex['reason']}: {ex['rows']}件")
    lines.append("")

    lines.append("## 月次売上合計")
    lines.append("")
    lines.append("| 月 | 売上合計 | 前月比(%) | 3ヶ月移動平均 |")
    lines.append("|---|---------|----------|-------------|")
    for m in sorted_months:
        total = monthly_totals[m]
        mom = mom_percent[m]
        avg = moving_avg[m]
        mom_str = f"{mom:.2f}%" if mom is not None else "—"
        lines.append(f"| {m} | {total:.2f} | {mom_str} | {avg:.2f} |")
    lines.append("")

    lines.append("## 主要指標")
    lines.append("")
    lines.append(f"- 合計売上: {results['values']['total_valid_amount']:.2f}")
    lines.append(f"- 月平均売上: {results['values']['avg_monthly_total']:.2f}")
    lines.append(f"- 最大月売上: {results['values']['max_monthly_total']:.2f}")
    lines.append(f"- 最小月売上: {results['values']['min_monthly_total']:.2f}")
    lines.append(f"- 月数: {results['values']['num_months']}")
    lines.append(f"- 地域数: {results['values']['num_regions']}")
    lines.append("")

    lines.append("## 地域別月次売上")
    lines.append("")
    for region in sorted(region_monthly_totals.keys()):
        lines.append(f"### {region}")
        lines.append("")
        lines.append("| 月 | 売上 |")
        lines.append("|---|-----|")
        for m in sorted_months:
            val = region_monthly_totals[region][m]
            lines.append(f"| {m} | {val:.2f} |")
        lines.append("")

    lines.append("---")
    lines.append(f"*生成日: 2026-07-16*")

    with open(REPORT_FILE, "w", encoding="utf-8") as f:
        f.write("\n".join(lines) + "\n")

    print(f"Results written to {RESULTS_FILE}")
    print(f"Report written to {REPORT_FILE}")


if __name__ == "__main__":
    main()
