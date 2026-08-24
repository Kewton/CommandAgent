import csv
import json
import statistics
from collections import defaultdict

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

    # We assume columns are 'date', 'region', 'amount' based on the requirement "月次×地域"
    # If column names differ, this script will identify them as missing/invalid.
    for row in rows:
        if not row.get('date') or not row.get('region') or not row.get('amount'):
            excluded['missing_fields'] += 1
            continue
        
        try:
            amount = float(row['amount'])
            # Basic date validation (YYYY-MM-DD)
            date_val = row['date']
            if len(date_val) < 7 or date_val[4] != '-' or date_val[7] != '-':
                raise ValueError("Invalid date format")
            
            month = date_val[:7] # YYYY-MM
            used_rows.append({
                'month': month,
                'region': row['region'],
                'amount': amount
            })
        except ValueError:
            excluded['invalid_types'] += 1

    # Aggregation
    regional_monthly_sales = defaultdict(float)
    total_sales = 0.0
    
    for row in used_rows:
        key = f"{row['month']} | {row['region']}"
        regional_monthly_sales[key] += row['amount']
        total_sales += row['amount']

    # Deterministic sorting for results
    sorted_keys = sorted(regional_monthly_sales.keys())
    values = {}
    for k in sorted_keys:
        values[k] = regional_monthly_sales[k]
    
    values['total_sales'] = total_sales

    # Results JSON
    results = {
        "reconciliation": {
            "input_rows": input_rows_count,
            "used_rows": len(used_rows),
            "excluded": [{"reason": reason, "rows": count} for reason, count in sorted(excluded.items())]
        },
        "values": values
    }

    with open(results_path, 'w', encoding='utf-8') as f:
        json.dump(results, f, indent=2)

    # Inspection JSON (describing the data observed)
    inspection = {
        "columns": list(rows[0].keys()) if rows else [],
        "sample_count": len(rows),
        "validation_rules": {
            "amount": "must be float",
            "date": "must be YYYY-MM-DD",
            "region": "must be present"
        }
    }
    with open(inspection_path, 'w', encoding='utf-8') as f:
        json.dump(inspection, f, indent=2)

    # Report MD
    report = []
    report.append("# Sales Summary Report\n")
    report.append("## Data Reconciliation")
    report.append(f"- Total input rows: {input_rows_count}")
    report.append(f"- Used rows: {len(used_rows)}")
    for excl in results["reconciliation"]["excluded"]:
        report.append(f"- Excluded ({excl['reason']}): {excl['rows']}")
    
    report.append("\n## Monthly Regional Sales")
    report.append("| Month | Region | Amount |")
    report.append("|---|---|---|")
    for k in sorted_keys:
        month, region = k.split(' | ')
        report.append(f"| {month} | {region} | {values[k]:.2f} |")
    
    report.append("\n## Total")
    report.append(f"**Total Sales: {total_sales:.2f}**")

    with open(report_path, 'w', encoding='utf-8') as f:
        f.write("\n".join(report))

if __name__ == '__main__':
    main()
