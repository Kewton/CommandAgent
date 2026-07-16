#!/usr/bin/env python3
"""
validate_sales.py
Parse data/sales.csv, filter invalid rows, log reasons, and output:
  - data/sales_clean.csv (valid rows only)
  - data/validation_log.csv (invalid rows with reasons)
"""

import csv
import os
import sys
from datetime import datetime

# ---------------------------------------------------------------------------
# Paths (workspace-relative)
# ---------------------------------------------------------------------------
BASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
INPUT_CSV = os.path.join(BASE_DIR, "data", "sales.csv")
CLEAN_CSV = os.path.join(BASE_DIR, "data", "sales_clean.csv")
LOG_CSV = os.path.join(BASE_DIR, "data", "validation_log.csv")

VALID_REGIONS = {"東京", "大阪", "名古屋"}


def is_valid_date(date_str: str) -> bool:
    """Return True if *date_str* is a valid YYYY-MM-DD date."""
    try:
        datetime.strptime(date_str, "%Y-%m-%d")
        return True
    except (ValueError, TypeError):
        return False


def is_valid_amount(amount_str: str) -> bool:
    """Return True if *amount_str* is a non-empty numeric value."""
    if not amount_str or not amount_str.strip():
        return False
    try:
        float(amount_str)
        return True
    except ValueError:
        return False


def validate_row(row: dict, row_num: int) -> tuple:
    """
    Validate a single row.
    Returns (is_valid: bool, reasons: list[str]).
    """
    reasons = []

    # Check date
    date_val = row.get("date", "").strip()
    if not date_val:
        reasons.append("missing_date")
    elif not is_valid_date(date_val):
        reasons.append("invalid_date")

    # Check region
    region_val = row.get("region", "").strip()
    if not region_val:
        reasons.append("missing_region")
    elif region_val not in VALID_REGIONS:
        reasons.append(f"unknown_region:{region_val}")

    # Check amount
    amount_val = row.get("amount", "").strip()
    if not amount_val:
        reasons.append("missing_amount")
    elif not is_valid_amount(amount_val):
        reasons.append("invalid_amount")

    return (len(reasons) == 0), reasons


def main():
    if not os.path.isfile(INPUT_CSV):
        print(f"ERROR: Input file not found: {INPUT_CSV}", file=sys.stderr)
        sys.exit(1)

    valid_rows = []
    invalid_rows = []

    with open(INPUT_CSV, newline="", encoding="utf-8") as fin:
        reader = csv.DictReader(fin)
        for row_num, row in enumerate(reader, start=2):  # row 1 is header
            is_valid, reasons = validate_row(row, row_num)
            if is_valid:
                valid_rows.append(row)
            else:
                invalid_rows.append(
                    {
                        "row_num": row_num,
                        "date": row.get("date", ""),
                        "region": row.get("region", ""),
                        "amount": row.get("amount", ""),
                        "reasons": "; ".join(reasons),
                    }
                )

    # Write clean CSV (preserve original column order)
    fieldnames = ["date", "region", "amount"]
    with open(CLEAN_CSV, "w", newline="", encoding="utf-8") as fout:
        writer = csv.DictWriter(fout, fieldnames=fieldnames)
        writer.writeheader()
        for row in valid_rows:
            writer.writerow(row)

    # Write validation log
    log_fieldnames = ["row_num", "date", "region", "amount", "reasons"]
    with open(LOG_CSV, "w", newline="", encoding="utf-8") as fout:
        writer = csv.DictWriter(fout, fieldnames=log_fieldnames)
        writer.writeheader()
        for row in invalid_rows:
            writer.writerow(row)

    # Summary to stdout
    total = len(valid_rows) + len(invalid_rows)
    print(f"Total rows (excl. header): {total}")
    print(f"Valid rows: {len(valid_rows)}")
    print(f"Invalid rows: {len(invalid_rows)}")
    if invalid_rows:
        print("\nInvalid rows detail:")
        for row in invalid_rows:
            print(
                f"  Row {row['row_num']}: "
                f"date={row['date']!r} region={row['region']!r} "
                f"amount={row['amount']!r} reasons={row['reasons']}"
            )

    print(f"\nClean CSV written: {CLEAN_CSV}")
    print(f"Validation log written: {LOG_CSV}")


if __name__ == "__main__":
    main()
