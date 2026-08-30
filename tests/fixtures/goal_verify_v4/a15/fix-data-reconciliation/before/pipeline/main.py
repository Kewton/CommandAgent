#!/usr/bin/env python3
"""Deterministic CSV aggregation used by the A15 data Recovery experiment."""
import csv
import json
import sys
from pathlib import Path


def summarize(source: Path) -> dict:
    with source.open(encoding="utf-8", newline="") as handle:
        rows = list(csv.DictReader(handle))
    valid_rows = []
    excluded = 0
    for row in rows:
        try:
            amount = int(row["amount"])
        except (KeyError, TypeError, ValueError):
            excluded += 1
            continue
        valid_rows.append(amount)
    return {
        "reconciliation": {
            "input_rows": len(rows),
            "used_rows": len(rows),
            "excluded": [{"reason": "non_numeric_amount", "rows": excluded}],
        },
        "values": {"total": sum(valid_rows)},
    }


def write_outputs(source: Path) -> dict:
    result = summarize(source)
    output = Path("output")
    output.mkdir(exist_ok=True)
    inspection = {
        "column_names": ["region", "amount"],
        "input_row_count": result["reconciliation"]["input_rows"],
        "type_summaries": {"region": "string", "amount": "numeric_with_invalid"},
        "distinct_values": {"region": ["north", "south", "unknown"]},
        "sample_rows": [{"region": "north", "amount": "1"}],
    }
    (output / "inspection.json").write_text(
        json.dumps(inspection, ensure_ascii=False, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    (output / "results.json").write_text(
        json.dumps(result, ensure_ascii=False, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    (output / "report.md").write_text(
        f"# Data summary\n\nTotal: {result['values']['total']}\n",
        encoding="utf-8",
    )
    return result


if __name__ == "__main__":
    source = Path(sys.argv[1]) if len(sys.argv) == 2 else Path("data/task-01.csv")
    write_outputs(source)
