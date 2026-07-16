import csv
import json
import os

def inspect_csv(file_path):
    if not os.path.exists(file_path):
        print(f"Error: File {file_path} not found.")
        return None

    with open(file_path, mode='r', encoding='utf-8') as f:
        reader = csv.DictReader(f)
        columns = reader.fieldnames
        rows = list(reader)

    input_row_count = len(rows)
    
    # Simple type inference and distinct values
    type_summaries = {}
    distinct_values = {}
    
    for col in columns:
        values = [row[col] for row in rows if row[col] is not None]
        
        # Check if numeric
        is_numeric = True
        for v in values:
            try:
                float(v)
            except (ValueError, TypeError):
                is_numeric = False
                break
        
        type_summaries[col] = "number" if is_numeric else "string"
        
        # Store distinct values for non-numeric or small sets
        unique_vals = sorted(list(set(values)))
        distinct_values[col] = unique_vals

    sample_rows = rows[:5] # First 5 rows as samples

    inspection_data = {
        "column_names": columns,
        "input_row_count": input_row_count,
        "type_summaries": type_summaries,
        "distinct_values": distinct_values,
        "sample_rows": sample_rows
    }
    
    return inspection_data

def main():
    input_csv = 'data/sales.csv'
    output_json = 'output/inspection.json'
    
    # Ensure output directory exists
    os.makedirs(os.path.dirname(output_json), exist_ok=True)
    
    data = inspect_csv(input_csv)
    if data:
        with open(output_json, 'w', encoding='utf-8') as f:
            json.dump(data, f, indent=2, sort_keys=True)
        print(f"Inspection results written to {output_json}")

if __name__ == "__main__":
    main()
