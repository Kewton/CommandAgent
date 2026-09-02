#!/usr/bin/env python3
"""Sum the amounts of a fixture file and print the total."""
import json
import sys


def total(items):
    return sum(item["amount"] for item in items)


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        return 2
    with open(argv[1], encoding="utf-8") as handle:
        data = json.load(handle)
    print(total(data["items"]))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
