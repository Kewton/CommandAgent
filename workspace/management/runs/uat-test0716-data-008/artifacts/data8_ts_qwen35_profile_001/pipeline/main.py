#!/usr/bin/env python3
"""
pipeline/main.py — Data inspection script.

Reads data/sales.csv, infers column types, identifies categorical columns,
computes distinct values, and writes output/inspection.json.

Uses only Python 3 standard library modules.
"""

import csv
import json
import os
import sys
from collections import OrderedDict


def infer_column_type(values):
    """Infer the type of a column from its sample values."""
    if not values:
        return "string"
    
    all_numeric = True
    all_string = True
    
    for v in values:
        if v is None or v == "":
            continue
        try:
            float(v)
        except (ValueError, TypeError):
            all_numeric = False
            break
    
    if all_numeric:
        return "number"
    else:
        return "string"


def main():
    # Determine paths
    base_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    data_path = os.path.join(base_dir, "data", "sales.csv")
    output_dir = os.path.join(base_dir, "output")
    inspection_path = os.path.join(output_dir, "inspection.json")
    
    # Ensure output directory exists
    os.makedirs(output_dir, exist_ok=True)
    
    # Read CSV
    with open(data_path, "r", encoding="utf-8") as f:
        reader = csv.DictReader(f)
        fieldnames = reader.fieldnames
        
        # Collect all rows
        rows = []
        for row in reader:
            rows.append(row)
    
    input_row_count = len(rows)
    
    # Infer types and compute distinct values
    column_names = list(fieldnames)
    type_summaries = OrderedDict()
    distinct_values = OrderedDict()
    
    # Collect values per column
    col_values = {col: [] for col in fieldnames}
    for row in rows:
        for col in fieldnames:
            col_values[col].append(row.get(col, ""))
    
    for col in fieldnames:
        values = col_values[col]
        # Filter out empty strings for type inference
        non_empty = [v for v in values if v is not None and v.strip() != ""]
        col_type = infer_column_type(non_empty)
        type_summaries[col] = col_type
        
        # For string columns, collect distinct values (sorted for determinism)
        if col_type == "string":
            distinct_set = set()
            for v in values:
                if v is not None and v.strip() != "":
                    distinct_set.add(v)
            distinct_values[col] = sorted(list(distinct_set))
    
    # Build sample rows (first 3 rows)
    sample_rows = []
    for i, row in enumerate(rows[:3]):
        sample_row = OrderedDict()
        for col in fieldnames:
            val = row.get(col, "")
            # Convert to appropriate type for JSON
            if type_summaries[col] == "number":
                try:
                    val = float(val)
                except (ValueError, TypeError):
                    val = val
            sample_row[col] = val
        sample_rows.append(sample_row)
    
    # Build inspection result
    inspection = OrderedDict()
    inspection["column_names"] = column_names
    inspection["input_row_count"] = input_row_count
    inspection["type_summaries"] = type_summaries
    inspection["distinct_values"] = distinct_values
    inspection["sample_rows"] = sample_rows
    
    # Write inspection.json
    with open(inspection_path, "w", encoding="utf-8") as f:
        json.dump(inspection, f, indent=2, ensure_ascii=False)
        f.write("\n")
    
    print(f"inspection.json written: {input_row_count} rows, columns={column_names}")


if __name__ == "__main__":
    main()
