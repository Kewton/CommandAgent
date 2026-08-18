#!/usr/bin/env python3
"""Derive OpenAI cost from recorded provider_turn_duration events."""
from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

import tomllib


def _event_paths(events: Path) -> list[Path]:
    if events.is_file():
        return [events]
    return sorted(events.rglob("events.jsonl"))


def _integer(event: dict[str, Any], key: str) -> int:
    value = event.get(key)
    return value if isinstance(value, int) and not isinstance(value, bool) else 0


def cost(events: Path, pricing: Path) -> dict[str, object]:
    rates = tomllib.loads(pricing.read_text())
    total = 0.0
    usage = 0
    cached = 0
    models: dict[str, dict[str, int | float]] = {}
    for path in _event_paths(events):
        for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
            event = json.loads(line)
            if (
                event.get("event") != "provider_turn_duration"
                or event.get("provider") != "openai"
            ):
                continue
            tokens = event.get("provider_total_tokens")
            model = event.get("model")
            if not isinstance(tokens, int) or not isinstance(model, str):
                continue
            if model not in rates:
                raise ValueError(f"pricing is missing exact model ID: {model}")
            rate = rates[model]
            cached_tokens = _integer(event, "provider_cached_input_tokens")
            prompt = _integer(event, "prompt_eval_count") or _integer(
                event, "estimated_prompt_tokens_sent"
            )
            output = max(tokens - prompt, 0)
            turn_cost = (
                max(prompt - cached_tokens, 0)
                * rate["input_per_million"]
                / 1_000_000
                + cached_tokens
                * rate["cached_input_per_million"]
                / 1_000_000
                + output * rate["output_per_million"] / 1_000_000
            )
            model_totals = models.setdefault(
                model,
                {
                    "turns": 0,
                    "provider_total_tokens": 0,
                    "cached_input_tokens": 0,
                    "cost_usd": 0.0,
                },
            )
            model_totals["turns"] += 1
            model_totals["provider_total_tokens"] += tokens
            model_totals["cached_input_tokens"] += cached_tokens
            model_totals["cost_usd"] += turn_cost
            total += turn_cost
            usage += tokens
            cached += cached_tokens
    for model_totals in models.values():
        model_totals["cost_usd"] = round(float(model_totals["cost_usd"]), 8)
    model = next(iter(models)) if len(models) == 1 else None
    return {
        "provider": "openai",
        "model": model,
        "models": models,
        "provider_total_tokens": usage,
        "cached_input_tokens": cached,
        "cost_usd": round(total, 8),
    }


if __name__ == "__main__":
    print(json.dumps(cost(Path(sys.argv[1]), Path(sys.argv[2])), ensure_ascii=False))
