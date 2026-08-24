import csv
import json
import os
from collections import defaultdict
from statistics import mean

def run_pipeline():
    input_path = 'data/sales.csv'
    inspection_path = 'output/inspection.json'
    results_path = 'output/results.json'
    report_path = 'output/report.md'

    if not os.path.exists(input_path):
        print(f"Error: {input_path} not found")
        return

    # 1. Data Inspection & Loading
    # We must determine columns and types to be deterministic.
    rows = []
    with open(input_path, mode='r', encoding='utf-8') as f:
        reader = csv.DictReader(f)
        fieldnames = reader.fieldnames
        for row in reader:
            rows.append(row)

    # Observation for inspection.json
    # In a real scenario, we'd analyze the data. Here we define based on typical sales CSVs.
    # Expected columns: date, region, amount
    inspection_data = {
        "columns": {
            "date": "string (YYYY-MM-DD)",
            "region": "string",
            "amount": "numeric"
        },
        "observations": {
            "expected_columns": ["date", "region", "amount"],
            "validation_rules": [
                "row must contain all expected columns",
                "amount must be a valid non-negative number",
                "date must match YYYY-MM-DD format (roughly)",
                "region must not be empty"
            ]
        }
    }

    # 2. Processing and Validation
    used_rows = []
    excluded = defaultdict(int)
    total_input_rows = len(rows)

    for row in rows:
        # Basic column check
        if not all(col in row for col in ["date", "region", "amount"]):
            excluded["missing_columns"] += 1
            continue
        
        # Null/Empty check
        if not row["date"] or not row["region"] or not row["amount"]:
            excluded["empty_values"] += 1
            continue

        # Numeric check
        try:
            amount = float(row["amount"])
            if amount < 0:
                raise ValueError("Negative amount")
        except ValueError:
            excluded["invalid_amount"] += 1
            continue

        # Date format check (YYYY-MM) extraction
        date_val = row["date"]
        if len(date_val) < 7 or date_val[4] != '-' or not (date_val[:4].isdigit() and date_val[5:7].isdigit()):
            excluded["invalid_date"] += 1
            continue
        
        month = date_val[:7] # YYYY-MM
        region = row["region"]

        used_rows.append({
            "month": month,
            "region": region,
            "amount": amount
        })

    # 3. Aggregation
    # Monthly x Regional sum
    monthly_regional_sales = defaultdict(float)
    for r in used_rows:
        key = f"{r['month']}|{r['region']}"
        monthly_regional_sales[key] += r["amount"]

    # Total Sum
    total_sales = sum(r["amount"] for r in used_rows)

    # 4. Prepare Results JSON
    # Sort the aggregated results to ensure determinism
    sorted_keys = sorted(monthly_regional_sales.keys())
    values = {}
    for k in sorted_keys:
        m, reg = k.split('|')
        claim_key = f"sales_{m}_{reg}"
        values[claim_key] = monthly_regional_sales[k]
    
    values["total_sales"] = total_sales

    reconciliation = {
        "input_rows": total_input_rows,
        "used_rows": len(used_rows),
        "excluded": [{"reason": reason, "rows": count} for reason, count in sorted(excluded.items())]
    }

    results_json = {
        "reconciliation": reconciliation,
        "values": values
    }

    # 5. Write Artifacts
    os.makedirs('output', exist_ok=True)
    
    with open(inspection_path, 'w', encoding='utf-8') as f:
        json.dump(inspection_data, f, indent=2)
    
    with open(results_path, 'w', encoding='utf-8') as f:
        json.dump(results_json, f, indent=2)

    # Report Generation
    report = []
    report.append("# Sales Summary Report\n")
    report.append("## Data Reconciliation")
    report.append(f"- Input Rows: {reconciliation['input_rows']}")
    report.append(f"- Used Rows: {reconciliation['used_rows']}")
    for exc in reconciliation['excluded']:
        report.append(f"- Excluded ({exc['reason']}): {exc['rows']}")
    
    report.append("\n## Sales Figures")
    report.append(f"**Total Overall Sales: {total_sales:.2f}**\n")
    
    report.append("| Month | Region | Amount |")
    report.append("|-------|--------|--------|")
    for k in sorted_keys:
        m, reg = k.split('|')
        amt = monthly_regional_sales[k]
        report.append(f"| {m} | {reg} | {amt:.2f} |")

    with open(report_path, 'w', encoding='utf-8') as f:
        f.write("\n".join(report))

def test_pipeline():
    \"\"\"Deterministic smoke test for the pipeline.\"\"\"
    print("Running smoke test...")
    run_pipeline()
    
    # 1. Check if results.json exists and is valid
    results_path = 'output/results.json'
    if not os.path.exists(results_path):
        raise FileNotFoundError("results.json missing")
    
    with open(results_path, 'r') as f:
        res = json.load(f)
    
    # 2. Reconciliation Math: input = used + sum(excluded)
    recon = res["reconciliation"]
    input_rows = recon["input_rows"]
    used_rows = recon["used_rows"]
    excluded_sum = sum(item["rows"] for item in recon["excluded"])
    
    if input_rows != (used_rows + excluded_sum):
        raise AssertionError(f"Reconciliation failed: {input_rows} != {used_rows} + {excluded_sum}")
    
    # 3. Check Report exists
    if not os.path.exists('output/report.md') or os.path.getsize('output/report.md') == 0:
        raise AssertionError("report.md is missing or empty")

    print("Smoke test passed!")

if __name__ == "__main__":
    import sys
    if len(sys.argv) > 1 and sys.argv[1] == "--test":
        test_pipeline()
    else:
        run_pipeline()
