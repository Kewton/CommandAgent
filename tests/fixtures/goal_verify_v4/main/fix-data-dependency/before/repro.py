#!/usr/bin/env python3
"""Load the task-specific dependency; every dependency is absent offline."""
import importlib
import sys


def main(argv):
    if len(argv) != 2 or not argv[1].isdigit():
        return 2
    importlib.import_module(f"repro_dep_{int(argv[1]):02d}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
