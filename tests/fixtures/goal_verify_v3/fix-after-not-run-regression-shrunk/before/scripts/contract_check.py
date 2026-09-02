#!/usr/bin/env python3
"""Static project contract: entry point and usage documentation exist."""
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    problems = []
    entry = ROOT / "repro.py"
    if not entry.is_file() or "def main(" not in entry.read_text(encoding="utf-8"):
        problems.append("repro.py must define main()")
    readme = ROOT / "README.md"
    if not readme.is_file() or "Usage" not in readme.read_text(encoding="utf-8"):
        problems.append("README.md must document Usage")
    for problem in problems:
        print(problem, file=sys.stderr)
    return 1 if problems else 0


if __name__ == "__main__":
    raise SystemExit(main())
