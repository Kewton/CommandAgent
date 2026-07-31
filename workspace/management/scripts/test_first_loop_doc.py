#!/usr/bin/env python3
"""Doc-drift guard for the zero-knowledge D-3c first-loop guide."""

from __future__ import annotations

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
GUIDE = ROOT / "docs" / "user" / "first-loop.md"


class FirstLoopDocDriftTests(unittest.TestCase):
    def test_referenced_commands_and_assets_exist(self) -> None:
        text = GUIDE.read_text(encoding="utf-8")
        paths = [
            "workspace/management/bench/assets/ingest/list/data/snapshots/events-list.html",
            "workspace/management/bench/suites/cli-create-elevated-cli-assist-v1-1.toml",
            "workspace/management/scripts/bench.py",
            "workspace/management/scripts/classify_runs.py",
            "workspace/management/scripts/pack_conformance.py",
            "workspace/management/scripts/scaffold.py",
            "workspace/management/runs/band_summary_cli.md",
            "packs/cli-assist/1.1.0/assist.yaml",
        ]
        missing = [path for path in paths if not (ROOT / path).is_file()]
        self.assertEqual(missing, [], f"first-loop references missing paths: {missing}")
        for path in paths:
            self.assertIn(path, text)

    def test_product_flags_and_boundary_commands_have_implementation_anchors(
        self,
    ) -> None:
        guide = GUIDE.read_text(encoding="utf-8")
        cli = (ROOT / "src" / "cli.rs").read_text(encoding="utf-8")
        repl = (ROOT / "src" / "tui" / "repl.rs").read_text(encoding="utf-8")
        slash = (ROOT / "src" / "tui" / "slash.rs").read_text(encoding="utf-8")
        for flag in [
            "--cwd",
            "--state-dir",
            "--planner-provider",
            "--planner-model",
            "--provider",
            "--model",
        ]:
            self.assertIn(flag, guide)
            self.assertIn(f"pub {flag[2:].replace('-', '_')}:", cli)
        self.assertIn('strip_prefix("/confirm ")', repl)
        self.assertIn('name: "/exit"', slash)

    def test_pack_loop_uses_the_real_scaffold_and_pin_contract(self) -> None:
        guide = GUIDE.read_text(encoding="utf-8")
        scaffold = (ROOT / "workspace/management/scripts/scaffold.py").read_text(
            encoding="utf-8"
        )
        conformance = (
            ROOT / "workspace/management/scripts/pack_conformance.py"
        ).read_text(encoding="utf-8")
        bench = (ROOT / "workspace/management/scripts/bench.py").read_text(
            encoding="utf-8"
        )
        for marker in [
            "scaffold.py pack cli-assist --from-version 1.1.0 --version 1.1.1",
            "pack_conformance.py --pack packs/cli-assist/1.1.1 --write-pin",
            'pack_version = "1.1.1"',
            'pack_hash = "<exact-byte-hash>"',
            "--suite /tmp/cli-assist-local-1.1.1.toml",
        ]:
            self.assertIn(marker, guide)
        self.assertIn('sub.add_parser("pack")', scaffold)
        self.assertIn('"--write-pin"', conformance)
        for field in ("pack_id", "pack_version", "pack_hash"):
            self.assertIn(f'"{field}"', bench)

    def test_gate_examples_keep_every_required_user_decision_visible(self) -> None:
        text = GUIDE.read_text(encoding="utf-8")
        for marker in [
            "# Gate 1 — Request confirmation",
            "This card is a proposal, not an earned result.",
            "Dispatching ingest × create × list.",
            "# Gate 3 — Acceptance",
            "## 1. Confirmed identity",
            "## 2. Terminal projection",
            "## 3. Definition of done",
            "## 4. Machine evidence",
            "## 5. Stop reason",
            "typed unknown",
            "recovery_circle",
            "elevated_model",
            "pack_change",
        ]:
            self.assertIn(marker, text)


if __name__ == "__main__":
    unittest.main()
