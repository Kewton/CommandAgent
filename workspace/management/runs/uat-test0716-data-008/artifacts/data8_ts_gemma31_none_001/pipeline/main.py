import csv
import json
from collections import defaultdict
from datetime import datetime

def main():
    input_path = 'data/sales.csv'
    output_dir = 'output'
    
    # Output files
    cleaned_csv_path = f'{output_dir}/cleaned_sales.csv'
    validation_json_path = f'{output_dir}/validation_summary.json'
    inspection_json_path = f'{output_dir}/inspection.json'
    results_json_path = f'{output_dir}/results.json'
    report_md_path = f'{output_dir}/report.md'

    # 1. Read data and perform initial cleaning/validation
    input_rows = 0
    used_rows = 0
    excluded = defaultdict(int)
    cleaned_data = []
    seen_rows = set()

    try:
        with open(input_path, mode='r', encoding='utf-8') as f:
            reader = csv.DictReader(f)
            # Standardize headers to lowercase for consistency
            fieldnames = reader.fieldnames
            if not fieldnames:
                raise ValueError("CSV file has no headers")
            
            # Map actual column names to internal keys
            col_map = {}
            for fn in fieldnames:
                fn_low = fn.lower()
                if 'date' in fn_low: col_map['date'] = fn
                if 'sales' in fn_low or 'amount' in fn_low: col_map['sales'] = fn

            if 'date' not in col_map or 'sales' not in col_map:
                raise ValueError(f"Required columns 'date' and 'sales' not found. Found: {fieldnames}")

            for row in reader:
                input_rows += 1
                
                # Check for missing values
                date_val = row.get(col_map['date'], '').strip()
                sales_val = row.get(col_map['sales'], '').strip()
                
                if not date_val or not sales_val:
                    excluded['missing_values'] += 1
                    continue
                
                # Check for valid date format (YYYY-MM-DD)
                try:
                    datetime.strptime(date_val, '%Y-%m-%d')
                except ValueError:
                    excluded['invalid_date_format'] += 1
                    continue
                
                # Check for numeric sales
                try:
                    sales_float = float(sales_val)
                except ValueError:
                    excluded['non_numeric_sales'] += 1
                    continue

                # Check for duplicates (exact row content)
                row_tuple = tuple(row.items())
                if row_tuple in seen_rows:
                    excluded['duplicate_row'] += 1
                    continue
                seen_rows.add(row_tuple)

                # Data is valid
                used_rows += 1
                cleaned_data.append({
                    'date': date_val,
                    'sales': sales_float
                })

    except FileNotFoundError:
        print(f"Error: {input_path} not found.")
        return
    except Exception as e:
        print(f"Pipeline Error: {e}")
        return

    # 2. Process monthly aggregations
    monthly_sales = defaultdict(float)
    for entry in cleaned_data:
        month = entry['date'][:7] # YYYY-MM
        monthly_sales[month] += entry['sales']

    sorted_months = sorted(monthly_sales.keys())
    
    # Calculate MoM % change and 3-month moving average
    metrics = {}
    results_values = {}
    
    previous_sales = None
    history = []
    
    for month in sorted_months:
        current_sales = monthly_sales[month]
        
        # MoM % Change
        mom_change = 0.0
        if previous_sales is not None and previous_sales != 0:
            mom_change = ((current_sales - previous_sales) / previous_sales) * 100
        
        # 3-month Moving Average
        history.append(current_sales)
        if len(history) > 3:
            history.pop(0)
        moving_avg = sum(history) / len(history)
        
        metrics[month] = {
            'total': current_sales,
            'mom_pct': mom_change,
            'ma3': moving_avg
        }
        
        # Populate results.json values for claims binding
        results_values[f'monthly_sales_{month}'] = current_sales
        results_values[f'mom_pct_{month}'] = mom_change
        results_values[f'ma3_{month}'] = moving_avg
        
        previous_sales = current_sales

    # 3. Write artifacts
    
    # output/cleaned_sales.csv
    with open(cleaned_csv_path, 'w', encoding='utf-8', newline='') as f:
        writer = csv.DictWriter(f, fieldnames=['date', 'sales'])
        writer.writeheader()
        writer.writerows(cleaned_data)

    # output/validation_summary.json
    val_summary = {reason: count for reason, count in excluded.items()}
    with open(validation_json_path, 'w', encoding='utf-8') as f:
        json.dump(val_summary, f, indent=2)

    # output/inspection.json
    inspection = {
        'input_file': input_path,
        'columns_found': fieldnames,
        'date_format': 'YYYY-MM-DD',
        'total_input_rows': input_rows,
        'valid_rows': used_rows,
        'exclusion_reasons': list(excluded.keys())
    }
    with open(inspection_json_path, 'w', encoding='utf-8') as f:
        json.dump(inspection, f, indent=2)

    # output/results.json
    results = {
        'reconciliation': {
            'input_rows': input_rows,
            'used_rows': used_rows,
            'excluded': [{'reason': r, 'rows': c} for r, c in excluded.items()]
        },
        'values': results_values
    }
    with open(results_json_path, 'w', encoding='utf-8') as f:
        json.dump(results, f, indent=2)

    # output/report.md
    with open(report_md_path, 'w', encoding='utf-8') as f:
        f.write('# Sales Summary Report\n\n')
        f.write('## Data Reconciliation\n')
        f.write(f'- Total Input Rows: {input_rows}\n')
        f.write(f'- Valid Rows Used: {used_rows}\n')
        f.write(f'- Excluded Rows: {input_rows - used_rows}\n\n')
        
        if excluded:
            f.write('### Exclusion Details\n')
            for reason, count in excluded.items():
                f.write(f'- {reason}: {count}\n')
            f.write('\n')

        f.write('## Monthly Metrics\n')
        f.write('| Month | Total Sales | MoM Change (%) | 3-Month Moving Avg |\n')
        f.write('|-------|-------------|----------------|-------------------|\n')
        for month in sorted_months:
            m = metrics[month]
            f.write(f'| {month} | {m["total"]:,.2f} | {m["mom_pct"]:.2f}% | {m["ma3"]:,.2f} |\n')

if __name__ == '__main__':
    import os
    os.makedirs('output', exist_ok=True)
    main()
