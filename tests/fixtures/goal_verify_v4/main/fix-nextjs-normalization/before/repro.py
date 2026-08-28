#!/usr/bin/env python3
import sys
from lib import normalize

CASES = {
    str(index): (f"　{chr(0xFF10 + index)}{chr(0xFF10 + index)}　", str(index) * 2)
    for index in range(1, 10)
}
CASES["10"] = ("　１０　", "10")


def main(argv):
    if len(argv) != 2 or argv[1] not in CASES:
        return 2
    source, expected = CASES[argv[1]]
    actual = normalize(source)
    if actual != expected:
        print(f"expected {expected!r}, got {actual!r}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
