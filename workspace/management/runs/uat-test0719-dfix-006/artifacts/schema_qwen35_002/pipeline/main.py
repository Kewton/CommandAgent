import csv
import json
from collections import defaultdict
from statistics import fsum

def main():
    input_file = 'data/sales.csv'
    results_file = 'output/results.json'
    report_file = 'output/report.md'
    
    input_rows = 0
    used_rows = 0
    excluded_counts = {
        'missing_value': 0,
        'invalid_format': 0
    }
    
    aggregation = defaultdict(lambda: defaultdict(float))
    total_sum = 0.0
    
    with open(input_file, 'r', newline='') as f:
        reader = csv.DictReader(f)
        for row in reader:
            input_rows += 1
            try:
                region = row.get('region', '')
                month = row.get('month', '')
                sales_str = row.get('sales', '')
                
                if not region or not month or not sales_str:
                    excluded_counts['missing_value'] += 1
                    continue
                
                sales = float(sales_str)
                aggregation[region][month] += sales
                total_sum += sales
                used_rows += 1
            except ValueError:
                excluded_counts['invalid_format'] += 1
                
    excluded_list = []
    for reason, count in sorted(excluded_counts.items()):
        if count > 0:
            excluded_list.append({"reason": reason, "rows": count})
            
    values = {}
    values['total_sales'] = total_sum
    for region in sorted(aggregation.keys()):
        for month in sorted(aggregation[region].keys()):
            key = f"{region}_{month}"
            values[key] = aggregation[region][month]
            
    results = {
        'aggregation': {k: dict(v) for k, v in sorted(aggregation.items())},
        'reconciliation': {
            'input_rows': input_rows,
            'used_rows': used_rows,
            'excluded': excluded_list
        },
        'values': values
    }
    
    with open(results_file, 'w') as f:
        json.dump(results, f, indent=2, ensure_ascii=False)
        
    # Generate report
    report_lines = [
        "# Sales Summary Report",
        f"- Total Input Rows: {input_rows}",
        f"- Valid Rows Used: {used_rows}",
        "- Excluded Rows:",
        f"  - invalid_format: {excluded_counts['invalid_format']}",
        f"  - missing_value: {excluded_counts['missing_value']}",
        f"- Total Sales: {total_sum:.2f}"
    ]
    
    report_lines.append("## Monthly Sales by Region")
    report_lines.append("| Region | Month | Sales |")
    report_lines.append("|---|---|---|")
    for region in sorted(aggregation.keys()):
        for month in sorted(aggregation[region].keys()):
            report_lines.append(f"| {region} | {month} | {aggregation[region][month]:.2f} |")
            
    with open(report_file, 'w') as f:
        f.write('\n'.join(report_lines) + '\n')

if __name__ == '__main__':
    main()
