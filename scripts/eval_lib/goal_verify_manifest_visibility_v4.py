from __future__ import annotations

import hashlib
import json
from typing import Any

POLICY_ID = "commandagent.goal_verify.candidate_manifest.source_config_v1"

_HIDDEN_DIRECTORY_PARTS = {
    ".cache",
    ".mypy_cache",
    ".next",
    ".nox",
    ".pytest_cache",
    ".ruff_cache",
    ".tox",
    ".turbo",
    ".venv",
    "__pycache__",
    "build",
    "coverage",
    "dist",
    "htmlcov",
    "out",
    "venv",
}
_HIDDEN_FILE_NAMES = {
    ".coverage",
    "coverage.xml",
    "lcov.info",
}
_HIDDEN_FILE_SUFFIXES = {
    ".gcda",
    ".gcno",
    ".profraw",
    ".pyc",
    ".pyo",
}


def project_candidate_visible_manifest(manifest: dict[str, Any]) -> dict[str, Any]:
    """Keep integrity identity while hiding generated workspace detail from the LLM."""
    entries = manifest.get("entries")
    if not isinstance(entries, list):
        raise TypeError("workspace manifest entries must be a list")
    visible = [
        _candidate_entry(entry)
        for entry in entries
        if isinstance(entry, dict) and _candidate_entry_visible(entry)
    ]
    encoded = json.dumps(
        visible,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")
    return {
        "schema_version": manifest["schema_version"],
        "snapshot_sha256": manifest["snapshot_sha256"],
        "candidate_visibility_policy": POLICY_ID,
        "candidate_entries_sha256": hashlib.sha256(encoded).hexdigest(),
        "entries": visible,
    }


def _candidate_entry(entry: dict[str, Any]) -> dict[str, Any]:
    return {
        key: value
        for key, value in entry.items()
        if key in {"path", "kind", "sha256", "size"}
    }


def _candidate_entry_visible(entry: dict[str, Any]) -> bool:
    raw_path = entry.get("path")
    if not isinstance(raw_path, str) or not raw_path:
        return False
    parts = raw_path.split("/")
    if any(part in _HIDDEN_DIRECTORY_PARTS for part in parts[:-1]):
        return False
    name = parts[-1]
    if entry.get("kind") == "symlink" and name in _HIDDEN_DIRECTORY_PARTS:
        return False
    if name in _HIDDEN_FILE_NAMES:
        return False
    return not any(name.endswith(suffix) for suffix in _HIDDEN_FILE_SUFFIXES)
