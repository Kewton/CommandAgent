"""Durable attempt reservations; no browser bootstrap or reset lives here."""

from __future__ import annotations

import copy
import fcntl
import os
import time
import uuid
from contextlib import contextmanager
from importlib.metadata import PackageNotFoundError, version
from pathlib import Path
from urllib.parse import urlsplit

from .evidence import (
    CONTRACT,
    MAX_AGE_SECONDS,
    PreflightError,
    diagnose,
    digest,
    entry_point,
    file_hash,
    read_json,
    write_new,
)


def validate_request(request):
    if not isinstance(request, dict):
        raise PreflightError("Request must be a JSON object")
    if (
        not isinstance(request.get("session_id"), str)
        or not request["session_id"].strip()
    ):
        raise PreflightError("A current session ID is required")
    if request.get("method") not in ("browser-plugin", "isolated-playwright"):
        raise PreflightError("Unknown automation method")
    url = urlsplit(request.get("target_url", ""))
    if (
        url.scheme not in ("http", "https")
        or not url.hostname
        or url.username
        or url.password
    ):
        raise PreflightError(
            "An HTTP(S) target without embedded credentials is required"
        )
    action = request.get("action", {})
    if not isinstance(action, dict) or not isinstance(
        request.get("conditions", {}), dict
    ):
        raise PreflightError("Action and conditions must be JSON objects")
    if (
        any(
            not isinstance(action.get(k), str) or not action[k].strip()
            for k in ("role", "name", "before", "after")
        )
        or action["before"] == action["after"]
    ):
        raise PreflightError(
            "Declare a harmless named action and distinct visible before/after text"
        )
    if (
        not isinstance(request.get("authorization"), str)
        or not request["authorization"].strip()
    ):
        raise PreflightError("Record authorization for the target and harmless action")
    if request["method"] == "isolated-playwright":
        if url.hostname not in ("127.0.0.1", "localhost", "::1"):
            raise PreflightError(
                "Isolated fallback is restricted to local unauthenticated evaluation"
            )
        if request.get("isolated_fallback_authorized") is not True:
            raise PreflightError(
                "Explicit isolated-profile fallback authorization is required"
            )


def snapshot(request):
    validate_request(request)
    entry = entry_point(request)
    # Explanatory prose is deliberately excluded: a new retry reason or changed
    # authorization wording cannot grant more identical attempts.
    conditions = {
        k: request.get(k)
        for k in (
            "session_id",
            "method",
            "target_url",
            "action",
            "conditions",
            "executable",
        )
    }
    conditions["entry_point"] = {
        k: v for k, v in entry.items() if k != "installed_versions"
    }
    if request["method"] == "isolated-playwright":
        executable = Path(request.get("executable", ""))
        conditions["executable_sha256"] = (
            file_hash(executable) if executable.is_file() else None
        )
        try:
            conditions["playwright_version"] = version("playwright")
        except PackageNotFoundError:
            conditions["playwright_version"] = None
    recovery = request.get("recovery_evidence")
    if recovery:
        conditions["recovery_evidence_sha256"] = file_hash(recovery)
    return entry, digest(conditions)


@contextmanager
def locked(root):
    root = Path(root)
    root.mkdir(parents=True, exist_ok=True)
    with (root / ".lock").open("a") as stream:
        fcntl.flock(stream, fcntl.LOCK_EX)
        yield root


def save_state(root, state):
    temp = root / f".state-{uuid.uuid4().hex}.json"
    write_new(temp, state)
    os.replace(temp, root / "state.json")


def state_at(root, session_id):
    path = root / "state.json"
    state = (
        read_json(path)
        if path.exists()
        else {"contract": CONTRACT, "session_id": session_id, "attempts": []}
    )
    if state.get("contract") != CONTRACT or state.get("session_id") != session_id:
        raise PreflightError("Ledger belongs to another contract/session")
    return state


def prepare(root, request, reason="", now=None):
    now = time.time() if now is None else now
    request = copy.deepcopy(request)
    entry, fingerprint = snapshot(request)
    with locked(root) as root:
        state = state_at(root, request["session_id"])
        attempts = state["attempts"]
        latest = attempts[-1] if attempts else None
        if (
            latest
            and latest["fingerprint"] == fingerprint
            and latest["status"] == "passed"
        ):
            report = root / latest["id"] / "report.json"
            if now - read_json(report)["finished_at"] <= MAX_AGE_SECONDS:
                validate_report(report, request, now=now)
                return {"decision": "reuse", "report": str(report.resolve())}
            reason = (
                reason
                or "Previous operational evidence expired; recheck without resetting connection"
            )
        matching = [a for a in attempts if a["fingerprint"] == fingerprint]
        if any(a["status"] == "pending" for a in matching):
            raise PreflightError(
                "Attempt already reserved; finish it or record its interruption, do not retry"
            )
        if sum(a["status"] == "blocked" for a in matching) >= 2:
            raise PreflightError(
                "Repeated failure suppressed: unchanged conditions already failed twice"
            )
        changed = latest is not None and latest["fingerprint"] != fingerprint
        if changed and not reason.strip():
            raise PreflightError("Changed conditions require a saved retry reason")
        attempt_id = uuid.uuid4().hex
        directory = root / attempt_id
        directory.mkdir()
        ticket = {
            "contract": CONTRACT,
            "id": attempt_id,
            "request": request,
            "entry_point": entry,
            "fingerprint": fingerprint,
            "created_at": now,
            "previous_fingerprint": latest["fingerprint"] if latest else None,
            "retry_reason": reason
            or ("Reproduce failure once" if matching else "Initial check"),
        }
        write_new(directory / "ticket.json", ticket)
        attempts.append(
            {
                "id": attempt_id,
                "fingerprint": fingerprint,
                "status": "pending",
                "ticket_sha256": file_hash(directory / "ticket.json"),
            }
        )
        save_state(root, state)
        return {
            "decision": "probe",
            "ticket": str((directory / "ticket.json").resolve()),
            "operation_allowed": entry["status"] in ("present", "not_applicable"),
        }


def finish(ticket_path, observation, now=None):
    now = time.time() if now is None else now
    ticket_path = Path(ticket_path).resolve()
    directory = ticket_path.parent
    ticket = read_json(ticket_path)
    if not isinstance(observation, dict):
        raise PreflightError("Observation must be a JSON object")
    with locked(directory.parent) as root:
        state = state_at(root, ticket["request"]["session_id"])
        attempts = [a for a in state["attempts"] if a["id"] == ticket["id"]]
        if len(attempts) != 1 or attempts[0]["status"] != "pending":
            raise PreflightError("Only a reserved, unfinished attempt may be completed")
        if attempts[0]["fingerprint"] != ticket["fingerprint"] or attempts[0][
            "ticket_sha256"
        ] != file_hash(ticket_path):
            raise PreflightError("Ticket conditions changed after reservation")
        result = diagnose(ticket, observation, directory)
        report = {**ticket, "finished_at": now, "observation": observation, **result}
        write_new(directory / "report.json", report)
        attempts[0].update(
            status=result["status"], report_sha256=file_hash(directory / "report.json")
        )
        save_state(root, state)
        return directory / "report.json"


def claim_probe(ticket_path):
    """Reserve adapter execution once, including direct Python API callers."""
    path = Path(ticket_path).resolve()
    ticket = read_json(path)
    with locked(path.parent.parent) as root:
        state = state_at(root, ticket["request"]["session_id"])
        reservation = next(
            (a for a in state["attempts"] if a["id"] == ticket["id"]), {}
        )
        if reservation.get("status") != "pending" or reservation.get(
            "ticket_sha256"
        ) != file_hash(path):
            raise PreflightError(
                "An intact pending attempt is required before browser work"
            )
        write_new(path.parent / "probe-started.json", {"ticket_id": ticket["id"]})


def validate_report(path, request, now=None):
    now = time.time() if now is None else now
    path = Path(path).resolve()
    report = read_json(path)
    if report.get("contract") != CONTRACT:
        raise PreflightError("Unknown report contract")
    _, fingerprint = snapshot(request)
    if fingerprint != report["fingerprint"] or report["request"] != request:
        raise PreflightError("Conditions changed; create a new campaign/preflight")
    age = now - report["finished_at"]
    if (
        not 0 <= age <= MAX_AGE_SECONDS
        or not 0 <= now - report["created_at"] <= MAX_AGE_SECONDS
    ):
        raise PreflightError("Browser evidence is stale or future-dated")
    state = state_at(path.parent.parent, request["session_id"])
    latest = state["attempts"][-1]
    if (
        latest["id"] != report["id"]
        or latest["status"] != "passed"
        or latest.get("report_sha256") != file_hash(path)
    ):
        raise PreflightError(
            "Report is not the latest intact successful session outcome"
        )
    result = diagnose(report, report["observation"], path.parent)
    if result["status"] != "passed" or any(
        report.get(k) != v for k, v in result.items()
    ):
        raise PreflightError("Operational evidence is insufficient or has changed")
    return report
