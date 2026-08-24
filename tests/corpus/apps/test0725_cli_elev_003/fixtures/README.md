# Line Filter CLI Tool

A simple CLI tool to filter lines from a text file based on a specified pattern.

## Usage

```bash
python3 cli/main.py <file_path> --pattern <search_string> [--count]
```

### Arguments
- `<file_path>`: Path to the input text file.
- `--pattern`: The string to search for in each line.
- `--count`: Optional flag. If provided, the tool will only output the total number of matching lines instead of the lines themselves.
- `--help`: Show usage information.

## Examples

### 1. Extract lines containing "apple"
```bash
python3 cli/main.py data/sample.txt --pattern apple
```
**Expected Output:**
```text
I like apple.
Apple is red.
An apple a day keeps the doctor away.
```

### 2. Count lines containing "banana"
```bash
python3 cli/main.py data/sample.txt --pattern banana --count
```
**Expected Output:**
```text
2
```

## Files
- `cli/main.py`: The main implementation of the CLI tool.
- `data/sample.txt`: Sample input data for testing.
