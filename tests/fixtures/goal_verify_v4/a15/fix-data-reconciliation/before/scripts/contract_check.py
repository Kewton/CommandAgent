#!/usr/bin/env python3
import json
from pathlib import Path


required = [
    Path("pipeline/main.py"),
    Path("output/inspection.json"),
    Path("output/results.json"),
    Path("output/report.md"),
]
if not all(path.is_file() for path in required):
    raise SystemExit(1)
document = json.loads(Path("output/results.json").read_text(encoding="utf-8"))
raise SystemExit(0 if set(document) == {"reconciliation", "values"} else 1)
