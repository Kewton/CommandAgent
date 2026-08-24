#!/usr/bin/env python3
"""Sales data pipeline: validate, aggregate, and report.

Reads data/sales.csv, validates rows, aggregates monthly and regional sales,
and generates output/inspection.json, output/results.json, and output/report.md.

Uses only Python 3 standard library (csv, json, statistics).
Deterministic output: fixed ordering, no time-dependent values.
"""

import csv
import json
import os
from datetime import datetime

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

KNOWN_REGIONS = {"東京", "大阪", "名古屋"}
VALID_REGIONS = sorted(KNOWN_REGIONS)  # deterministic order

INPUT_PATH = os.path.join("data", "sales.csv")
OUTPUT_DIR = "output"
INSPECTION_PATH = os.path.join(OUTPUT_DIR, "inspection.json")
RESULTS_PATH = os.path.join(OUTPUT_DIR, "results.json")
REPORT_PATH = os.path.join(OUTPUT_DIR, "report.md")

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _is_valid_date(date_str: str) -> bool:
    """Return True if *date_str* is a valid ISO date (YYYY-MM-DD)."""
    if not date_str or not date_str.strip():
        return False
    try:
        datetime.strptime(date_str.strip(), "%Y-%m-%d")
        return True
    except ValueError:
        return False


def _is_valid_amount(amount_str: str) -> bool:
    """Return True if *amount_str* can be parsed as a float."""
    if not amount_str or not amount_str.strip():
        return False
    try:
        float(amount_str.strip())
        return True
    except ValueError:
        return False


# ---------------------------------------------------------------------------
# Main pipeline
# ---------------------------------------------------------------------------


def run() -> None:
    """Execute the full pipeline."""

    # --- 1. Read & validate ---------------------------------------------------
    rows: list[dict] = []
    excluded_reasons: dict[str, int] = {}  # reason -> count
    total_input_rows = 0

    with open(INPUT_PATH, newline="", encoding="utf-8") as fh:
        reader = csv.DictReader(fh)
        for row in reader:
            total_input_rows += 1
            date_str = row.get("date", "").strip()
            region_str = row.get("region", "").strip()
            amount_str = row.get("amount", "").strip()

            # Validate date
            if not _is_valid_date(date_str):
                excluded_reasons["invalid_date"] = (
                    excluded_reasons.get("invalid_date", 0) + 1
                )
                continue

            # Validate region
            if region_str not in KNOWN_REGIONS:
                excluded_reasons["unknown_region"] = (
                    excluded_reasons.get("unknown_region", 0) + 1
                )
                continue

            # Validate amount
            if not _is_valid_amount(amount_str):
                excluded_reasons["invalid_amount"] = (
                    excluded_reasons.get("invalid_amount", 0) + 1
                )
                continue

            # Row is valid – store parsed values
            parsed_date = datetime.strptime(date_str, "%Y-%m-%d")
            rows.append(
                {
                    "date": parsed_date,
                    "year": parsed_date.year,
                    "month": parsed_date.month,
                    "region": region_str,
                    "amount": float(amount_str),
                }
            )

    used_rows = len(rows)

    # --- 2. Aggregation -------------------------------------------------------
    # Monthly totals:  {"YYYY-MM": total_amount}
    monthly_totals: dict[str, float] = {}
    # Regional totals: {"region": total_amount}
    regional_totals: dict[str, float] = {}
    # Monthly × Regional: {"YYYY-MM": {"region": total}}
    monthly_region: dict[str, dict[str, float]] = {}

    for r in rows:
        ym = f"{r['year']:04d}-{r['month']:02d}"
        region = r["region"]
        amount = r["amount"]

        # Monthly
        monthly_totals[ym] = monthly_totals.get(ym, 0.0) + amount

        # Regional
        regional_totals[region] = regional_totals.get(region, 0.0) + amount

        # Monthly × Regional
        if ym not in monthly_region:
            monthly_region[ym] = {}
        monthly_region[ym][region] = (
            monthly_region[ym].get(region, 0.0) + amount
        )

    grand_total = sum(monthly_totals.values())

    # --- 3. Build inspection.json ---------------------------------------------
    # Observation-based: collect unique regions and months seen in valid data
    observed_regions = sorted(set(r["region"] for r in rows))
    observed_months = sorted(
        set(f"{r['year']:04d}-{r['month']:02d}" for r in rows)
    )

    inspection = {
        "columns": ["date", "region", "amount"],
        "region_values": observed_regions,
        "month_values": observed_months,
        "total_input_rows": total_input_rows,
        "valid_rows": used_rows,
        "excluded": [
            {"reason": reason, "rows": count}
            for reason, count in sorted(excluded_reasons.items())
        ],
    }

    # --- 4. Build results.json -----------------------------------------------
    # Deterministic key ordering
    sorted_months = sorted(monthly_totals.keys())
    sorted_regions = sorted(regional_totals.keys())

    monthly_data = {}
    for ym in sorted_months:
        monthly_data[ym] = {
            "total": round(monthly_totals[ym], 2),
            "regions": {
                reg: round(monthly_region[ym].get(reg, 0.0), 2)
                for reg in sorted_regions
            },
        }

    regional_data = {
        reg: round(regional_totals[reg], 2) for reg in sorted_regions
    }

    results = {
        "reconciliation": {
            "input_rows": total_input_rows,
            "used_rows": used_rows,
            "excluded": [
                {"reason": reason, "rows": count}
                for reason, count in sorted(excluded_reasons.items())
            ],
        },
        "values": {
            "grand_total": round(grand_total, 2),
            **{f"total_{reg}": regional_totals[reg] for reg in sorted_regions},
            **{f"total_{ym}": monthly_totals[ym] for ym in sorted_months},
        },
        "monthly": monthly_data,
        "regional": regional_data,
    }

    # --- 5. Write outputs -----------------------------------------------------
    os.makedirs(OUTPUT_DIR, exist_ok=True)

    with open(INSPECTION_PATH, "w", encoding="utf-8") as fh:
        json.dump(inspection, fh, indent=2, ensure_ascii=False)
        fh.write("\n")

    with open(RESULTS_PATH, "w", encoding="utf-8") as fh:
        json.dump(results, fh, indent=2, ensure_ascii=False)
        fh.write("\n")

    # --- 6. Write report.md ---------------------------------------------------
    lines: list[str] = []
    lines.append("# Sales Summary Report")
    lines.append("")
    lines.append("## Reconciliation")
    lines.append("")
    lines.append(
        f"- Input rows: **{total_input_rows}**"
    )
    lines.append(
        f"- Used rows: **{used_rows}**"
    )
    for exc in inspection["excluded"]:
        lines.append(
            f"- Excluded ({exc['reason']}): **{exc['rows']}** rows"
        )
    lines.append("")

    lines.append("## Monthly Summary")
    lines.append("")
    lines.append("| Month | Total |")
    lines.append("|-------|-------|")
    for ym in sorted_months:
        lines.append(f"| {ym} | {monthly_totals[ym]:.2f} |")
    lines.append(f"| **Total** | **{grand_total:.2f}** |")
    lines.append("")

    lines.append("## Regional Summary")
    lines.append("")
    lines.append("| Region | Total |")
    lines.append("|--------|-------|")
    for reg in sorted_regions:
        lines.append(f"| {reg} | {regional_totals[reg]:.2f} |")
    lines.append(f"| **Total** | **{grand_total:.2f}** |")
    lines.append("")

    lines.append("## Monthly × Regional Breakdown")
    lines.append("")
    for ym in sorted_months:
        lines.append(f"### {ym}")
        lines.append("")
        lines.append("| Region | Amount |")
        lines.append("|--------|--------|")
        for reg in sorted_regions:
            amt = monthly_region[ym].get(reg, 0.0)
            lines.append(f"| {reg} | {amt:.2f} |")
        lines.append("")

    with open(REPORT_PATH, "w", encoding="utf-8") as fh:
        fh.write("\n".join(lines))
        fh.write("\n")

    print(f"Pipeline complete: {used_rows}/{total_input_rows} rows used.")
    print(f"  Grand total: {grand_total:.2f}")
    print(f"  Outputs: {INSPECTION_PATH}, {RESULTS_PATH}, {REPORT_PATH}")


if __name__ == "__main__":
    run()
