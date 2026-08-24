# Text Pattern Extractor CLI

This is a simple CLI tool to extract lines from a text file that match a specific pattern.

## Installation

```bash
pip install .
```

## Usage

```bash
# Basic usage to extract lines matching a pattern
python3 src/anvil_app/main.py data/sample.txt --pattern "error"

# Extract lines matching a pattern and only show the count
python3 src/anvil_app/main.py data/sample.txt --pattern "error" --count

# Show help
python3 src/anvil_app/main.py --help
```

## Examples

### Extracting lines with "error"
Input: `data/sample.txt`
Command: `python3 src/anvil_app/main.py data/sample.txt --pattern "error"`
Output:
```
[2023-10-01 10:00] ERROR: Database connection failed.
[2023-10-01 10:05] ERROR: Timeout occurred.
```

### Counting lines with "error"
Input: `data/sample.txt`
Command: `python3 src/anvil_app/main.py data/sample.txt --pattern "error" --count`
Output:
```
2
```
