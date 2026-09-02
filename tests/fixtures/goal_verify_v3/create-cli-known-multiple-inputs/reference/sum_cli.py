#!/usr/bin/env python3
import sys


def main() -> int:
    if len(sys.argv) != 3:
        return 2
    try:
        left, right = (int(value) for value in sys.argv[1:])
    except ValueError:
        return 2
    print(left + right)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
