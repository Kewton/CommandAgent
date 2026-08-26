#!/usr/bin/env python3
"""Print the n-th Fibonacci number. Exit 2 on invalid input."""
import sys


def fib(n: int) -> int:
    a, b = 1, 1
    for _ in range(n - 1):
        a, b = b, a + b
    return a


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        return 2
    try:
        n = int(argv[1])
    except ValueError:
        return 2
    if n < 1:
        return 2
    print(fib(n))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
