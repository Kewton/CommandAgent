import csv
import json
from collections import defaultdict
from datetime import datetime

def main():
    input_path = 'data/sales.csv'
    inspection_path = 'output/inspection.json'
    results_path = 'output/results.json'
    report_path = 'output/report.md'

    # 1. Inspection Phase
    try:
        with open(input_path, mode='r', encoding='utf-8') as f:
            reader = csv.reader(f)
            header = next(reader, None)
            if header is None:
                print("Empty CSV file")
                return
            
            rows = list(reader)
    except FileNotFoundError:
        print(f"File {input_path} not found")
        return

    # Determine columns and types from data
    sample_rows = rows[:5]
    all_regions = set()
    for row in rows:
        if len(row) >= 2: # Assuming region is one of the cols
            all_regions.add(row[1])

    inspection = {
        "columns": header,
        "sample_rows": sample_rows,
        "observed_regions": sorted(list(all_regions)) if all_regions else []
    }
    with open(inspection_path, 'w', encoding='utf-8') as f:
        json.dump(inspection, f, indent=2)

    # 2. Processing Phase
    # Define indices based on expected order: date, region, amount
    # We use the header to find correct indices for robustness
    try:
        idx_date = header.index('date')
        idx_region = header.index('region')
        idx_amount = header.index('amount')
    except ValueError:
        # Fallback to positions if headers differ from expectations
        idx_date, idx_region, idx_amount = 0, 1, 2

    input_rows_count = len(rows)
    used_rows_count = 0
    excluded = defaultdict(int)
    
    # sales_map[month][region] = sum
    sales_map = defaultdict(lambda: defaultdict(float))
    total_sales = 0.0

    for row in rows:
        if len(row) < 3:
            excluded['missing_columns'] += 1
            continue
        
        date_str = row[idx_date].strip()
        region_str = row[idx_region].strip()
        amount_str = row[idx_amount].strip()

        if not date_str or not region_str or not amount_str:
            excluded['empty_fields'] += 1
            continue

        try:
            # Date validation (YYYY-MM-DD)
            date_obj = datetime.strptime(date_str, '%Y-%m-%d')
            month = date_obj.strftime('%Y-%m')
            
            # Amount validation
            amount = float(amount_str)
            
            # Accumulate
            sales_map[month][region_str] += amount
            total_sales += amount
            used_rows_count += 1
        except ValueError as e:
            if 'invalid literal for float()' in str(e):
                excluded['invalid_amount'] += 1
            else:
                excluded['invalid_date'] += 1
            continue

    # Format excluded list for results.json
    excluded_list = [{"reason": reason, "rows": count} for reason, count in sorted(excluded.items())]

    # 3. Results Generation
    values = {}
    # Monthly x Regional claims
    sorted_months = sorted(sales_map.keys())
    for month in sorted_months:
        sorted_regions = sorted(sales_map[month].keys())
        for region in sorted_regions:
            key = f"sales_{month}_{region}"
            values[key] = sales_map[month][region]

    # Overall total claim
    values['total_sales'] = total_sales

    results = {
        "reconciliation": {
            "input_rows": input_rows_count,
            "used_rows": used_rows_count,
            "excluded": excluded_list
        },
        "values": values
    }

    with open(results_path, 'w', encoding='utf-8') as f:
        json.dump(results, f, indent=2)

    # 4. Report Generation
    report_lines = []
    report_lines.append("# Sales Summary Report\n")
    
    report_lines.append("## Data Reconciliation")
    report_lines.append(f"- Total input rows: {input_rows_count}")
    report_lines.append(f"- Used rows: {used_rows_count}")
    for item in excluded_list:
        report_lines.append(f"- Excluded ({item['reason']}): {item['rows']}")
    report_lines.append("")

    report_lines.append("## Monthly Regional Sales")
    for month in sorted_months:
        report_lines.append(f"### {month}")
        sorted_regions = sorted(sales_map[month].keys())
        for region in sorted_regions:
            val = sales_map[month][region]
            key = f"sales_{month}_{region}"
            report_lines.append(f"- {region}: {val:.2f} (key: {key})")
        report_lines.append("")

    report_lines.append("## Total Summary")
    report_lines.append(f"- Overall Total Sales: {total_sales:.2f} (key: total_sales)")

    with open(report_path, 'w', encoding='utf-8') as f:
        f.write("\n".join(report_lines))

if __name__ == "__main__":
    main()
