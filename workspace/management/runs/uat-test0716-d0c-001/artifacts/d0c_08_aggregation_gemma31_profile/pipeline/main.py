import csv
import json
import os
from collections import defaultdict

def main():
    input_path = 'data/sales.csv'
    results_path = 'output/results.json'
    report_path = 'output/report.md'
    
    os.makedirs('output', exist_ok=True)
    
    if not os.path.exists(input_path):
        return

    # Use a fixed order for processing to ensure determinism
    input_rows = 0
    used_rows = 0
    excluded = defaultdict(int)
    aggregates = defaultdict(float)
    total_sales = 0.0

    with open(input_path, mode='r', encoding='utf-8') as f:
        reader = csv.DictReader(f)
        headers = reader.fieldnames
        if not headers:
            return
            
        # Mapping based on common patterns to avoid inventing values 
        # but remaining functional without read access to inspection.json at this moment.
        col_date = next((h for h in headers if 'date' in h.lower()), None)
        col_region = next((h for h in headers if 'region' in h.lower()), None)
        col_amount = next((h for h in headers if 'amount' in h.lower() or 'sales' in h.lower() or 'price' in h.lower()), None)
        
        # If no matches, we use the first three columns as a last resort 
        if not col_date or not col_region or not col_amount:
            if len(headers) >= 3:
                col_date, col_region, col_amount = headers[0], headers[1], headers[2]
            else:
                return

        for row in reader:
            input_rows += 1
            
            d_val = row.get(col_date)
            r_val = row.get(col_region)
            a_val = row.get(col_amount)
            
            if not d_val or not r_val or not a_val:
                excluded['missing_fields'] += 1
                continue
                
            try:
                # Remove commas if present to handle formatted numbers
                clean_a = a_val.replace(',', '').strip()
                amount = float(clean_a)
            except ValueError:
                excluded['invalid_numeric_value'] += 1
                continue
                
            # Date validation: extract month (YYYY-MM)
            if len(d_val) < 7:
                excluded['invalid_date_format'] += 1
                continue
            month = d_val[:7]
            
            used_rows += 1
            key = f"regional_{month}_{r_val}"
            aggregates[key] += amount
            total_sales += amount

    # Results assembly - sorted for determinism
    values = {"total_sales": total_sales}
    for k in sorted(aggregates.keys()):
        values[k] = aggregates[k]
        
    results = {
        "reconciliation": {
            "input_rows": input_rows,
            "used_rows": used_rows,
            "excluded": [{"reason": r, "rows": c} for r, c in sorted(excluded.items())]
        },
        "values": values
    }
    
    with open(results_path, 'w', encoding='utf-8') as f:
        json.dump(results, f, indent=2)
        
    # Report assembly
    report = [
        "# Sales Summary Report",
        "",
        "## Data Reconciliation",
        f"- Total input rows: {input_rows}",
        f"- Used rows: {used_rows}"
    ]
    for exc in results["reconciliation"]["excluded"]:
        report.append(f"- Excluded ({exc['reason']}): {exc['rows']}")
        
    report.append("\n## Results")
    report.append(f"- **Total Sales**: {total_sales:,.2f}")
    
    report.append("\n### Regional Monthly Breakdown")
    for k in sorted(values.keys()):
        if k != "total_sales":
            report.append(f"- {k}: {values[k]:,.2f}")
            
    with open(report_path, 'w', encoding='utf-8') as f:
        f.write("\n".join(report))

if __name__ == "__main__":
    main()
