"""Immutable, opt-in campaign pinning and guarded command launch."""

from __future__ import annotations

import subprocess
import time
from pathlib import Path

from .evidence import CONTRACT, PreflightError, file_hash, read_json, write_new
from .session import locked, validate_report


def freeze(directory, campaign_id, report_path, request, now=None):
    if not campaign_id.strip():
        raise PreflightError("A new campaign ID is required")
    report = validate_report(report_path, request, now=now)
    directory = Path(directory)
    # Never overwrite an existing campaign, even one containing a different harness.
    directory.mkdir(parents=True, exist_ok=False)
    manifest = {
        "contract": CONTRACT,
        "campaign_id": campaign_id,
        "request": request,
        "conditions": report["observation"]["conditions"],
        "required_viewports": request["required_viewports"],
        "method": request["method"],
        "report": str(Path(report_path).resolve()),
        "report_sha256": file_hash(report_path),
        "frozen_at": time.time() if now is None else now,
        "plugin_permanent_repair": "not_established",
    }
    path = directory / "browser-manifest.json"
    write_new(path, manifest)
    write_new(directory / "manifest-pin.json", {"sha256": file_hash(path)})
    return path


def gate(manifest_path, request, now=None):
    path = Path(manifest_path)
    if read_json(path.parent / "manifest-pin.json")["sha256"] != file_hash(path):
        raise PreflightError("Frozen manifest changed; use a new campaign")
    manifest = read_json(path)
    if manifest.get("contract") != CONTRACT or manifest.get("request") != request:
        raise PreflightError("Current request differs from the frozen campaign")
    if manifest.get("required_viewports") != request.get("required_viewports"):
        raise PreflightError(
            "Required evaluation viewports differ from the frozen campaign"
        )
    if file_hash(manifest["report"]) != manifest["report_sha256"]:
        raise PreflightError("Frozen Browser report changed")
    report = validate_report(manifest["report"], request, now=now)
    if manifest["conditions"] != report["observation"]["conditions"]:
        raise PreflightError(
            "Frozen browser conditions differ from observed conditions"
        )
    return manifest


def launch(manifest_path, request, command, now=None):
    if not command:
        raise PreflightError("A command to guard is required")
    directory = Path(manifest_path).parent
    with locked(directory):
        manifest = gate(manifest_path, request, now=now)
        # Also reserve under the session lock so another attempt cannot begin
        # between the final evidence check and the durable launch decision.
        with locked(Path(manifest["report"]).parent.parent):
            gate(manifest_path, request, now=now)
            write_new(
                directory / "started.json",
                {
                    "campaign_id": manifest["campaign_id"],
                    "manifest_sha256": file_hash(manifest_path),
                    "started_at": time.time() if now is None else now,
                },
            )
    # No shell, daemon lifecycle management, model invocation or generated command.
    # The caller explicitly supplies the authorized workload.
    result = subprocess.run(command, check=False)
    write_new(directory / "exit.json", {"returncode": result.returncode})
    return result.returncode
