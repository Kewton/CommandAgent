#!/usr/bin/env python3
"""Reproducer: full-width digits must normalize to ASCII digits."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from lib import normalize  # noqa: E402


def main() -> int:
    actual = normalize("　１２３　")
    if actual != "123":
        print(f"expected 123, got {actual!r}", file=sys.stderr)
        return 1
    print("ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
