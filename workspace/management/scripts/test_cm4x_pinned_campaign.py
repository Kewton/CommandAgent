from __future__ import annotations

import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

import bench
import cm4x_pinned_campaign


class Cm4xPinnedCampaignTests(unittest.TestCase):
    def test_exact_binary_pin_accepts_match_and_rejects_drift(self) -> None:
        with TemporaryDirectory() as directory:
            binary = Path(directory) / "commandagent"
            binary.write_bytes(b"sealed instrument")
            expected = bench.sha256_file(binary)

            self.assertEqual(
                cm4x_pinned_campaign.verify_file_sha(binary, expected, "binary"),
                expected,
            )
            binary.write_bytes(b"drifted instrument")
            with self.assertRaisesRegex(bench.BenchError, "SHA-256 mismatch"):
                cm4x_pinned_campaign.verify_file_sha(binary, expected, "binary")


if __name__ == "__main__":
    unittest.main()
