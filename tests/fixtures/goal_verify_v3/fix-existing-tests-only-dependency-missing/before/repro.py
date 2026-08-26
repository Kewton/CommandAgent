#!/usr/bin/env python3
"""Reproducer that needs the optional dependency repro_dep (intentionally absent)."""
import sys
from pathlib import Path

import repro_dep  # noqa: F401  (ModuleNotFoundError is the expected observation)

sys.path.insert(0, str(Path(__file__).resolve().parent))

from lib import parse_rows  # noqa: E402


def main() -> int:
    rows = parse_rows("a,b\n1,2\n")
    return 0 if repro_dep.check(rows) else 1


if __name__ == "__main__":
    raise SystemExit(main())
