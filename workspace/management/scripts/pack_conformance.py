#!/usr/bin/env python3
"""Run strict pack schema, vocabulary, floor, and exact-hash conformance."""

from __future__ import annotations

import argparse
import subprocess
import sys
from collections.abc import Sequence
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
HASH_PIN = "pack.sha256"


def build_command(
    pack: Path, expected_hash: str | None, binary: Path | None
) -> list[str]:
    if binary is None:
        command = [
            "cargo",
            "run",
            "--quiet",
            "--bin",
            "pack_conformance",
            "--",
            str(pack),
        ]
    else:
        command = [str(binary), str(pack)]
    if expected_hash is not None:
        command.extend(["--expect-hash", expected_hash])
    return command


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pack", type=Path, required=True)
    parser.add_argument("--expect-hash")
    parser.add_argument("--binary", type=Path)
    return parser.parse_args(argv)


def expected_hash(pack: Path, explicit: str | None) -> str | None:
    if explicit is not None:
        return explicit
    pin = pack / HASH_PIN
    if not pin.is_file():
        return None
    value = pin.read_text(encoding="utf-8").strip()
    if (
        not value.startswith("sha256:")
        or len(value) != len("sha256:") + 64
        or any(character not in "0123456789abcdef" for character in value[7:])
    ):
        raise ValueError(f"invalid exact-byte hash pin: {pin}")
    return value


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    result = subprocess.run(
        build_command(
            args.pack, expected_hash(args.pack, args.expect_hash), args.binary
        ),
        cwd=ROOT,
        check=False,
    )
    return result.returncode


if __name__ == "__main__":
    sys.exit(main())
