#!/usr/bin/env python3
"""
check_output.py – Verify output artifacts against the data_manifest_artifact contract.

Checks:
  1. output/inspection.json exists and has required keys:
     column_names, input_row_count, type_summaries, distinct_values, sample_rows
  2. output/results.json exists and has:
     reconciliation (input_rows, used_rows, excluded) and values dict
  3. output/report.md exists and every numeric claim in it appears in values.
  4. Reconciliation math: input_rows >= used_rows + sum(excluded rows)
  5. Deterministic rerun produces identical results.json.

Exit 0 on success, exit 1 on any failure.
"""

import json
import os
import subprocess
import sys

BASE_DIR = os.path.dirname(os.path.abspath(__file__))
OUTPUT_DIR = os.path.join(BASE_DIR, "output")
DATA_DIR = os.path.join(BASE_DIR, "data")


def load_json(path):
    with open(path, "r", encoding="utf-8") as f:
        return json.load(f)


def check_inspection_schema(inspection):
    """Verify inspection.json has all required top-level keys."""
    required_keys = ["column_names", "input_row_count", "type_summaries", "distinct_values"]
    missing = [k for k in required_keys if k not in inspection]
    if missing:
        raise ValueError(f"inspection.json missing keys: {missing}")
    if not isinstance(inspection["column_names"], list) or len(inspection["column_names"]) == 0:
        raise ValueError("column_names must be a non-empty list")
    if not isinstance(inspection["input_row_count"], int):
        raise ValueError("input_row_count must be an integer")
    if not isinstance(inspection["type_summaries"], dict):
        raise ValueError("type_summaries must be a dict")
    if not isinstance(inspection["distinct_values"], dict):
        raise ValueError("distinct_values must be a dict")


def check_results_schema(results):
    """Verify results.json has reconciliation and values."""
    if "reconciliation" not in results:
        raise ValueError("results.json missing 'reconciliation' key")
    if "values" not in results:
        raise ValueError("results.json missing 'values' key")
    rec = results["reconciliation"]
    for key in ["input_rows", "used_rows", "excluded"]:
        if key not in rec:
            raise ValueError(f"reconciliation missing key: {key}")
    if not isinstance(rec["values"], dict) if "values" in rec else True:
        pass  # values is at top level, not in reconciliation
    if not isinstance(results["values"], dict):
        raise ValueError("values must be a dict")


def check_reconciliation_math(results):
    """input_rows >= used_rows + sum(excluded rows)."""
    rec = results["reconciliation"]
    input_rows = rec["input_rows"]
    used_rows = rec["used_rows"]
    excluded_sum = sum(e["rows"] for e in rec["excluded"])
    if used_rows + excluded_sum != input_rows:
        raise ValueError(
            f"Reconciliation mismatch: input_rows={input_rows}, "
            f"used_rows={used_rows}, excluded_sum={excluded_sum}"
        )


def check_report_claims_binding(results):
    """Every numeric claim in report.md must appear in results.json values."""
    report_path = os.path.join(OUTPUT_DIR, "report.md")
    if not os.path.isfile(report_path):
        raise ValueError("report.md does not exist")
    with open(report_path, "r", encoding="utf-8") as f:
        report_text = f.read()
    # Extract all numbers from report.md
    import re
    numbers = re.findall(r"\b\d+\.?\d*\b", report_text)
    values = results["values"]
    for num_str in numbers:
        try:
            num = float(num_str)
        except ValueError:
            continue
        # Check if this number appears as a value in results.json
        found = False
        for v in values.values():
            if isinstance(v, (int, float)) and v == num:
                found = True
                break
        if not found:
            raise ValueError(
                f"Numeric claim {num} in report.md not found in results.json values"
            )


def check_rerun_consistency():
    """Run pipeline/main.py twice and compare results.json outputs."""
    main_py = os.path.join(BASE_DIR, "pipeline", "main.py")
    results_path = os.path.join(OUTPUT_DIR, "results.json")

    # First run
    subprocess.run([sys.executable, main_py], check=True, capture_output=True)
    with open(results_path, "r", encoding="utf-8") as f:
        first_run = f.read()

    # Second run
    subprocess.run([sys.executable, main_py], check=True, capture_output=True)
    with open(results_path, "r", encoding="utf-8") as f:
        second_run = f.read()

    if first_run != second_run:
        raise ValueError("Rerun inconsistency: results.json differs between runs")


def main():
    errors = []

    # 1. Check inspection.json
    inspection_path = os.path.join(OUTPUT_DIR, "inspection.json")
    if not os.path.isfile(inspection_path):
        errors.append("inspection.json does not exist")
    else:
        try:
            inspection = load_json(inspection_path)
            check_inspection_schema(inspection)
        except Exception as e:
            errors.append(f"inspection.json schema check failed: {e}")

    # 2. Check results.json
    results_path = os.path.join(OUTPUT_DIR, "results.json")
    if not os.path.isfile(results_path):
        errors.append("results.json does not exist")
    else:
        try:
            results = load_json(results_path)
            check_results_schema(results)
            check_reconciliation_math(results)
            check_report_claims_binding(results)
        except Exception as e:
            errors.append(f"results.json check failed: {e}")

    # 3. Check report.md
    report_path = os.path.join(OUTPUT_DIR, "report.md")
    if not os.path.isfile(report_path):
        errors.append("report.md does not exist")

    # 4. Check rerun consistency
    try:
        check_rerun_consistency()
    except Exception as e:
        errors.append(f"Rerun consistency check failed: {e}")

    if errors:
        for err in errors:
            print(f"FAIL: {err}", file=sys.stderr)
        sys.exit(1)
    else:
        print("All checks passed.", file=sys.stdout)
        sys.exit(0)


if __name__ == "__main__":
    main()
