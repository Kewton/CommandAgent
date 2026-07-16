import csv
import json
import os
from collections import defaultdict
from statistics import mean

def main():
    input_path = 'data/sales.csv'
    output_dir = 'output'
    os.makedirs(output_dir, exist_ok=True)
    
    cleaned_path = os.path.join(output_dir, 'cleaned.csv')
    inspection_path = os.path.join(output_dir, 'inspection.json')
    results_path = os.path.join(output_dir, 'results.json')
    report_path = os.path.join(output_dir, 'report.md')

    if not os.path.exists(input_path):
        print(f"Error: {input_path} not found")
        return

    # Data Loading and Validation
    input_rows = 0
    valid_rows = []
    excluded = defaultdict(int)
    
    with open(input_path, mode='r', encoding='utf-8') as f:
        reader = csv.DictReader(f)
        for row in reader:
            input_rows += 1
            # Validation rules
            if not row.get('date') or not row.get('amount'):
                excluded['missing_fields'] += 1
                continue
            try:
                # Normalize amount to float
                amount = float(row['amount'])
                # Simple date check (expects YYYY-MM-DD)
                date = row['date']
                if len(date) < 7: # At least YYYY-MM
                    raise ValueError("Invalid date format")
                
                valid_rows.append({'date': date, 'amount': amount})
            except ValueError as e:
                excluded['invalid_types'] += 1
                continue

    # Write cleaned data
    with open(cleaned_path, mode='w', encoding='utf-8', newline='') as f:
        if valid_rows:
            writer = csv.DictWriter(f, fieldnames=['date', 'amount'])
            writer.writeheader()
            writer.writerows(valid_rows)

    # Inspection data for output/inspection.json
    # Note: In a real scenario, we'd analyze the actual distributions here.
    inspection = {
        "column_types": {"date": "string", "amount": "float"},
        "observed_anomalies": list(excluded.keys()),
        "total_input_rows": input_rows,
        "total_valid_rows": len(valid_rows)
    }
    with open(inspection_path, 'w', encoding='utf-8') as f:
        json.dump(inspection, f, indent=2)

    # Aggregation: Monthly Total
    monthly_sales = defaultdict(float)
    for row in valid_rows:
        # Assume date format starts with YYYY-MM
        month = row['date'][:7] 
        monthly_sales[month] += row['amount']

    sorted_months = sorted(monthly_sales.keys())
    monthly_totals = [monthly_sales[m] for m in sorted_months]
    
    # Calculations: MoM and Moving Average
    mom_growth = []
    for i in range(len(monthly_totals)):
        if i == 0:
            mom_growth.append(None)
        else:
            prev = monthly_totals[i-1]
            curr = monthly_totals[i]
            growth = ((curr - prev) / prev * 100) if prev != 0 else 0
            mom_growth.append(growth)

    moving_averages = []
    for i in range(len(monthly_totals)):
        window = monthly_totals[max(0, i-2):i+1]
        moving_averages.append(mean(window))

    # Prepare results.json values
    values = {}
    for i, month in enumerate(sorted_months):
        values[f"sales_{month}"] = monthly_totals[i]
        if mom_growth[i] is not None:
            values[f"mom_{month}"] = mom_growth[i]
        values[f"ma3_{month}"] = moving_averages[i]

    results = {
        "reconciliation": {
            "input_rows": input_rows,
            "used_rows": len(valid_rows),
            "excluded": [{"reason": k, "rows": v} for k, v in sorted(excluded.items())]
        },
        "values": values
    }
    with open(results_path, 'w', encoding='utf-8') as f:
        json.dump(results, f, indent=2)

    # Generate report.md
    report = [
        "# Sales Summary Report",
        "",
        "## Data Reconciliation",
        f"- Total Input Rows: {input_rows}",
        f"- Valid Rows Used: {len(valid_rows)}",
        f"- Excluded Rows: {sum(excluded.values())}",
    ]
    for reason, count in sorted(excluded.items()):
        report.append(f"  - {reason}: {count}")

    report.append("\n## Monthly Metrics")
    report.append("| Month | Total Sales | MoM Growth (%) | 3-Month MA |")
    report.append("|-------|-------------|----------------|------------|")
    for i, month in enumerate(sorted_months):
        mom_val = f"{mom_growth[i]:.2f}%" if mom_growth[i] is not None else "N/A"
        report.append(f"| {month} | {monthly_totals[i]:.2f} | {mom_val} | {moving_averages[i]:.2f} |")

    with open(report_path, 'w', encoding='utf-8') as f:
        f.write("\n".join(report))

if __name__ == "__main__":
    main()
