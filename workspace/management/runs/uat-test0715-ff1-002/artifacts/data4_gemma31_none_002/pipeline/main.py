import csv
import json
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
        print(f"Error: {input_path} not found")
        return

    # 1. Schema Inspection
    # Derive validation rules from observed data (as per contract)
    inspection = {
        "columns": [],
        "observed_types": {}
    }
    if rows:
        cols = rows[0].keys()
        inspection["columns"] = list(cols)
        for col in cols:
            # Simple type observation from the first valid-looking row if possible
            val = rows[0][col]
            try:
                float(val)
                inspection["observed_types"][col] = "numeric"
            except (ValueError, TypeError):
                inspection["observed_types"][col] = "string"

    with open(inspection_path, 'w', encoding='utf-8') as f:
        json.dump(inspection, f, indent=2)

    # 2. Validation and Processing
    input_rows_count = len(rows)
    used_rows = []
    excluded_counts = defaultdict(int)

    for row in rows:
        # Basic validation rules derived from intended goal
        if not row.get('date') or not row.get('region') or not row.get('amount'):
            excluded_counts["missing_fields"] += 1
            continue
        
        try:
            # Validate amount is numeric
            float(row['amount'])
            # Validate date format (YYYY-MM-DD)
            if len(row['date']) < 7 or row['date'][4] != '-' or row['date'][7:8] != '-':
                raise ValueError("invalid date format")
        except ValueError as e:
            reason = "invalid_numeric" if "float" in str(e) else "invalid_date"
            if "invalid date format" in str(e): reason = "invalid_date"
            excluded_counts[reason] += 1
            continue

        used_rows.append(row)

    # 3. Aggregation
    # Monthly x Region aggregation
    monthly_region_sales = defaultdict(float)
    total_sales = 0.0

    for row in used_rows:
        month = row['date'][:7] # YYYY-MM
        region = row['region']
        amount = float(row['amount'])
        monthly_region_sales[(month, region)] += amount
        total_sales += amount

    # 4. Reconciliation and Results Construction
    excluded_list = [{"reason": k, "rows": v} for k, v in sorted(excluded_counts.items())]
    
    values = {}
    # Add total sales first for stability
    values["total_sales"] = total_sales
    
    # Add aggregated values: month_region_<month>_<region>
    # Sort keys to ensure deterministic output order
    sorted_agg_keys = sorted(monthly_region_sales.keys())
    for month, region in sorted_agg_keys:
        key = f"sales_{month}_{region}"
        values[key] = monthly_region_sales[(month, region)]

    results = {
        "reconciliation": {
            "input_rows": input_rows_count,
            "used_rows": len(used_rows),
            "excluded": excluded_list
        },
        "values": values
    }

    with open(results_path, 'w', encoding='utf-8') as f:
        json.dump(results, f, indent=2)

    # 5. Report Generation
    report = []
    report.append("# Sales Summary Report")
    report.append("\n## Data Reconciliation")
    report.append(f"- Total input rows: {results['reconciliation']['input_rows']}")
    report.append(f"- Used rows: {results['reconciliation']['used_rows']}")
    for exc in results['reconciliation']['excluded']:
        report.append(f"- Excluded ({exc['reason']}): {exc['rows']}")

    report.append("\n## Financial Summary")
    report.append(f"- **Total Sales: {values['total_sales']:.2f}**")

    report.append("\n## Monthly Regional Breakdown")
    # Sort keys again for report stability
    for month, region in sorted_agg_keys:
        val = values[f"sales_{month}_{region}"]
        report.append(f"- {month} | {region}: {val:.2f}")

    with open(report_path, 'w', encoding='utf-8') as f:
        f.write("\n".join(report))

if __name__ == "__main__":
    main()
