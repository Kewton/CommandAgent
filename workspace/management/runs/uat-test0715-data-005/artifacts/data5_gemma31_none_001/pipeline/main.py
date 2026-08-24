import csv
import json
from datetime import datetime
from collections import defaultdict

def main():
    input_path = 'data/sales.csv'
    inspection_path = 'output/inspection.json'
    results_path = 'output/results.json'
    report_path = 'output/report.md'

    # 1. Read data and perform inspection
    try:
        with open(input_path, mode='r', encoding='utf-8') as f:
            reader = list(csv.DictReader(f))
    except FileNotFoundError:
        print(f"Error: {input_path} not found.")
        return

    if not reader:
        print("Error: No data found in CSV.")
        return

    headers = reader[0].keys()
    sample_rows = reader[:5]
    
    # Identify region column and unique values
    # We look for a column containing 'region' (case-insensitive)
    region_col = next((h for h in headers if 'region' in h.lower()), None)
    date_col = next((h for h in headers if 'date' in h.lower()), None)
    amount_col = next((h for h in headers if 'amount' in h.lower() or 'sales' in h.lower()), None)

    regions = set()
    if region_col:
        for row in reader:
            val = row.get(region_col)
            if val:
                regions.add(val)
    
    inspection_data = {
        "headers": list(headers),
        "sample_rows": sample_rows,
        "unique_regions": sorted(list(regions)),
        "columns": {
            "region": region_col,
            "date": date_col,
            "amount": amount_col
        }
    }

    with open(inspection_path, 'w', encoding='utf-8') as f:
        json.dump(inspection_data, f, indent=2)

    # 2. Validation and Aggregation
    input_rows_count = len(reader)
    used_rows_count = 0
    excluded = defaultdict(int)
    
    # Aggregation map: {(month, region): sum}
    aggregation = defaultdict(float)
    total_sales = 0.0

    valid_regions = set(inspection_data["unique_regions"])
    
    for row in reader:
        # Check for missing fields
        if not all([row.get(region_col), row.get(date_col), row.get(amount_col)]):
            excluded["missing_fields"] += 1
            continue

        # Validate date (YYYY-MM-DD)
        date_str = row[date_col]
        try:
            dt = datetime.strptime(date_str, '%Y-%m-%d')
            month = dt.strftime('%Y-%m')
        except ValueError:
            excluded["invalid_date"] += 1
            continue

        # Validate amount (numeric)
        amount_str = row[amount_col]
        try:
            amount = float(amount_str)
        except ValueError:
            excluded["non_numeric_sales"] += 1
            continue

        # Validate region (from observed set)
        region = row[region_col]
        if region not in valid_regions:
            excluded["unknown_region"] += 1
            continue

        # All validations passed
        used_rows_count += 1
        aggregation[(month, region)] += amount
        total_sales += amount

    # Prepare results.json
    # Sort excluded reasons for determinism
    sorted_excluded = [{"reason": r, "rows": count} for r, count in sorted(excluded.items())]
    
    # Flat values map for claims binding
    values = {
        "total_sales": total_sales
    }
    for (month, region), val in sorted(aggregation.items()):
        key = f"sales_{month}_{region}".replace(" ", "_")
        values[key] = val

    # Also add excluded counts to values for report access
    for r, count in sorted(excluded.items()):
        values[f"excluded_{r}"] = count

    results_data = {
        "reconciliation": {
            "input_rows": input_rows_count,
            "used_rows": used_rows_count,
            "excluded": sorted_excluded
        },
        "values": values
    }

    with open(results_path, 'w', encoding='utf-8') as f:
        json.dump(results_data, f, indent=2)

    # 3. Generate report.md
    with open(report_path, 'w', encoding='utf-8') as f:
        f.write("# Sales Summary Report\n\n")
        f.write("## Data Inspection\n")
        f.write(f"- Headers: {', '.join(headers)}\n")
        f.write(f"- Unique Regions: {', '.join(sorted(list(regions)))}\n\n")
        
        f.write("## Validation Results\n")
        f.write(f"- Total Rows Processed: {input_rows_count}\n")
        f.write(f"- Valid Rows Used: {used_rows_count}\n")
        for item in sorted_excluded:
            f.write(f"- Excluded ({item['reason']}): {item['rows']}\n")
        f.write("\n")

        f.write("## Aggregation Results\n")
        f.write(f"- **Total Overall Sales: {total_sales:.2f}**\n\n")
        f.write("| Month | Region | Total Sales |\n")
        f.write("|-------|--------|-------------|\n")
        for (month, region), val in sorted(aggregation.items()):
            f.write(f"| {month} | {region} | {val:.2f} |\n")

if __name__ == "__main__":
    main()
