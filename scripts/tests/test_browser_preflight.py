from __future__ import annotations

import copy
import json
import subprocess
import sys
from contextlib import nullcontext
from pathlib import Path
from types import ModuleType, SimpleNamespace

import pytest
from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from browser_preflight import playwright_probe
from browser_preflight.campaign import freeze, gate, launch
from browser_preflight.evidence import (
    PreflightError,
    diagnose,
    entry_point,
    file_hash,
    read_json,
)
from browser_preflight.session import claim_probe, finish, prepare, validate_report

FIXTURES = Path(__file__).parent / "fixtures/browser-preflight"
NOW = 1800000000


@pytest.fixture
def request_data(tmp_path):
    root = tmp_path / "plugins/browser/current"
    skill = root / "skills/control/SKILL.md"
    skill.parent.mkdir(parents=True)
    skill.write_text("Session Browser entry: scripts/browser-client.mjs\n")
    entry = root / "scripts/browser-client.mjs"
    entry.parent.mkdir()
    entry.write_text("// fixture only\n")
    return {
        "session_id": "session-one",
        "method": "browser-plugin",
        "target_url": "http://127.0.0.1:8080/",
        "authorization": "Issue 450: local fixture read, harmless button and screenshot",
        "action": {
            "role": "button",
            "name": "Check browser",
            "before": "Ready for browser check",
            "after": "Browser action confirmed",
        },
        "conditions": {
            "browser_version": "152.0",
            "automation_version": "26.825",
            "headless": False,
            "viewport": {"width": 1000, "height": 800},
            "locale": "en-US",
            "timezone": "Asia/Tokyo",
        },
        "session_instruction": {
            "session_id": "session-one",
            "skill_path": str(skill),
            "sha256": file_hash(skill),
            "entry_point": str(entry),
        },
    }


def receipt(ticket_path):
    ticket = read_json(ticket_path)
    root = Path(ticket_path).parent
    request = ticket["request"]
    (root / "before.txt").write_text(request["action"]["before"])
    (root / "after.txt").write_text(request["action"]["after"])
    Image.new("RGB", (32, 24), "navy").save(root / "screenshot.png")
    return {
        "ticket_id": ticket["id"],
        "url": request["target_url"],
        "tabs_count": 1,
        "before": "before.txt",
        "after": "after.txt",
        "screenshot": "screenshot.png",
        "action": request["action"],
        "conditions": request["conditions"],
    }


def success(root, request):
    ticket = prepare(root, request, now=NOW)["ticket"]
    return finish(ticket, receipt(ticket), now=NOW + 1)


@pytest.mark.parametrize(
    "case", read_json(FIXTURES / "cases.json"), ids=lambda c: c["name"]
)
def test_diagnostic_fixtures(tmp_path, request_data, case):
    ticket_path = prepare(tmp_path / "ledger", request_data, now=NOW)["ticket"]
    ticket = read_json(ticket_path)
    observation = receipt(ticket_path)
    ticket["entry_point"]["status"] = case["entry"]
    change = case["change"]
    if change == "operation":
        observation = {
            "error": "Connection failed in this conversation",
            "stage": "connect",
        }
    elif change == "http_only":
        observation = {"http": {"ok": True, "status": 200}}
    elif change == "empty_only":
        observation = {"tabs_count": 0}
    elif change == "empty_success":
        observation["tabs_count"] = 0
    elif change == "image_error":
        observation.update(error="Screenshot transport failed", stage="screenshot")
        observation.pop("screenshot")
    elif change == "corrupt_image":
        (Path(ticket_path).parent / "screenshot.png").write_bytes(b"not a PNG")
    elif change == "unchanged":
        observation["after"] = observation["before"]
    elif change == "wrong_url":
        observation["url"] = "http://127.0.0.1:8080/login"
    elif change == "unknown_conditions":
        observation["conditions"] = {}
    result = diagnose(ticket, observation, Path(ticket_path).parent)
    if case["expected"] == "passed":
        assert result["status"] == "passed"
    else:
        assert result["status"] == "blocked"
        assert case["expected"] in result["findings"]
    assert result["scope"] == "observed_session_only"
    assert result["plugin_permanent_repair"] == "not_established"


def test_exact_session_entry_never_selects_other_installed_version(
    tmp_path, request_data
):
    old = Path(request_data["session_instruction"]["entry_point"])
    newer = old.parents[2] / "zzz-newest/scripts/browser-client.mjs"
    newer.parent.mkdir(parents=True)
    newer.write_text("exists, but not this session's entry point")
    old.unlink()
    result = entry_point(request_data)
    assert result["status"] == "missing"
    assert result["referenced_path"] == str(old)
    assert result["installed_versions"] == ["zzz-newest"]
    request_data["session_instruction"]["entry_point"] = str(newer)
    assert entry_point(request_data)["status"] == "unknown"


@pytest.mark.parametrize("mutation", ["no_source", "other_session", "modified_skill"])
def test_unproven_entry_is_unknown(request_data, mutation):
    if mutation == "no_source":
        request_data.pop("session_instruction")
    elif mutation == "other_session":
        request_data["session_instruction"]["session_id"] = "someone-else"
    else:
        Path(request_data["session_instruction"]["skill_path"]).write_text("changed")
    assert entry_point(request_data)["status"] == "unknown"


def test_suppression_survives_reopen_and_reason_alone_cannot_bypass(
    tmp_path, request_data
):
    ledger = tmp_path / "ledger"
    for _ in range(2):
        ticket = prepare(ledger, request_data, now=NOW)["ticket"]
        finish(ticket, {"error": "same failure"}, now=NOW + 1)
    with pytest.raises(PreflightError, match="Repeated failure suppressed"):
        prepare(ledger, request_data, reason="please try again", now=NOW + 2)


def test_pending_attempt_cannot_be_retried(tmp_path, request_data):
    prepare(tmp_path, request_data, now=NOW)
    with pytest.raises(PreflightError, match="already reserved"):
        prepare(tmp_path, request_data, now=NOW + 1)


def test_changed_conditions_require_reason_and_old_failures_stay_suppressed(
    tmp_path, request_data
):
    original = copy.deepcopy(request_data)
    for _ in range(2):
        ticket = prepare(tmp_path, original, now=NOW)["ticket"]
        finish(ticket, {"error": "failed"}, now=NOW + 1)
    request_data["conditions"]["viewport"]["width"] = 1200
    with pytest.raises(PreflightError, match="retry reason"):
        prepare(tmp_path, request_data, now=NOW + 2)
    changed = prepare(
        tmp_path,
        request_data,
        reason="Authorized viewport changed to 1200",
        now=NOW + 2,
    )
    ticket = read_json(changed["ticket"])
    assert ticket["retry_reason"] == "Authorized viewport changed to 1200"
    assert ticket["previous_fingerprint"] != ticket["fingerprint"]
    finish(changed["ticket"], {"error": "still fails"}, now=NOW + 3)
    with pytest.raises(PreflightError, match="suppressed"):
        prepare(tmp_path, original, reason="Switch back", now=NOW + 4)


def test_new_recovery_evidence_allows_reasoned_retry(tmp_path, request_data):
    ticket = prepare(tmp_path / "ledger", request_data, now=NOW)["ticket"]
    finish(ticket, {}, now=NOW + 1)
    evidence = tmp_path / "recovery.txt"
    evidence.write_text("A different session succeeded; global outage is unconfirmed")
    request_data["recovery_evidence"] = str(evidence)
    result = prepare(
        tmp_path / "ledger",
        request_data,
        reason="New independent session evidence after documented recovery",
        now=NOW + 2,
    )
    assert result["decision"] == "probe"


def test_fresh_healthy_connection_reuses_evidence_without_probe(tmp_path, request_data):
    report = success(tmp_path, request_data)
    assert prepare(tmp_path, request_data, now=NOW + 2) == {
        "decision": "reuse",
        "report": str(report),
    }
    assert len(read_json(tmp_path / "state.json")["attempts"]) == 1


def test_authorized_fallback_must_be_local_and_isolated(tmp_path, request_data):
    request_data["method"] = "isolated-playwright"
    with pytest.raises(PreflightError, match="authorization"):
        prepare(tmp_path, request_data)
    request_data["isolated_fallback_authorized"] = True
    request_data["target_url"] = "https://example.com/"
    with pytest.raises(PreflightError, match="local unauthenticated"):
        prepare(tmp_path, request_data)
    request_data["target_url"] = "http://localhost:1234/"
    ticket = prepare(tmp_path, request_data, now=NOW)["ticket"]
    report = finish(ticket, receipt(ticket), now=NOW + 1)
    assert "isolation_unverified" in read_json(report)["blockers"]


def test_ticket_cannot_be_edited_or_completed_twice(tmp_path, request_data):
    ticket = prepare(tmp_path, request_data, now=NOW)["ticket"]
    data = read_json(ticket)
    data["request"]["target_url"] += "other"
    Path(ticket).write_text(json.dumps(data))
    with pytest.raises(PreflightError, match="Ticket conditions changed"):
        finish(ticket, {}, now=NOW + 1)


def test_freeze_and_guarded_launch(tmp_path, request_data):
    report = success(tmp_path / "ledger", request_data)
    manifest = freeze(tmp_path / "campaign", "c1", report, request_data, now=NOW + 2)
    assert gate(manifest, request_data, now=NOW + 3)["method"] == "browser-plugin"
    marker = tmp_path / "executed"
    command = [
        sys.executable,
        "-c",
        "from pathlib import Path; import sys; Path(sys.argv[1]).touch()",
        str(marker),
    ]
    assert launch(manifest, request_data, command, now=NOW + 4) == 0
    assert marker.exists()
    with pytest.raises(FileExistsError):
        launch(manifest, request_data, command, now=NOW + 5)
    with pytest.raises(FileExistsError):
        freeze(tmp_path / "campaign", "c1", report, request_data, now=NOW + 5)


@pytest.mark.parametrize(
    "mutation",
    [
        "request",
        "image",
        "text",
        "manifest",
        "report",
        "stale",
        "future",
        "latest_failure",
    ],
)
def test_gate_blocks_changed_or_insufficient_evidence_before_execution(
    tmp_path, request_data, mutation
):
    report = success(tmp_path / "ledger", request_data)
    manifest = freeze(tmp_path / "campaign", "c1", report, request_data, now=NOW + 2)
    now = NOW + 3
    if mutation == "request":
        request_data["conditions"]["headless"] = True
    elif mutation == "image":
        Image.new("RGB", (32, 24), "red").save(report.parent / "screenshot.png")
    elif mutation == "text":
        (report.parent / "before.txt").write_text("tampered")
    elif mutation in ("manifest", "report"):
        path = manifest if mutation == "manifest" else report
        path.write_text(path.read_text() + " ")
    elif mutation == "stale":
        now += 901
    elif mutation == "future":
        now = NOW - 1
    else:
        changed = copy.deepcopy(request_data)
        changed["conditions"]["headless"] = True
        ticket = prepare(
            tmp_path / "ledger", changed, reason="Headless changed", now=NOW + 2
        )["ticket"]
        finish(ticket, {"error": "disconnected"}, now=NOW + 3)
    marker = tmp_path / "must-not-exist"
    command = [
        sys.executable,
        "-c",
        "from pathlib import Path; import sys; Path(sys.argv[1]).touch()",
        str(marker),
    ]
    with pytest.raises(PreflightError):
        launch(manifest, request_data, command, now=now)
    assert not marker.exists()
    assert not (manifest.parent / "started.json").exists()


def test_cannot_freeze_http_only_result(tmp_path, request_data):
    ticket = prepare(tmp_path / "ledger", request_data, now=NOW)["ticket"]
    report = finish(ticket, {"http": {"ok": True}}, now=NOW + 1)
    with pytest.raises(PreflightError):
        freeze(tmp_path / "campaign", "c1", report, request_data, now=NOW + 2)
    assert not (tmp_path / "campaign").exists()


def test_artifact_escape_and_wrong_observation_binding_block(tmp_path, request_data):
    ticket = prepare(tmp_path, request_data, now=NOW)["ticket"]
    observation = receipt(ticket)
    observation.update(before="../outside.txt", ticket_id="previous-attempt")
    report = finish(ticket, observation, now=NOW + 1)
    assert {"read_failed", "observation_unbound"} <= set(read_json(report)["blockers"])


def test_cli_rejects_unknown_request_before_browser_work(tmp_path):
    request = tmp_path / "request.json"
    request.write_text("{}")
    result = subprocess.run(
        [
            sys.executable,
            str(Path(__file__).parents[1] / "browser-preflight.py"),
            "probe",
            "--request",
            str(request),
            "--ledger",
            str(tmp_path / "ledger"),
        ],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 2
    assert json.loads(result.stderr)["status"] == "blocked"
    assert not (tmp_path / "ledger").exists()


def test_expired_success_requires_new_operations(tmp_path, request_data):
    report = success(tmp_path, request_data)
    with pytest.raises(PreflightError, match="stale"):
        validate_report(report, request_data, now=NOW + 1000)
    ticket = prepare(tmp_path, request_data, now=NOW + 1000)["ticket"]
    assert "expired" in read_json(ticket)["retry_reason"]


def test_adapter_reservation_is_consumed_once(tmp_path, request_data):
    ticket = prepare(tmp_path, request_data, now=NOW)["ticket"]
    claim_probe(ticket)
    with pytest.raises(FileExistsError):
        claim_probe(ticket)
    finish(ticket, {"error": "interrupted after adapter reservation"}, now=NOW + 1)
    with pytest.raises(PreflightError, match="pending"):
        claim_probe(ticket)
    with pytest.raises(PreflightError, match="unfinished"):
        finish(ticket, {}, now=NOW + 2)


def test_installing_another_version_does_not_discard_healthy_result(
    tmp_path, request_data
):
    report = success(tmp_path / "ledger", request_data)
    entry = Path(request_data["session_instruction"]["entry_point"])
    unrelated = entry.parents[2] / "newest/scripts/browser-client.mjs"
    unrelated.parent.mkdir(parents=True)
    unrelated.write_text("new unrelated version")
    assert prepare(tmp_path / "ledger", request_data, now=NOW + 2)["report"] == str(
        report
    )


@pytest.mark.parametrize("failure_stage", ["navigate", "screenshot"])
def test_fallback_failure_closes_only_owned_profile(
    tmp_path, request_data, monkeypatch, failure_stage
):
    request_data.update(method="isolated-playwright", isolated_fallback_authorized=True)
    executable = tmp_path / "fake-chrome"
    executable.write_text("test browser executable")
    request_data["executable"] = str(executable)
    ticket = prepare(tmp_path / "ledger", request_data, now=NOW)["ticket"]
    calls = []
    profiles = []
    clicked = False

    def click():
        nonlocal clicked
        clicked = True

    def navigate(*args, **kwargs):
        if failure_stage == "navigate":
            raise RuntimeError("navigation failed")

    def screenshot(**kwargs):
        raise RuntimeError("image failed")

    page = SimpleNamespace(
        url=request_data["target_url"],
        viewport_size=request_data["conditions"]["viewport"],
        goto=navigate,
        screenshot=screenshot,
        evaluate=lambda code: "en-US" if code == "navigator.language" else "Asia/Tokyo",
        get_by_text=lambda *a, **kw: SimpleNamespace(wait_for=lambda **kw: None),
        get_by_role=lambda *a, **kw: SimpleNamespace(
            is_visible=lambda: True, click=click
        ),
        locator=lambda *a: SimpleNamespace(
            inner_text=lambda: request_data["action"]["after" if clicked else "before"]
        ),
    )
    context = SimpleNamespace(
        pages=[],
        browser=SimpleNamespace(version="152.0"),
        new_page=lambda: page,
        set_default_timeout=lambda _: None,
        close=lambda: calls.append("close_owned_context"),
    )

    def launch_context(profile, **kwargs):
        assert Path(profile).is_dir()
        assert kwargs["executable_path"] == str(executable)
        profiles.append(Path(profile))
        return context

    fake_api = ModuleType("playwright.sync_api")
    fake_api.sync_playwright = lambda: nullcontext(
        SimpleNamespace(
            chromium=SimpleNamespace(launch_persistent_context=launch_context)
        )
    )
    monkeypatch.setitem(sys.modules, "playwright", ModuleType("playwright"))
    monkeypatch.setitem(sys.modules, "playwright.sync_api", fake_api)
    monkeypatch.setattr(
        playwright_probe,
        "urlopen",
        lambda *a, **kw: nullcontext(SimpleNamespace(status=200)),
    )
    monkeypatch.setattr(playwright_probe, "version", lambda _: "test-version")
    observation = playwright_probe.probe(ticket)
    assert observation["stage"] == failure_stage
    assert "failed" in observation["error"]
    assert observation["owned_resources_closed"] is True
    assert calls == ["close_owned_context"]
    assert len(profiles) == 1 and not profiles[0].exists()
    with pytest.raises(FileExistsError):
        playwright_probe.probe(ticket)


@pytest.mark.parametrize("value", [None, [], False, " ", "UNKNOWN", 152])
def test_malformed_environment_cannot_freeze(tmp_path, request_data, value):
    ticket = prepare(tmp_path, request_data, now=NOW)["ticket"]
    observation = receipt(ticket)
    observation["conditions"]["browser_version"] = value
    report = finish(ticket, observation, now=NOW + 1)
    assert "conditions_unverified" in read_json(report)["blockers"]


def test_http_url_is_not_an_operational_url_observation(tmp_path, request_data):
    ticket = prepare(tmp_path, request_data, now=NOW)["ticket"]
    report = finish(
        ticket, {"url": request_data["target_url"], "http": {"ok": True}}, now=NOW + 1
    )
    assert "http_only" in read_json(report)["blockers"]
