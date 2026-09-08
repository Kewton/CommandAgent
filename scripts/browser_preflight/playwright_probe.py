"""Authorized local fallback; owns only its temporary profile and browser."""

from __future__ import annotations

import tempfile
from importlib.metadata import version
from pathlib import Path
from urllib.request import urlopen

from .evidence import PreflightError, read_json, write_new
from .session import claim_probe, validate_request


def probe(ticket_path):
    ticket_path = Path(ticket_path)
    ticket = read_json(ticket_path)
    request = ticket["request"]
    validate_request(request)
    if request["method"] != "isolated-playwright":
        raise PreflightError("Standalone adapter requires isolated-playwright method")
    directory = ticket_path.parent
    claim_probe(ticket_path)
    result = {"ticket_id": ticket["id"], "stage": "dependencies"}
    try:
        from playwright.sync_api import sync_playwright

        automation_version = version("playwright")
        executable = Path(request["executable"])
        if not executable.is_absolute() or not executable.is_file():
            raise PreflightError("An explicit installed browser executable is required")
        try:
            with urlopen(request["target_url"], timeout=5) as response:
                result["http"] = {
                    "ok": response.status == 200,
                    "status": response.status,
                }
        except OSError as error:
            result["http"] = {"ok": False, "error": str(error)}
        settings = request["conditions"]
        with tempfile.TemporaryDirectory(prefix="issue-browser-preflight-") as profile:
            with sync_playwright() as playwright:
                result["stage"] = "connect"
                context = playwright.chromium.launch_persistent_context(
                    profile,
                    executable_path=str(executable),
                    headless=settings["headless"],
                    viewport=settings["viewport"],
                    locale=settings["locale"],
                    timezone_id=settings["timezone"],
                    timeout=15000,
                )
                result["isolated_profile"] = True
                try:
                    context.set_default_timeout(10000)
                    result["tabs_count"] = len(context.pages)
                    page = context.new_page()
                    result["stage"] = "navigate"
                    page.goto(request["target_url"], wait_until="domcontentloaded")
                    result["url"] = page.url
                    result["conditions"] = {
                        "browser_version": context.browser.version,
                        "automation_version": automation_version,
                        "headless": settings["headless"],
                        "viewport": page.viewport_size,
                        "locale": page.evaluate("navigator.language"),
                        "timezone": page.evaluate(
                            "Intl.DateTimeFormat().resolvedOptions().timeZone"
                        ),
                    }
                    action = request["action"]
                    result["stage"] = "read"
                    page.get_by_text(action["before"], exact=True).wait_for(
                        state="visible"
                    )
                    (directory / "before.txt").write_text(
                        page.locator("body").inner_text()
                    )
                    result["before"] = "before.txt"
                    result["stage"] = "interaction"
                    target = page.get_by_role(
                        action["role"], name=action["name"], exact=True
                    )
                    if not target.is_visible():
                        raise PreflightError("Declared harmless action is not visible")
                    target.click()
                    page.get_by_text(action["after"], exact=True).wait_for(
                        state="visible"
                    )
                    (directory / "after.txt").write_text(
                        page.locator("body").inner_text()
                    )
                    result.update(action=action, after="after.txt", url=page.url)
                    result["stage"] = "screenshot"
                    page.screenshot(path=str(directory / "screenshot.png"))
                    result.update(screenshot="screenshot.png", stage="complete")
                finally:
                    context.close()
                    result["owned_resources_closed"] = True
    except Exception as error:
        result["error"] = f"{type(error).__name__}: {error}"
    write_new(directory / "observation.json", result)
    return result
