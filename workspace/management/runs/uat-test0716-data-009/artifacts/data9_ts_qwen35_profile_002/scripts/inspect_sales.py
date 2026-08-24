#!/usr/bin/env python3
"""inspect_sales.py — Read data/sales.csv and produce output/inspection.json."""

import csv
import json
import os
import sys
from datetime import datetime


def infer_type(value: str) -> str:
    """Infer the type of a single cell value."""
    v = value.strip()
    if v == "":
        return "empty"
    # Try integer
    try:
        int(v)
        return "integer"
    except ValueError:
        pass
    # Try float
    try:
        float(v)
        return "float"
    except ValueError:
        pass
    # Boolean
    if v.lower() in ("true", "false", "yes", "no"):
        return "boolean"
    # Date-like check
    try:
        datetime.strptime(v, "%Y-%m-%d")
        return "date"
    except ValueError:
        pass
    return "string"


def main():
    base_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    csv_path = os.path.join(base_dir, "data", "sales.csv")
    output_dir = os.path.join(base_dir, "output")
    os.makedirs(output_dir, exist_ok=True)
    inspection_path = os.path.join(output_dir, "inspection.json")

    # Read CSV
    with open(csv_path, "r", encoding="utf-8") as f:
        reader = csv.reader(f)
        header = next(reader)
        column_names = header

        rows = []
        for row in reader:
            rows.append(row)

    input_row_count = len(rows)

    # Determine column count
    num_cols = len(column_names)

    # Collect values per column
    col_values = {name: [] for name in column_names}
    for row in rows:
        for i, name in enumerate(column_names):
            if i < len(row):
                col_values[name].append(row[i].strip())
            else:
                col_values[name].append("")

    # Type summaries per column
    type_summaries = {}
    for name in column_names:
        type_counts = {}
        for v in col_values[name]:
            t = infer_type(v)
            type_counts[t] = type_counts.get(t, 0) + 1
        type_summaries[name] = type_counts

    # Distinct values for non-numeric columns
    distinct_values = {}
    for name in column_names:
        types = type_summaries[name]
        # Check if all non-empty values are numeric
        all_numeric = True
        for v in col_values[name]:
            t = infer_type(v)
            if t not in ("integer", "float", "empty"):
                all_numeric = False
                break
        if not all_numeric:
            distinct_values[name] = sorted(
                set(v for v in col_values[name] if v != "")
            )

    # Sample rows (first 5)
    sample_rows = []
    for row in rows[:5]:
        sample_rows.append(dict(zip(column_names, row)))

    # Build inspection dict
    inspection = {
        "column_names": column_names,
        "input_row_count": input_row_count,
        "type_summaries": type_summaries,
        "distinct_values": distinct_values,
        "sample_rows": sample_rows,
    }

    with open(inspection_path, "w", encoding="utf-8") as f:
        json.dump(inspection, f, indent=2, ensure_ascii=False)

    print(f"Wrote {inspection_path}")
    print(f"  column_names: {column_names}")
    print(f"  input_row_count: {input_row_count}")
    print(f"  type_summaries: {json.dumps(type_summaries, ensure_ascii=False)}")
    print(f"  distinct_values: {json.dumps(distinct_values, ensure_ascii=False)}")
    print(f"  sample_rows: {len(sample_rows)} rows")


if __name__ == "__main__":
    main()
