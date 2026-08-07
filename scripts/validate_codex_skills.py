#!/usr/bin/env python3
"""Validate repository-local Codex skills and their UI metadata."""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

import yaml

REPO_ROOT = Path(__file__).resolve().parents[1]
SKILLS_ROOT = REPO_ROOT / ".agents" / "skills"
NAME_RE = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
FRONTMATTER_RE = re.compile(r"^---\n(.*?)\n---(?:\n|$)", re.DOTALL)
REFERENCE_LINK_RE = re.compile(r"\]\((references/[^)]+)\)")
ALLOWED_FRONTMATTER = {"name", "description", "license", "allowed-tools", "metadata"}


def load_yaml_mapping(text: str, source: Path) -> dict[str, object]:
    try:
        value = yaml.safe_load(text)
    except yaml.YAMLError as exc:
        raise ValueError(f"{source}: invalid YAML: {exc}") from exc
    if not isinstance(value, dict):
        raise ValueError(f"{source}: expected a YAML mapping")
    return value


def require_nonempty_string(mapping: dict[str, object], key: str, source: Path) -> str:
    value = mapping.get(key)
    if not isinstance(value, str) or not value.strip():
        raise ValueError(f"{source}: {key!r} must be a non-empty string")
    return value.strip()


def validate_skill(skill_dir: Path) -> list[str]:
    errors: list[str] = []
    skill_md = skill_dir / "SKILL.md"
    if not skill_md.is_file():
        return [f"{skill_dir}: SKILL.md not found"]

    content = skill_md.read_text(encoding="utf-8")
    match = FRONTMATTER_RE.match(content)
    if match is None:
        return [f"{skill_md}: invalid or missing YAML frontmatter"]

    try:
        frontmatter = load_yaml_mapping(match.group(1), skill_md)
        unexpected = set(frontmatter) - ALLOWED_FRONTMATTER
        if unexpected:
            errors.append(f"{skill_md}: unexpected frontmatter keys: {sorted(unexpected)}")
        name = require_nonempty_string(frontmatter, "name", skill_md)
        description = require_nonempty_string(frontmatter, "description", skill_md)
        if not NAME_RE.fullmatch(name) or len(name) > 64:
            errors.append(f"{skill_md}: invalid skill name {name!r}")
        if name != skill_dir.name:
            errors.append(f"{skill_md}: name {name!r} does not match directory {skill_dir.name!r}")
        if len(description) > 1024 or "<" in description or ">" in description:
            errors.append(f"{skill_md}: invalid description")
        if "TODO" in content:
            errors.append(f"{skill_md}: unresolved TODO marker")
        for relative_reference in REFERENCE_LINK_RE.findall(content):
            if not (skill_dir / relative_reference).is_file():
                errors.append(f"{skill_md}: missing linked reference {relative_reference!r}")
    except ValueError as exc:
        errors.append(str(exc))
        name = skill_dir.name

    metadata_path = skill_dir / "agents" / "openai.yaml"
    if not metadata_path.is_file():
        errors.append(f"{metadata_path}: file not found")
        return errors

    try:
        metadata = load_yaml_mapping(metadata_path.read_text(encoding="utf-8"), metadata_path)
        interface = metadata.get("interface")
        if not isinstance(interface, dict):
            raise ValueError(f"{metadata_path}: 'interface' must be a mapping")
        require_nonempty_string(interface, "display_name", metadata_path)
        short_description = require_nonempty_string(
            interface, "short_description", metadata_path
        )
        if not 25 <= len(short_description) <= 64:
            errors.append(
                f"{metadata_path}: short_description must contain 25 to 64 characters"
            )
        default_prompt = require_nonempty_string(interface, "default_prompt", metadata_path)
        if f"${name}" not in default_prompt:
            errors.append(f"{metadata_path}: default_prompt must mention ${name}")
        policy = metadata.get("policy")
        if policy is not None:
            if not isinstance(policy, dict) or not isinstance(
                policy.get("allow_implicit_invocation"), bool
            ):
                errors.append(
                    f"{metadata_path}: policy.allow_implicit_invocation must be boolean"
                )
    except ValueError as exc:
        errors.append(str(exc))

    return errors


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--tracked-only",
        action="store_true",
        help="validate only skills already tracked by Git",
    )
    return parser.parse_args(argv)


def discover_skill_dirs(*, tracked_only: bool) -> list[Path]:
    if not tracked_only:
        return sorted(path for path in SKILLS_ROOT.iterdir() if path.is_dir())

    tracked_files = subprocess.run(
        ["git", "ls-files", "--", ".agents/skills/*/SKILL.md"],
        cwd=REPO_ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.splitlines()
    return sorted({(REPO_ROOT / relative).parent for relative in tracked_files})


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    if not SKILLS_ROOT.is_dir():
        print(f"Skills directory not found: {SKILLS_ROOT}", file=sys.stderr)
        return 1

    skill_dirs = discover_skill_dirs(tracked_only=args.tracked_only)
    if not skill_dirs:
        print(f"No skills found under {SKILLS_ROOT}", file=sys.stderr)
        return 1

    errors = [error for skill_dir in skill_dirs for error in validate_skill(skill_dir)]
    if errors:
        print("Codex skill validation failed:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print(f"Validated {len(skill_dirs)} Codex skills.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
