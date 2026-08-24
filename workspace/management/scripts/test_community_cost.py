#!/usr/bin/env python3
from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

import community_cost


class CommunityCostTests(unittest.TestCase):
    def test_mixed_luna_and_terra_events_use_exact_model_prices(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            events = root / "events.jsonl"
            pricing = root / "pricing.toml"
            pricing.write_text(
                """
["gpt-5.6-luna"]
input_per_million = 1.0
cached_input_per_million = 0.1
output_per_million = 2.0
["gpt-5.6-terra"]
input_per_million = 3.0
cached_input_per_million = 0.3
output_per_million = 4.0
""".strip()
                + "\n",
                encoding="utf-8",
            )
            rows = [
                {
                    "event": "provider_turn_duration",
                    "provider": "openai",
                    "model": "gpt-5.6-luna",
                    "prompt_eval_count": 100,
                    "provider_cached_input_tokens": 20,
                    "provider_total_tokens": 130,
                },
                {
                    "event": "provider_turn_duration",
                    "provider": "openai",
                    "model": "gpt-5.6-terra",
                    "prompt_eval_count": 200,
                    "provider_cached_input_tokens": 50,
                    "provider_total_tokens": 240,
                },
            ]
            events.write_text(
                "".join(json.dumps(row) + "\n" for row in rows), encoding="utf-8"
            )

            result = community_cost.cost(events, pricing)

            self.assertIsNone(result["model"])
            self.assertEqual(result["provider_total_tokens"], 370)
            self.assertEqual(result["cached_input_tokens"], 70)
            self.assertEqual(result["models"]["gpt-5.6-luna"]["turns"], 1)
            self.assertEqual(result["models"]["gpt-5.6-terra"]["turns"], 1)
            self.assertAlmostEqual(result["cost_usd"], 0.000767, places=9)

    def test_unknown_exact_model_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            events = root / "events.jsonl"
            pricing = root / "pricing.toml"
            pricing.write_text(
                '["gpt-5.6-terra"]\ninput_per_million=1\ncached_input_per_million=1\noutput_per_million=1\n',
                encoding="utf-8",
            )
            events.write_text(
                json.dumps(
                    {
                        "event": "provider_turn_duration",
                        "provider": "openai",
                        "model": "gpt-5.6-terra-latest",
                        "provider_total_tokens": 1,
                    }
                )
                + "\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "exact model ID"):
                community_cost.cost(events, pricing)


if __name__ == "__main__":
    unittest.main()
