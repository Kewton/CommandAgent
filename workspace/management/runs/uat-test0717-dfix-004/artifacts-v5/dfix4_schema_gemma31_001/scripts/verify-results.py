import json
import sys

def verify():
    path = 'output/results.json'
    try:
        with open(path, 'r') as f:
            data = json.load(f)
    except Exception as e:
        print(f"Error reading {path}: {e}")
        sys.exit(1)

    # 1. Validate root keys
    if set(data.keys()) != {"reconciliation", "values"}:
        print(f"Root keys mismatch. Expected {{'reconciliation', 'values'}}, got {list(data.keys())}")
        sys.exit(1)

    recon = data["reconciliation"]
    values = data["values"]

    # 2. Validate reconciliation structure
    if not isinstance(recon, dict):
        print("reconciliation must be a dictionary")
        sys.exit(1)
    
    expected_recon_keys = {"input_rows", "used_rows", "excluded"}
    if set(recon.keys()) != expected_recon_keys:
        print(f"Reconciliation keys mismatch. Expected {expected_recon_keys}, got {set(recon.keys())}")
        sys.exit(1)

    input_rows = recon["input_rows"]
    used_rows = recon["used_rows"]
    excluded = recon["excluded"]

    if not isinstance(input_rows, int) or not isinstance(used_rows, int):
        print("input_rows and used_rows must be integers")
        sys.exit(1)

    if not isinstance(excluded, list):
        print("excluded must be a list")
        sys.exit(1)

    sum_excluded = 0
    for i, item in enumerate(excluded):
        if not isinstance(item, dict) or set(item.keys()) != {"reason", "rows"}:
            print(f"Excluded item {i} must be a dictionary with keys {{'reason', 'rows'}}")
            sys.exit(1)
        if not isinstance(item["reason"], str) or not isinstance(item["rows"], int):
            print(f"Excluded item {i} values type mismatch: reason (str), rows (int)")
            sys.exit(1)
        sum_excluded += item["rows"]

    # 3. Verify reconciliation math
    if input_rows != used_rows + sum_excluded:
        print(f"Reconciliation math failure: {input_rows} != {used_rows} + {sum_excluded}")
        sys.exit(1)

    # 4. Validate values structure
    if not isinstance(values, dict):
        print("values must be a dictionary")
        sys.exit(1)

    for k, v in values.items():
        if not isinstance(v, (int, float)):
            print(f"Value for key {k} must be a number, got {type(v)}")
            sys.exit(1)

    print("Results schema and reconciliation validation successful.")
    sys.exit(0)

if __name__ == "__main__":
    verify()
