"""Fail-closed evaluation of Browser observations (never HTTP-as-GUI)."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

CONTRACT = "browser-preflight-v1"
MAX_AGE_SECONDS = 900


class PreflightError(ValueError):
    pass


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True).encode()).hexdigest()


def file_hash(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def read_json(path):
    return json.loads(Path(path).read_text())


def write_new(path, value):
    with Path(path).open("x") as stream:
        json.dump(value, stream, indent=2, ensure_ascii=False)
        stream.write("\n")


def entry_point(request):
    """The caller attests to session instructions; inventory never selects a version."""
    if request["method"] != "browser-plugin":
        return {"status": "not_applicable"}
    instruction = request.get("session_instruction", {})
    if not isinstance(instruction, dict):
        instruction = {}
    source = {
        k: instruction.get(k)
        for k in ("session_id", "skill_path", "entry_point", "sha256")
    }
    result = {"status": "unknown", "source": source, "installed_versions": []}
    if source.get("session_id") != request["session_id"]:
        return result
    skill = Path(source.get("skill_path") or "")
    entry = Path(source.get("entry_point") or "")
    if not skill.is_absolute() or not entry.is_absolute() or len(skill.parents) < 3:
        return result
    root = skill.parents[2]
    result.update(referenced_version=root.name, referenced_path=str(entry))
    if root.parent.is_dir():
        result["installed_versions"] = sorted(
            p.name
            for p in root.parent.iterdir()
            if (p / "scripts/browser-client.mjs").is_file()
        )
    result["file_exists"] = entry.is_file()
    if not skill.is_file():
        result["error"] = f"Session instruction file missing: {skill}"
        return result
    result["skill_sha256"] = file_hash(skill)
    if result["skill_sha256"] != source.get("sha256"):
        result["error"] = "Session instruction hash mismatch"
        return result
    if entry != root / "scripts/browser-client.mjs" or (
        "scripts/browser-client.mjs" not in skill.read_text()
    ):
        result["error"] = "Entry point is not grounded in the supplied session skill"
        return result
    result["status"] = "present" if entry.is_file() else "missing"
    if entry.is_file():
        result["entry_sha256"] = file_hash(entry)
    else:
        result["error"] = f"Session entry point missing: {entry}"
    return result


def artifact(root, name, kind):
    """Only accept saved, nonempty artifacts inside this attempt directory."""
    if not isinstance(name, str) or not name:
        raise PreflightError(f"Missing {kind} artifact")
    root = Path(root).resolve()
    path = (root / name).resolve()
    if not path.is_relative_to(root) or not path.is_file() or path.stat().st_size == 0:
        raise PreflightError(f"Invalid {kind} artifact: {name}")
    result = {"path": name, "sha256": file_hash(path)}
    if kind == "image":
        from PIL import Image

        with Image.open(path) as image:
            image.verify()
        with Image.open(path) as image:
            image.load()
            if image.width < 2 or image.height < 2:
                raise PreflightError("Screenshot has no useful dimensions")
            result.update(format=image.format, width=image.width, height=image.height)
    else:
        if not path.read_text().strip():
            raise PreflightError(f"Empty {kind} artifact")
    return result


def diagnose(ticket, observation, root):
    request = ticket["request"]
    entry = ticket["entry_point"]
    findings = []
    artifacts = {}
    if observation.get("ticket_id") != ticket["id"]:
        findings.append("observation_unbound")
    if entry["status"] == "unknown":
        findings.append("entry_point_unknown")
    elif entry["status"] == "missing":
        findings.append("entry_point_missing")
    http = observation.get("http", {})
    if (
        isinstance(http, dict)
        and http.get("ok") is True
        and not observation.get("before")
    ):
        findings.append("http_only")
    if observation.get("tabs_count") == 0:
        findings.append("empty_tabs")
    if observation.get("error"):
        findings.append(
            "image_failed"
            if observation.get("stage") == "screenshot"
            else "operation_failed"
        )
    if observation.get("cleanup_error"):
        findings.append("cleanup_failed")
    if observation.get("url") != request["target_url"]:
        findings.append("target_unverified")
    for field, kind in (("before", "text"), ("after", "text"), ("screenshot", "image")):
        try:
            artifacts[field] = artifact(root, observation.get(field), kind)
        except (OSError, ValueError, ImportError, SyntaxError) as error:
            findings.append("image_failed" if kind == "image" else "read_failed")
            artifacts[field] = {"error": str(error)}
    if not any("error" in artifacts[key] for key in ("before", "after")):
        before = (Path(root) / observation["before"]).read_text()
        after = (Path(root) / observation["after"]).read_text()
        action = request["action"]
        if (
            observation.get("action") != action
            or action["before"] not in before
            or action["after"] in before
            or action["after"] not in after
            or before == after
        ):
            findings.append("interaction_unverified")
    else:
        findings.append("interaction_unverified")
    actual = observation.get("conditions", {})
    if not isinstance(actual, dict):
        actual = {}
    expected = request.get("conditions", {})
    required = (
        "browser_version",
        "automation_version",
        "headless",
        "viewport",
        "locale",
        "timezone",
    )
    if (
        any(actual.get(k) is None for k in required)
        or any(
            not isinstance(actual.get(k), str)
            or not actual[k].strip()
            or actual[k].strip().lower() == "unknown"
            for k in ("browser_version", "automation_version", "locale", "timezone")
        )
        or type(actual.get("headless")) is not bool
        or not isinstance(actual.get("viewport"), dict)
        or any(
            type(actual["viewport"].get(k)) is not int or actual["viewport"][k] < 2
            for k in ("width", "height")
        )
        or any(actual.get(k) != v for k, v in expected.items())
    ):
        findings.append("conditions_unverified")
    if request["method"] == "isolated-playwright" and (
        observation.get("isolated_profile") is not True
        or observation.get("owned_resources_closed") is not True
    ):
        findings.append("isolation_unverified")
    # An empty tab list is diagnostic, not an invalid connection. A new owned tab
    # can still demonstrate all four capabilities on that same connection.
    blockers = sorted(set(findings) - {"empty_tabs"})
    return {
        "status": "passed" if not blockers else "blocked",
        "findings": sorted(set(findings)),
        "blockers": blockers,
        "artifacts": artifacts,
        "scope": "observed_session_only",
        "automatic_diagnosis": "implemented",
        "plugin_permanent_repair": "not_established",
    }
