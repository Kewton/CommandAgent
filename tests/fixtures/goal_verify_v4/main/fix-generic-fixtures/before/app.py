#!/usr/bin/env python3
import json
import sys


def main(argv):
    if len(argv) != 2:
        return 2
    with open(argv[1], encoding="utf-8") as handle:
        payload = json.load(handle)
    print(sum(item["amount"] for item in payload["items"]))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
