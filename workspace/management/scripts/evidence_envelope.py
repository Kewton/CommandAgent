"""Shared additive evidence-envelope reader and family coverage guard."""

from __future__ import annotations

import json
from pathlib import Path

import tomllib

ROOT = Path(__file__).resolve().parents[1]
REGISTRY_PATH = ROOT / "evidence-families.toml"
REQUIRED_FIELDS = {
    "envelope_version",
    "family",
    "kind",
    "epoch",
    "claims",
    "nearest_miss",
    "source_refs",
}

# Each map is an executable adapter boundary, not documentation: envelope_for
# rejects a family absent from the calling consumer's map. A registry addition
# therefore requires all three transverse consumers to make an explicit choice.
CONSUMER_ADAPTERS = {
    "collector": {
        "E": "calibration_claims",
        "F": "metadata_only",
        "I": "calibration_claims",
        "C": "calibration_claims",
        "N": "calibration_claims",
        "T": "calibration_claims",
        "tool_parse": "calibration_claims",
        "circle": "metadata_only",
        "workflow": "metadata_only",
        "score": "metadata_only",
    },
    "sheet": {
        "E": "summary_and_legacy_detail",
        "F": "summary_and_legacy_detail",
        "I": "summary_and_legacy_detail",
        "C": "summary_and_legacy_detail",
        "N": "summary_and_legacy_detail",
        "T": "summary_and_legacy_detail",
        "tool_parse": "summary_and_legacy_detail",
        "circle": "summary_and_legacy_detail",
        "workflow": "summary_and_legacy_detail",
        "score": "summary_and_legacy_detail",
    },
    "classify": {
        "E": "exclude_envelope_from_terminal_text",
        "F": "exclude_envelope_from_terminal_text",
        "I": "exclude_envelope_from_terminal_text",
        "C": "exclude_envelope_from_terminal_text",
        "N": "exclude_envelope_from_terminal_text",
        "T": "exclude_envelope_from_terminal_text",
        "tool_parse": "exclude_envelope_from_terminal_text",
        "circle": "exclude_envelope_from_terminal_text",
        "workflow": "exclude_envelope_from_terminal_text",
        "score": "exclude_envelope_from_terminal_text",
    },
}


class EnvelopeError(ValueError):
    """An emitted envelope is present but invalid or unsupported."""


def registered_families(path=REGISTRY_PATH):
    values = tomllib.loads(Path(path).read_text(encoding="utf-8")).get("families", [])
    if not isinstance(values, list) or not all(isinstance(value, str) for value in values):
        raise EnvelopeError("evidence family registry must contain a string list")
    if len(values) != len(set(values)):
        raise EnvelopeError("evidence family registry contains duplicates")
    return tuple(values)


def guard_errors(registry=None, adapters=CONSUMER_ADAPTERS):
    expected = set(registry or registered_families())
    errors = []
    for consumer in ("collector", "sheet", "classify"):
        actual = set(adapters.get(consumer, {}))
        missing = sorted(expected - actual)
        dead = sorted(actual - expected)
        if missing:
            errors.append(f"{consumer}: missing family adapters: {', '.join(missing)}")
        if dead:
            errors.append(f"{consumer}: dead family adapters: {', '.join(dead)}")
    return errors


def envelope_for(document, consumer):
    """Return a validated envelope, or None for a historical legacy document."""
    if not isinstance(document, dict) or "evidence_envelope" not in document:
        return None
    if consumer not in CONSUMER_ADAPTERS:
        raise EnvelopeError(f"unknown evidence consumer: {consumer}")
    envelope = document["evidence_envelope"]
    if not isinstance(envelope, dict):
        raise EnvelopeError("evidence_envelope must be an object")
    missing = sorted(REQUIRED_FIELDS - set(envelope))
    if missing:
        raise EnvelopeError(f"evidence_envelope missing fields: {', '.join(missing)}")
    family = envelope.get("family")
    if family not in CONSUMER_ADAPTERS[consumer]:
        raise EnvelopeError(f"{consumer} has no adapter for evidence family: {family}")
    if envelope.get("envelope_version") != 1:
        raise EnvelopeError(
            f"unsupported evidence envelope version: {envelope.get('envelope_version')}"
        )
    if not isinstance(envelope.get("kind"), str) or not envelope["kind"]:
        raise EnvelopeError("evidence envelope kind must be a non-empty string")
    if not isinstance(envelope.get("epoch"), int):
        raise EnvelopeError("evidence envelope epoch must be an integer")
    for field in ("claims", "nearest_miss", "source_refs"):
        if not isinstance(envelope.get(field), list):
            raise EnvelopeError(f"evidence envelope {field} must be a list")
    return envelope


def legacy_view(document, consumer):
    """Strip only the additive envelope after validating its consumer adapter."""
    envelope = envelope_for(document, consumer)
    if envelope is None:
        return document
    return {key: value for key, value in document.items() if key != "evidence_envelope"}


def classification_text(text):
    """Exclude additive envelope copies without changing historical text."""
    try:
        document = json.loads(text)
    except ValueError:
        return text
    if not isinstance(document, dict) or envelope_for(document, "classify") is None:
        return text
    return json.dumps(legacy_view(document, "classify"), ensure_ascii=False)


def classification_jsonl_text(text):
    lines = []
    changed = False
    for line in text.splitlines():
        replacement = classification_text(line)
        changed = changed or replacement != line
        lines.append(replacement)
    return "\n".join(lines) if changed else text
