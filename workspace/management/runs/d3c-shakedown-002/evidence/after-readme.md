# Text Pattern Extractor CLI

This CLI tool extracts lines containing a specified pattern from a text file or stdin.

## Installation

```bash
pip install .
```

## Usage

```bash
anvil-cli [options] [file]
```

### Arguments

- `file`: (Optional) Path to the text file to process. If omitted, the tool reads from standard input.

### Options

- `--pattern <string>`: The search string to filter lines.
- `--count`: If specified, output only the total number of matching lines.
- `--help`: Show this help message and exit.

## Examples

### Extracting lines with a pattern from a file
```bash
anvil-cli --pattern "Python" data/sample.txt
```
**Expected Output:**
```
Python is a high-level programming language.
Python is widely used for data science.
```

### Counting matching lines from a file
```bash
anvil-cli --pattern "Python" --count data/sample.txt
```
**Expected Output:**
```
2
```

### Using standard input
```bash
cat data/sample.txt | anvil-cli --pattern "language"
```
**Expected Output:**
```
Python is a high-level programming language.
```
