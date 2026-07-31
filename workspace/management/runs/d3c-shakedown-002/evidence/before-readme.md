# Line Extraction CLI Tool

This is a simple CLI tool to extract lines from a text file that contain a specific search pattern.

## Usage

```bash
python cli/main.py --pattern <pattern> [file_path]
```

### Arguments
- `file_path` (Optional): Path to the input text file. If omitted, the tool reads from standard input.

### Options
- `--pattern <string>` (Required): The string to search for in each line.
- `--count` (Flag): If specified, the tool prints only the number of matching lines instead of the lines themselves.
- `--help`: Show the help message and exit.

## Examples

### Extracting matching lines
To find all lines containing "apple" in `data/sample.txt`:
```bash
python cli/main.py --pattern apple data/sample.txt
```
**Expected Output:**
```text
I like apple.
Apple is red.
```

### Counting matching lines
To count how many lines contain "apple" in `data/sample.txt`:
```bash
python cli/main.py --pattern apple --count data/sample.txt
```
**Expected Output:**
```text
2
```

### Using with standard input
```bash
echo "hello world" | python cli/main.py --pattern hello
```
**Expected Output:**
```text
hello world
```
