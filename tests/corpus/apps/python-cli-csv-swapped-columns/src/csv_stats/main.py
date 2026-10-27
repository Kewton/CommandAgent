#!/usr/bin/env python3
"""Synthetic minimal CSV stats CLI with the IC-37 defect.

The header names max before min but every row prints min before max.
"""
import csv
import sys


def main() -> None:
    with open(sys.argv[1], newline="") as handle:
        rows = list(csv.DictReader(handle))
    print("column | sum | mean | max | min")
    for column in rows[0]:
        try:
            values = [float(row[column]) for row in rows]
        except ValueError:
            continue
        total = sum(values)
        mean = total / len(values)
        print(f"{column} | {total:g} | {mean:g} | {min(values):g} | {max(values):g}")


if __name__ == "__main__":
    main()
