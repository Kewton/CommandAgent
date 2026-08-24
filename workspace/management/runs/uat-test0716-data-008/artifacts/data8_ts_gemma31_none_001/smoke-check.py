import json
import csv
import os
import sys

def check_results_json():
    path = 'output/results.json'
    if not os.path.exists(path):
        print(f"Error: {path} missing")
        return False
    try:
        with open(path, 'r') as f:
            data = json.load(f)
    except Exception as e:
        print(f"Error reading {path}: {e}")
        return False

    if 'reconciliation' not in data or 'values' not in data:
        print(f"Error: {path} must contain 'reconciliation' and 'values'")
        return False

    recon = data['reconciliation']
    input_rows = recon.get('input_rows')
    used_rows = recon.get('used_rows')
    excluded = recon.get('excluded', [])

    if input_rows is None or used_rows is None:
        print(f"Error: {path} reconciliation missing row counts")
        return False

    total_excluded = sum(item.get('rows', 0) for item in excluded)
    if input_rows != (used_rows + total_excluded):
        print(f"Error: Reconciliation mismatch: {input_rows} != {used_rows} + {total_excluded}")
        return False

    return True

def check_cleaned_csv():
    path = 'output/cleaned_sales.csv'
    if not os.path.exists(path):
        print(f"Error: {path} missing")
        return False
    try:
        with open(path, 'r', newline='') as f:
            reader = csv.reader(f)
            headers = next(reader)
            if not headers:
                print(f"Error: {path} has no headers")
                return False
            data_rows = list(reader)
            if len(data_rows) == 0:
                print(f"Error: {path} has no data rows")
                return False
    except Exception as e:
        print(f"Error reading {path}: {e}")
        return False
    return True

def check_validation_summary():
    path = 'output/validation_summary.json'
    if not os.path.exists(path):
        print(f"Error: {path} missing")
        return False
    try:
        with open(path, 'r') as f:
            json.load(f)
    except Exception as e:
        print(f"Error parsing {path}: {e}")
        return False
    return True

def check_report_md():
    path = 'output/report.md'
    if not os.path.exists(path):
        print(f"Error: {path} missing")
        return False
    try:
        with open(path, 'r') as f:
            content = f.read()
            if not content or len(content.strip()) < 10:
                print(f"Error: {path} is empty or too short")
                return False
    except Exception as e:
        print(f"Error reading {path}: {e}")
        return False
    return True

def main():
    checks = [
        ('results.json', check_results_json),
        ('cleaned_sales.csv', check_cleaned_csv),
        ('validation_summary.json', check_validation_summary),
        ('report.md', check_report_md),
    ]

    all_passed = True
    for name, func in checks:
        if not func():
            print(f"Check failed for {name}")
            all_passed = False
        else:
            print(f"Check passed for {name}")

    if all_passed:
        print("All smoke checks passed!")
        sys.exit(0)
    else:
        print("Some smoke checks failed.")
        sys.exit(1)

if __name__ == '__main__':
    main()
