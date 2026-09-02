#!/usr/bin/env python3
from pathlib import Path
import sys


def main(argv):
    if len(argv) != 2:
        return 2
    path = Path(argv[1])
    if path.is_absolute() or ".." in path.parts or not path.is_file():
        return 1
    print(path.read_text(encoding="utf-8", errors="replace")[:200])
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
