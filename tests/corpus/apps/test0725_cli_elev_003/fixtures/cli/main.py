import argparse
import sys

def main():
    parser = argparse.ArgumentParser(description="Extract lines containing a specific pattern from a text file.")
    parser.add_argument("file", help="Path to the input text file")
    parser.add_argument("--pattern", required=True, help="The search string to filter lines")
    parser.add_argument("--count", action="store_true", help="Display only the number of matching lines")

    args = parser.parse_args()

    try:
        with open(args.file, "r", encoding="utf-8") as f:
            lines = f.readlines()
    except FileNotFoundError:
        print(f"Error: File {args.file} not found.", file=sys.stderr)
        sys.exit(1)
    except Exception as e:
        print(f"Error reading file: {e}", file=sys.stderr)
        sys.exit(1)

    matches = [line.strip() for line in lines if args.pattern in line]

    if args.count:
        print(len(matches))
    else:
        for match in matches:
            print(match)

if __name__ == "__main__":
    main()
