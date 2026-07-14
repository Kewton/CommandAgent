import csv
import json
import os

def inspect_csv(file_path):
    if not os.path.exists(file_path):
        print(f"Error: {file_path} not found.")
        return None

    with open(file_path, mode='r', encoding='utf-8') as f:
        reader = csv.DictReader(f)
        columns = reader.fieldnames
        rows = list(reader)

    row_count = len(rows)
    type_summaries = {}
    distinct_values = {}
    
    for col in columns:
        values = [row[col] for row in rows]
        # Basic type inference: check if all non-empty values are digits/floats
        non_empty = [v for v in values if v.strip()]
        if not non_empty:
            type_summaries[col] = "empty"
        else:
            try:
                [float(v) for v in non_empty]
                type_summaries[col] = "numeric"
            except ValueError:
                type_summaries[col] = "string"
        
        # Store distinct values for categorical (string) columns
        if type_summaries[col] == "string":
            distinct_values[col] = sorted(list(set(values)))

    inspection = {
        "column_names": columns,
        "input_row_count": row_count,
        "type_summaries": type_summaries,
        "distinct_values": distinct_values,
        "sample_rows": rows[:5]
    }
    return inspection

if __name__ == "__main__":
    input_path = 'data/sales.csv'
    output_path = 'output/inspection.json'
    
    result = inspect_csv(input_path)
    if result:
        os.makedirs(os.path.dirname(output_path), exist_ok=True)
        with open(output_path, 'w', encoding='utf-8') as f:
            json.dump(result, f, indent=2)
        print(f"Inspection complete. Result written to {output_path}")
