#!/usr/bin/env python3
"""Display-only failure class matching from persisted campaign evidence."""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import tomllib

ROOT = Path(__file__).resolve().parents[1]


def classes(path=ROOT / "classes.toml"):
    return tomllib.loads(path.read_text(encoding="utf-8"))["class"]


def text_for(run):
    chunks = []
    for p in run.rglob("*"):
        if p.is_file() and p.suffix in {".json", ".jsonl", ".md", ".log", ".yaml"}:
            try:
                chunks.append(p.read_text(errors="replace"))
            except OSError:
                pass
    return "\n".join(chunks)


def terminal_text(run):
    """Return only terminal fields; paths and incidental prose do not classify."""
    values = []
    for p in run.rglob("events.jsonl"):
        for line in p.read_text(errors="replace").splitlines():
            try:
                event = json.loads(line)
            except ValueError:
                continue
            if event.get("event") in {
                "run_stop",
                "tui_command_stop",
                "ultra_final_acceptance",
            }:
                for key in (
                    "failure_kind",
                    "stop_class",
                    "reason",
                    "stop_reason",
                    "final_acceptance_status",
                ):
                    if event.get(key):
                        values.append(f"{key}:{event[key]}")
    return "\n".join(values)


def classify(run, registry=None):
    registry = registry or classes()
    hay = terminal_text(run) or text_for(run)
    hits = []
    for item in registry:
        terms = [
            item.get("match_stop_class"),
            item.get("match_reason"),
            item.get("match_phase"),
            item.get("match_event"),
        ]
        terms = [t for t in terms if t]
        matched = [t for t in terms if t in hay]
        if matched:
            hits.append((max(map(len, matched)), item, matched))
    if not hits:
        return {
            "run": str(run),
            "classes": [],
            "attribution": "UNKNOWN",
            "stop_class": "UNKNOWN",
        }
    best = max(x[0] for x in hits)
    chosen = [x for x in hits if x[0] == best]
    return {
        "run": str(run),
        "classes": [x[1]["id"] for x in chosen],
        "attribution": "/".join(sorted({x[1]["attribution"] for x in chosen})),
        "stop_class": "; ".join(x[2][0] for x in chosen),
    }


def classify_campaign(campaign, registry=None):
    campaign = Path(campaign)
    registry = registry or classes()
    candidates = sorted(
        {p.parent for p in campaign.rglob("workflow-events.jsonl")}
        | {p.parent for p in campaign.rglob("events.jsonl")}
    )
    runs = {}
    for run in candidates:
        relative = run.relative_to(campaign)
        identity = (
            Path(*relative.parts[1:])
            if relative.parts and relative.parts[0] in {"artifacts", "workspaces"}
            else relative
        )
        current = runs.get(identity)
        if current is None or relative.parts[0] == "artifacts":
            runs[identity] = run
    return [classify(runs[key], registry) for key in sorted(runs)]


def render(rows):
    out = [
        "# Failure class classification",
        "",
        "| run | class id | attribution | stop pattern |",
        "|---|---|---|---|",
    ]
    for r in rows:
        out.append(
            f"| `{r['run']}` | {', '.join(r['classes']) or 'UNKNOWN'} | {r['attribution']} | {r['stop_class']} |"
        )
    unknown = [r for r in rows if not r["classes"]]
    out += ["", "## UNKNOWN runs", ""]
    out += [
        f"- `{r['run']}` — stop_class原文: `{r['stop_class']}`; 一次資料: `{r['run']}`"
        for r in unknown
    ] or ["- なし"]
    return "\n".join(out) + "\n"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("campaign")
    ap.add_argument("--out")
    args = ap.parse_args()
    result = render(classify_campaign(args.campaign))
    out = Path(args.out) if args.out else Path(args.campaign) / "failure-classes.md"
    out.write_text(result, encoding="utf-8")
    print(out)


if __name__ == "__main__":
    main()
