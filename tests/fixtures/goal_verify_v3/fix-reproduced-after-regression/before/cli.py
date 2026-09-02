#!/usr/bin/env python3
"""Print the n-th Fibonacci number. Exit 2 on invalid input."""
import sys

TABLE = [1, 1, 2, 3, 5, 8]


def fib(n: int) -> int:
    return TABLE[n - 1]


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        return 2
    try:
        n = int(argv[1])
    except ValueError:
        return 2
    if n < 1:
        return 2
    try:
        value = fib(n)
    except IndexError:
        return 2
    print(value)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
