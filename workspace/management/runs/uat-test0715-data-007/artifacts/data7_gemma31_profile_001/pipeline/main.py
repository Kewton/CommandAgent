import csv
import json
from collections import defaultdict
from statistics import mean

def main():
    input_path = 'data/sales.csv'
    results_path = 'output/results.json'
    report_path = 'output/report.md'
    inspection_path = 'output/inspection.json'

    # Load inspection data to derive validation rules as per contract
    try:
        with open(inspection_path, 'r', encoding='utf-8') as f:
            inspection = json.load(f)
            valid_regions = set(inspection.get('distinct_values', {}).get('region', []))
    except FileNotFoundError:
        # Fallback if inspection is missing, although it should be there from previous steps
        valid_regions = {'東京', '大阪', '名古屋'}

    input_rows_count = 0
    used_rows_count = 0
    excluded = defaultdict(int)
    
    # Store totals: {(month, region): sum}
    sales_data = defaultdict(float)
    grand_total = 0.0

    try:
        with open(input_path, 'r', encoding='utf-8') as f:
            reader = csv.DictReader(f)
            for row in reader:
                input_rows_count += 1
                
                # Validation
                date_val = row.get('date', '')
                region_val = row.get('region', '')
                amount_val = row.get('amount', '')

                # Basic format check for date YYYY-MM-DD
                if not date_val or len(date_val) != 10 or date_val[4] != '-' or date_val[7] != '-':
                    excluded['invalid_date'] += 1
                    continue
                
                if region_val not in valid_regions:
                    excluded['invalid_region'] += 1
                    continue

                try:
                    amount = float(amount_val)
                    if amount < 0:
                        raise ValueError("Negative amount")
                except (ValueError, TypeError):
                    excluded['invalid_amount'] += 1
                    continue

                # Valid row
                used_rows_count += 1
                month = date_val[:7] # YYYY-MM
                sales_data[(month, region_val)] += amount
                grand_total += amount

    except FileNotFoundError:
        print(f"Error: {input_path} not found")
        return

    # Prepare results.json values
    values = {}
    values['grand_total'] = grand_total
    
    # Sort keys for determinism
    sorted_keys = sorted(sales_data.keys())
    for month, region in sorted_keys:
        key = f"regional_{region}_{month}"
        values[key] = sales_data[(month, region)]

    results = {
        "reconciliation": {
            "input_rows": input_rows_count,
            "used_rows": used_rows_count,
            "excluded": [{"reason": r, "rows": c} for r, c in sorted(excluded.items())]
        },
        "values": values
    }

    with open(results_path, 'w', encoding='utf-8') as f:
        json.dump(results, f, indent=2, ensure_ascii=False)

    # Create report.md
    report = []
    report.append("# Sales Summary Report")
    report.append("\n## Data Reconciliation")
    report.append(f"- Total input rows: {input_rows_count}")
    report.append(f"- Used rows: {used_rows_count}")
    for exc in results["reconciliation"]["excluded"]:
        report.append(f"- Excluded ({exc['reason']}): {exc['rows']}")

    report.append("\n## Total Sales")
    report.append(f"Grand Total: {grand_total:,.2f}")

    report.append("\n## Regional Monthly Breakdown")
    for month, region in sorted_keys:
        val = values[f"regional_{region}_{month}"]
        report.append(f"- {month} | {region}: {val:,.2f}")

    with open(report_path, 'w', encoding='utf-8') as f:
        f.write("\n".join(report))

if __name__ == "__main__":
    main()
