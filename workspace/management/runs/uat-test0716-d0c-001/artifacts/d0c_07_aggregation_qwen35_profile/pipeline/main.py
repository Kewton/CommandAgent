#!/usr/bin/env python3
"""Pipeline script: reads data/sales.csv, inspects it, and writes output/inspection.json."""

import csv
import json
import os
import sys
from datetime import datetime

INPUT_FILE = os.path.join("data", "sales.csv")
OUTPUT_FILE = os.path.join("output", "inspection.json")


def main():
    os.makedirs("output", exist_ok=True)

    column_names = []
    input_row_count = 0
    type_summaries = {}
    distinct_values = {}
    sample_rows = []

    # Read all rows
    with open(INPUT_FILE, "r", encoding="utf-8") as f:
        reader = csv.DictReader(f)
        column_names = list(reader.fieldnames)
        rows = list(reader)

    input_row_count = len(rows)

    # Determine types and collect distinct values for categorical columns
    observed_types = {}
    for col in column_names:
        observed_types[col] = set()
        for row in rows:
            val = row.get(col, "")
            if val == "":
                observed_types[col].add("empty")
            elif val.isdigit() or (val.startswith("-") and val[1:].isdigit()):
                observed_types[col].add("number")
            else:
                try:
                    datetime.strptime(val, "%Y-%m-%d")
                    observed_types[col].add("date")
                except ValueError:
                    observed_types[col].add("string")

    # Build type_summaries
    for col in column_names:
        types = observed_types[col]
        if "date" in types:
            type_summaries[col] = "date"
        elif "number" in types:
            type_summaries[col] = "number"
        elif "empty" in types and len(types) == 1:
            type_summaries[col] = "string"
        else:
            type_summaries[col] = "string"

    # Collect distinct values for categorical columns (string type)
    for col in column_names:
        if type_summaries.get(col) == "string":
            distinct_vals = set()
            for row in rows:
                val = row.get(col, "")
                if val != "":
                    distinct_vals.add(val)
            distinct_values[col] = sorted(distinct_vals)

    # Collect sample rows (first 3 rows)
    for i, row in enumerate(rows[:3]):
        sample_row = {}
        for col in column_names:
            val = row.get(col, "")
            if val == "":
                sample_row[col] = ""
            elif val.isdigit():
                sample_row[col] = int(val)
            else:
                sample_row[col] = val
        sample_rows.append(sample_row)

    inspection = {
        "column_names": column_names,
        "input_row_count": input_row_count,
        "type_summaries": type_summaries,
        "distinct_values": distinct_values,
        "sample_rows": sample_rows,
    }

    with open(OUTPUT_FILE, "w", encoding="utf-8") as f:
        json.dump(inspection, f, ensure_ascii=False, indent=2)

    print(f"inspection.json written with {input_row_count} rows, columns: {column_names}")


if __name__ == "__main__":
    main()
