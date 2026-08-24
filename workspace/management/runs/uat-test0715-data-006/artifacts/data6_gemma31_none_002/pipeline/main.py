import csv
import json
from collections import defaultdict
from datetime import datetime

def main():
    input_path = 'data/sales.csv'
    inspection_path = 'output/inspection.json'
    results_path = 'output/results.json'
    report_path = 'output/report.md'

    try:
        with open(input_path, mode='r', encoding='utf-8') as f:
            reader = csv.DictReader(f)
            rows = list(reader)
    except FileNotFoundError:
        print(f"Error: {input_path} not found.")
        return

    input_rows_count = len(rows)
    used_rows = []
    excluded = defaultdict(int)
    
    # Data types for inspection
    types_observed = {"Date": set(), "Region": set(), "Amount": set()}
    
    for row in rows:
        date_val = row.get('Date')
        region_val = row.get('Region')
        amount_val = row.get('Amount')

        # Validation: Missing values
        if not date_val or not region_val or not amount_val:
            excluded['missing_value'] += 1
            continue
        
        # Validation: Date format (YYYY-MM-DD)
        try:
            dt = datetime.strptime(date_val, '%Y-%m-%d')
            month = dt.strftime('%Y-%m')
        except ValueError:
            excluded['invalid_date_format'] += 1
            continue
            
        # Validation: Amount numeric
        try:
            amount = float(amount_val)
        except ValueError:
            excluded['invalid_amount_type'] += 1
            continue

        used_rows.append({
            'Month': month,
            'Region': region_val,
            'Amount': amount
        })
        
        # Collect observations for inspection
        types_observed["Date"].add(type(date_val).__name__)
        types_observed["Region"].add(type(region_val).__name__)
        types_observed["Amount"].add(type(amount_val).__name__)

    # Aggregation
    regional_monthly_sales = defaultdict(float)
    total_sales = 0.0
    
    for row in used_rows:
        key = f"sales_{row['Month']}_{row['Region']}"
        regional_monthly_sales[key] += row['Amount']
        total_sales += row['Amount']

    # Prepare results values
    values = {"total_sales": total_sales}
    # Sort keys for determinism
    for key in sorted(regional_monthly_sales.keys()):
        values[key] = regional_monthly_sales[key]

    # Reconciliation
    reconciliation = {
        "input_rows": input_rows_count,
        "used_rows": len(used_rows),
        "excluded": [{"reason": r, "rows": c} for r, c in sorted(excluded.items())]
    }

    results = {
        "reconciliation": reconciliation,
        "values": values
    }

    # Inspection
    inspection = {
        "columns": ["Date", "Region", "Amount"],
        "types": {k: list(v) for k, v in types_observed.items()},
        "row_count": input_rows_count
    }

    # Write artifacts
    with open(inspection_path, 'w', encoding='utf-8') as f:
        json.dump(inspection, f, indent=2)

    with open(results_path, 'w', encoding='utf-8') as f:
        json.dump(results, f, indent=2)

    # Generate Report
    report = []
    report.append("# Sales Summary Report")
    report.append(f"\n## Reconciliation\n- Total input rows: {reconciliation['input_rows']}")
    report.append(f"- Used rows: {reconciliation['used_rows']}")
    for item in reconciliation['excluded']:
        report.append(f"- Excluded ({item['reason']}): {item['rows']}")

    report.append("\n## Total Sales\n" + f"{total_sales:,.2f}")
    
    report.append("\n## Regional Monthly Breakdown")
    for key in sorted(values.keys()):
        if key.startswith("sales_"):
            val = values[key]
            report.append(f"- {key}: {val:,.2f}")

    with open(report_path, 'w', encoding='utf-8') as f:
        f.write("\n".join(report))

if __name__ == "__main__":
    main()
