#!/usr/bin/env python3
"""Idempotently collect evidence matching material for E2/I2 calibration."""

# ruff: noqa: E701,E702
from __future__ import annotations
import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
STORE = ROOT / "calibration"


def records(campaign):
    campaign = Path(campaign)
    for p in campaign.rglob("*.json"):
        try:
            d = json.loads(p.read_text())
        except (OSError, ValueError, UnicodeDecodeError):
            continue
        if not isinstance(d, dict):
            continue
        if isinstance(d.get("claims"), list):
            for c in d["claims"]:
                yield {
                    "source_run": str(p),
                    "claim": c.get("raw", c.get("quote", "")),
                    "kind": "e2" if "matched_result_value" in c else "i2",
                    "judgement": "matched"
                    if c.get("ok", c.get("matched", False))
                    else "violation",
                    "nearest_miss": c.get("nearest_miss"),
                    "observation": c.get("matched_result_value", c.get("value")),
                }


def append(campaigns):
    buckets = {"e2": [], "i2": []}
    for campaign in campaigns:
        for r in records(campaign):
            buckets[r["kind"]].append(r)
    STORE.mkdir(parents=True, exist_ok=True)
    total = 0
    for kind, rows in buckets.items():
        out = STORE / kind / "records.jsonl"
        out.parent.mkdir(parents=True, exist_ok=True)
        existing = (
            {json.loads(x).get("source_run") for x in out.read_text().splitlines()}
            if out.exists()
            else set()
        )
        with out.open("a", encoding="utf-8") as f:
            for r in rows:
                if r["source_run"] in existing:
                    continue
                f.write(json.dumps(r, ensure_ascii=False) + "\n")
                existing.add(r["source_run"])
                total += 1
    return total


def stats():
    rows = []
    for p in STORE.glob("*/records.jsonl") if STORE.exists() else []:
        rows += [json.loads(x) for x in p.read_text().splitlines()]
    counts = {}
    for r in rows:
        counts[(r["kind"], r["judgement"])] = (
            counts.get((r["kind"], r["judgement"]), 0) + 1
        )
    return "\n".join(
        ["# Calibration corpus stats", ""]
        + [f"- {k[0]} / {k[1]}: {v}" for k, v in sorted(counts.items())]
        + [f"- total: {len(rows)}"]
    )


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    a = sub.add_parser("append")
    a.add_argument("campaign", nargs="+")
    sub.add_parser("stats")
    x = ap.parse_args()
    if x.cmd == "append":
        print(f"appended {append(x.campaign)} records")
    else:
        print(stats())


if __name__ == "__main__":
    main()
