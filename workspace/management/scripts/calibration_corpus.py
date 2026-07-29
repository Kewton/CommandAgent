#!/usr/bin/env python3
"""Idempotently collect evidence matching material for E2/I2 calibration."""

from __future__ import annotations

import argparse
import json
from pathlib import Path

from evidence_envelope import EnvelopeError, envelope_for

ROOT = Path(__file__).resolve().parents[1]
STORE = ROOT / "calibration"
ENVELOPE_KINDS = {
    ("E", "claims_binding"): ("e2", False),
    ("I", "investigation_binding"): ("i2", False),
    ("C", "help_binding"): ("c2", True),
    ("C", "argv_probe"): ("c3", True),
    ("N", "source_binding"): ("n2", True),
}


def evidence_record_id(campaign, evidence, kind, index):
    """Identify one logical run evidence file across workspace/artifact copies."""
    campaign = Path(campaign)
    evidence = Path(evidence)
    try:
        relative = evidence.relative_to(campaign)
    except ValueError:
        relative = evidence
    if relative.parts and relative.parts[0] in {"artifacts", "workspaces"}:
        relative = Path(*relative.parts[1:])
    return f"{campaign.name}/{relative.as_posix()}#{kind}:{index}"


def envelope_records(campaign, path, envelope):
    binding = ENVELOPE_KINDS.get((envelope["family"], envelope["kind"]))
    if binding is None:
        return
    kind, nearest_only = binding
    nearest = {}
    for item in envelope["nearest_miss"]:
        if not isinstance(item, dict) or not isinstance(item.get("claim_index"), int):
            raise EnvelopeError(f"invalid nearest_miss entry: {path}")
        nearest[item["claim_index"]] = item.get("value")
    for index, claim in enumerate(envelope["claims"]):
        if not isinstance(claim, dict):
            raise EnvelopeError(f"invalid claim entry: {path}")
        miss = nearest.get(index)
        if nearest_only and miss is None:
            continue
        observation = claim.get("observation")
        if isinstance(observation, dict):
            stderr = observation.get("stderr", {})
            stdout = observation.get("stdout", {})
            observation = (
                stderr.get("text")
                if isinstance(stderr, dict) and stderr.get("text")
                else (
                    stdout.get("text")
                    if isinstance(stdout, dict) and stdout.get("text")
                    else observation
                )
            )
        row = {
            "record_id": evidence_record_id(campaign, path, kind, index),
            "source_run": str(path),
            "claim": claim.get("label", ""),
            "kind": kind,
            "judgement": claim.get("judgement", "violation"),
            "nearest_miss": miss,
            "observation": observation,
        }
        if claim.get("direction") is not None:
            row["direction"] = claim["direction"]
        if claim.get("source_ref") is not None:
            row["source"] = claim["source_ref"]
        yield row


def records(campaign):
    campaign = Path(campaign)
    for p in campaign.rglob("*.json"):
        try:
            d = json.loads(p.read_text())
        except (OSError, ValueError, UnicodeDecodeError):
            continue
        if not isinstance(d, dict):
            continue
        envelope = envelope_for(d, "collector")
        if envelope is not None:
            yield from envelope_records(campaign, p, envelope)
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
        if d.get("capability_id") == "help_binding" and isinstance(
            d.get("bindings"), list
        ):
            for index, binding in enumerate(d["bindings"]):
                if not isinstance(binding, dict) or binding.get("nearest_miss") is None:
                    continue
                observation = binding.get("observation", {})
                stderr = (
                    observation.get("stderr", {})
                    if isinstance(observation, dict)
                    else {}
                )
                stdout = (
                    observation.get("stdout", {})
                    if isinstance(observation, dict)
                    else {}
                )
                yield {
                    "record_id": evidence_record_id(campaign, p, "c2", index),
                    "source_run": str(p),
                    "claim": binding.get("option", ""),
                    "kind": "c2",
                    "judgement": "matched" if binding.get("ok", False) else "violation",
                    "nearest_miss": binding.get("nearest_miss"),
                    "observation": stderr.get("text") or stdout.get("text"),
                    "direction": binding.get("direction"),
                }
        if d.get("capability_id") == "cli_probe" and isinstance(
            d.get("output_claims"), list
        ):
            for index, claim in enumerate(d["output_claims"]):
                if not isinstance(claim, dict) or claim.get("nearest_miss") is None:
                    continue
                observation = claim.get("observation", {})
                stdout = (
                    observation.get("stdout", {})
                    if isinstance(observation, dict)
                    else {}
                )
                yield {
                    "record_id": evidence_record_id(campaign, p, "c3", index),
                    "source_run": str(p),
                    "claim": claim.get("claim", ""),
                    "kind": "c3",
                    "judgement": "matched"
                    if claim.get("matched", False)
                    else "violation",
                    "nearest_miss": claim.get("nearest_miss"),
                    "observation": stdout.get("text", claim.get("nearest_miss")),
                    "source": claim.get("source"),
                }


def append(campaigns, store=STORE):
    buckets = {"e2": [], "i2": []}
    for campaign in campaigns:
        for r in records(campaign):
            buckets.setdefault(r["kind"], []).append(r)
    store.mkdir(parents=True, exist_ok=True)
    total = 0
    for kind, rows in buckets.items():
        out = store / kind / "records.jsonl"
        out.parent.mkdir(parents=True, exist_ok=True)
        existing = (
            {
                record.get("record_id", record.get("source_run"))
                for record in map(json.loads, out.read_text().splitlines())
            }
            if out.exists()
            else set()
        )
        with out.open("a", encoding="utf-8") as f:
            for r in rows:
                key = r.get("record_id", r["source_run"])
                if key in existing:
                    continue
                f.write(json.dumps(r, ensure_ascii=False) + "\n")
                existing.add(key)
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
