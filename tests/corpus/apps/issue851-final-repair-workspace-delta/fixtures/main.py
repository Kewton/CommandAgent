import csv
import sys
from collections import defaultdict

def calculate_stats(file_path):
    try:
        with open(file_path, mode='r', encoding='utf-8') as f:
            reader = csv.DictReader(f)
            
            # Use lists to store values for each column to calculate statistics
            data = defaultdict(list)
            for row in reader:
                for col, val in row.items():
                    if val is None:
                        continue
                    try:
                        # Try to convert value to float
                        num_val = float(val)
                        data[col].append(num_val)
                    except ValueError:
                        # Not a numeric column or this specific cell is not numeric
                        # We keep track of the column but don't add this value
                        if col not in data:
                            # Initialize with an empty list to mark as potentially numeric 
                            # but we only care if it actually contains numbers
                            pass

            # Filter out columns that didn't have any numeric values
            numeric_cols = {col: vals for col, vals in data.items() if vals}
            
            if not numeric_cols:
                print("No numeric columns found in the CSV file.")
                return None

            stats = {}
            for col, vals in numeric_cols.items():
                stats[col] = {
                    'sum': sum(vals),
                    'avg': sum(vals) / len(vals),
                    'max': max(vals),
                    'min': min(vals),
                    'count': len(vals)
                }
            return stats

    except FileNotFoundError:
        print(f"Error: File '{file_path}' not found.")
        sys.exit(1)
    except Exception as e:
        print(f"An unexpected error occurred: {e}")
        sys.exit(1)

def print_stats_table(stats):
    if not stats:
        return

    headers = ["Column", "Sum", "Average", "Max", "Min"]
    # Determine column widths for alignment
    col_names = list(stats.keys())
    
    # Calculate max width for each column
    # Column name width
    w_col = max(len(headers[0]), max(len(name) for name in col_names))
    # Value widths (using a reasonable default or calculating from actual values)
    w_val = 15 

    # Header
    header_row = f"{headers[0]:<{w_col}} | {headers[1]:<{w_val}} | {headers[2]:<{w_val}} | {headers[3]:<{w_val}} | {headers[4]:<{w_val}}"
    separator = "-" * len(header_row)
    
    print(header_row)
    print(separator)

    for col in col_names:
        s = stats[col]
        print(f"{col:<{w_col}} | {s['sum']:<{w_val}.4f} | {s['avg']:<{w_val}.4f} | {s['max']:<{w_val}.4f} | {s['min']:<{w_val}.4f}")

def main():
    if len(sys.argv) < 2:
        print("Usage: python3 -m csv_stats.main <csv_file>")
        sys.exit(1)

    file_path = sys.argv[1]
    stats = calculate_stats(file_path)
    if stats:
        print_stats_table(stats)

if __name__ == "__main__":
    main()
