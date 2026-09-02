from __future__ import annotations

import hashlib
import json
import shutil
import subprocess
from pathlib import Path
from typing import Any


def load_workspace_registry(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict) or not isinstance(value.get("workspaces"), list):
        raise TypeError("invalid v3 workspace registry")
    return value


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def workspace_file_hashes(root: Path, workspace: dict[str, Any]) -> dict[str, str]:
    base = (root / workspace["root"]).resolve()
    hashes = {}
    for relative in workspace["tracked_files"]:
        path = (base / relative).resolve()
        if not path.is_relative_to(base) or not path.is_file():
            raise ValueError(f"workspace file missing or escaping root: {relative}")
        hashes[relative] = sha256_file(path)
    return hashes


def validate_workspace_registry(
    *, root: Path, registry: dict[str, Any], require_frozen: bool = False
) -> list[str]:
    errors: list[str] = []
    seen = set()
    for workspace in registry["workspaces"]:
        case_id = workspace.get("case_id")
        if case_id in seen:
            errors.append(f"duplicate_workspace:{case_id}")
            continue
        seen.add(case_id)
        try:
            actual = workspace_file_hashes(root, workspace)
        except (KeyError, OSError, ValueError) as error:
            errors.append(f"workspace_invalid:{case_id}:{error}")
            continue
        frozen = workspace.get("frozen_file_sha256")
        if require_frozen and frozen != actual:
            errors.append(f"workspace_hash_mismatch:{case_id}")
    return errors


def prepare_workspace_stage(
    *,
    root: Path,
    workspace: dict[str, Any],
    stage: str,
    destination: Path,
    provisioned_root: Path | None = None,
) -> Path:
    if stage not in workspace["stages"]:
        raise ValueError(f"unknown workspace stage: {stage}")
    source = (root / workspace["root"] / stage).resolve()
    fixture_root = (root / workspace["root"]).resolve()
    if not source.is_relative_to(fixture_root) or not source.is_dir():
        raise ValueError("workspace stage is missing or escapes fixture root")
    if destination.exists():
        shutil.rmtree(destination)
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copytree(source, destination)
    provisioning = workspace.get("provisioning", {})
    if provisioning.get("mode") == "vendored_tarball":
        if provisioned_root is None:
            raise ValueError("vendored provisioning root is required")
        _restore_tarball(
            provisioned_root / provisioning["tarball"],
            destination,
            expected_sha256=provisioning["sha256"],
        )
        _link_browser_tool(provisioning, provisioned_root, destination)
    return destination.resolve()


def validate_provisioning(
    registry: dict[str, Any], provisioned_root: Path | None
) -> list[str]:
    errors = []
    for workspace in registry["workspaces"]:
        provisioning = workspace.get("provisioning", {})
        if provisioning.get("mode") != "vendored_tarball":
            continue
        if provisioned_root is None:
            errors.append(f"provisioning_root_missing:{workspace['case_id']}")
            continue
        archive = provisioned_root / provisioning["tarball"]
        if not archive.is_file():
            errors.append(f"provisioning_tarball_missing:{workspace['case_id']}")
        elif sha256_file(archive) != provisioning.get("sha256"):
            errors.append(f"provisioning_hash_mismatch:{workspace['case_id']}")
        browser = provisioning.get("browser_tool")
        if browser:
            source = provisioned_root / browser["source"]
            if not source.is_dir():
                errors.append(f"browser_tool_missing:{workspace['case_id']}")
            elif not _browser_tool_hashes_match(source, browser):
                errors.append(f"browser_tool_hash_mismatch:{workspace['case_id']}")
    return errors


def _restore_tarball(archive: Path, destination: Path, *, expected_sha256: str) -> None:
    if not archive.is_file() or sha256_file(archive) != expected_sha256:
        raise ValueError(f"provisioning tarball missing or hash mismatch: {archive.name}")
    listing = subprocess.run(
        ["tar", "--zstd", "-tf", str(archive)],
        text=True,
        capture_output=True,
        check=True,
    ).stdout.splitlines()
    if not listing or any(
        Path(member).is_absolute() or ".." in Path(member).parts for member in listing
    ):
        raise ValueError("unsafe provisioning archive member")
    subprocess.run(
        ["tar", "--zstd", "-xf", str(archive), "-C", str(destination)],
        check=True,
    )


def _link_browser_tool(
    provisioning: dict[str, Any], provisioned_root: Path, destination: Path
) -> None:
    browser = provisioning.get("browser_tool")
    if not browser:
        return
    source = (provisioned_root / browser["source"]).resolve()
    if not source.is_dir() or not _browser_tool_hashes_match(source, browser):
        raise ValueError("browser tool missing or hash mismatch")
    relative = Path(browser["workspace_link"])
    if relative.is_absolute() or ".." in relative.parts:
        raise ValueError("unsafe browser workspace link")
    link = destination / relative
    link.parent.mkdir(parents=True, exist_ok=True)
    link.symlink_to(source)


def _browser_tool_hashes_match(source: Path, browser: dict[str, Any]) -> bool:
    expected = browser.get("files_sha256")
    if not isinstance(expected, dict) or not expected:
        return False
    actual_files = {path.name for path in source.iterdir() if path.is_file()}
    if actual_files != set(expected):
        return False
    return all(sha256_file(source / name) == digest for name, digest in expected.items())


def workspace_by_case(registry: dict[str, Any]) -> dict[str, dict[str, Any]]:
    return {row["case_id"]: row for row in registry["workspaces"]}
