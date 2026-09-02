#!/usr/bin/env python3
import importlib.util
import sys
from pathlib import Path


def load_pipeline():
    spec = importlib.util.spec_from_file_location("a15_data_pipeline", "pipeline/main.py")
    if spec is None or spec.loader is None:
        raise RuntimeError("pipeline loader unavailable")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main(argv):
    if len(argv) != 2:
        return 2
    source = Path(argv[1])
    if source.parent != Path("data") or not source.is_file():
        return 2
    result = load_pipeline().write_outputs(source)
    task = int(source.stem.removeprefix("task-"))
    expected = {
        "reconciliation": {
            "input_rows": 3,
            "used_rows": 2,
            "excluded": [{"reason": "non_numeric_amount", "rows": 1}],
        },
        "values": {"total": task * 2 + 1},
    }
    if result != expected:
        print(f"expected {expected!r}, got {result!r}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
