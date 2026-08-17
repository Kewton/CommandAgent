#!/usr/bin/env python3
"""Derive Luna cost from the recorded provider_turn_duration event shape."""
from __future__ import annotations

import json
import sys
from pathlib import Path

import tomllib


def cost(events: Path, pricing: Path) -> dict[str, object]:
    rates = tomllib.loads(pricing.read_text())
    rate = rates["gpt-5.6-luna"]
    total = 0.0
    usage = 0
    cached = 0
    for line in events.read_text().splitlines():
        event = json.loads(line)
        if event.get("event") != "provider_turn_duration" or event.get("provider") != "openai":
            continue
        tokens = event.get("provider_total_tokens")
        if not isinstance(tokens, int):
            continue
        cached_tokens = event.get("provider_cached_input_tokens", 0)
        prompt = event.get("prompt_eval_count") or event.get("estimated_prompt_tokens_sent") or 0
        output = max(tokens - int(prompt), 0)
        total += ((max(int(prompt) - int(cached_tokens), 0) / 1_000_000) * rate["input_per_million"])
        total += (int(cached_tokens) / 1_000_000) * rate["cached_input_per_million"]
        total += (output / 1_000_000) * rate["output_per_million"]
        usage += tokens
        cached += int(cached_tokens)
    return {"provider": "openai", "model": "gpt-5.6-luna", "provider_total_tokens": usage, "cached_input_tokens": cached, "cost_usd": round(total, 8)}


if __name__ == "__main__":
    print(json.dumps(cost(Path(sys.argv[1]), Path(sys.argv[2])), ensure_ascii=False))
