from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

import pack_conformance


class PackConformanceCommandTests(unittest.TestCase):
    def test_builds_cargo_and_prebuilt_commands_with_optional_pin(self) -> None:
        pack = Path("packs/example")
        self.assertEqual(
            pack_conformance.build_command(pack, None, None),
            [
                "cargo",
                "run",
                "--quiet",
                "--bin",
                "pack_conformance",
                "--",
                "packs/example",
            ],
        )
        self.assertEqual(
            pack_conformance.build_command(
                pack, "sha256:" + ("a" * 64), Path("target/release/check")
            ),
            [
                "target/release/check",
                "packs/example",
                "--expect-hash",
                "sha256:" + ("a" * 64),
            ],
        )

    def test_uses_reviewed_hash_pin_unless_explicitly_overridden(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            pack = Path(directory)
            pinned = "sha256:" + ("b" * 64)
            (pack / pack_conformance.HASH_PIN).write_text(
                pinned + "\n", encoding="utf-8"
            )

            self.assertEqual(pack_conformance.expected_hash(pack, None), pinned)
            self.assertEqual(
                pack_conformance.expected_hash(pack, "sha256:" + ("c" * 64)),
                "sha256:" + ("c" * 64),
            )

    def test_writes_new_hash_pin_from_green_report_without_overwriting(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            pack = Path(directory)
            exact_hash = "sha256:" + ("d" * 64)
            report = '{"exact_byte_hash":"' + exact_hash + '"}'

            self.assertEqual(pack_conformance.write_hash_pin(pack, report), exact_hash)
            self.assertEqual(
                (pack / pack_conformance.HASH_PIN).read_text(encoding="utf-8"),
                exact_hash + "\n",
            )
            with self.assertRaisesRegex(ValueError, "refusing to replace"):
                pack_conformance.write_hash_pin(pack, report)


if __name__ == "__main__":
    unittest.main()
