#!/usr/bin/env python3
"""Owned loopback GUI smoke fixture; never invokes model generation."""

from __future__ import annotations

import argparse
import functools
import json
import sys
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from browser_preflight.campaign import freeze, launch
from browser_preflight.evidence import PreflightError, file_hash, read_json, write_new
from browser_preflight.playwright_probe import probe
from browser_preflight.session import finish, prepare

FIXTURE = Path(__file__).parent / "tests/fixtures/browser-preflight/smoke.html"


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path != "/":
            self.send_error(404)
            return
        data = FIXTURE.read_bytes()
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *_):
        pass


def base_request(url):
    return {
        "session_id": "issue-450-local-smoke",
        "method": "isolated-playwright",
        "target_url": url,
        "authorization": "Issue #450 approved local GUI smoke: owned fixture, harmless button, screenshot",
        "isolated_fallback_authorized": True,
        "action": {
            "role": "button",
            "name": "Check browser",
            "before": "Ready for browser check",
            "after": "Browser action confirmed",
        },
        "conditions": {
            "headless": False,
            "viewport": {"width": 1000, "height": 800},
            "locale": "en-US",
            "timezone": "Asia/Tokyo",
        },
    }


def run(output, url, executable, headless):
    request = base_request(url)
    request["executable"] = executable
    request["conditions"]["headless"] = headless
    write_new(output / "request.json", request)
    prepared = prepare(output / "ledger", request)
    report_path = finish(prepared["ticket"], probe(prepared["ticket"]))
    report = read_json(report_path)
    if report["status"] != "passed":
        raise PreflightError(
            f"GUI preflight blocked: {report['blockers']}; see {report_path}"
        )
    assert prepare(output / "ledger", request)["decision"] == "reuse"
    manifest = freeze(output / "campaign", output.name, report_path, request)
    bad = {**request, "target_url": url + "different"}
    marker = output / "should-not-launch"
    command = [
        sys.executable,
        "-c",
        "from pathlib import Path; import sys; Path(sys.argv[1]).touch()",
        str(marker),
    ]
    try:
        launch(manifest, bad, command)
    except PreflightError:
        pass
    else:
        raise AssertionError("Changed target must not launch")
    assert not marker.exists()
    assert (
        launch(
            manifest,
            request,
            [
                sys.executable,
                "-c",
                "print('Browser-gated smoke launch; no generation')",
            ],
        )
        == 0
    )
    return {
        "status": "passed",
        "report": str(report_path),
        "manifest": str(manifest),
        "healthy_reused": True,
        "changed_target_launch_refused": True,
        "owned_browser_closed": report["observation"]["owned_resources_closed"],
        "model_generation_started": False,
        "automatic_diagnosis": "implemented",
        "plugin_permanent_repair": "not_established",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["serve", "run"])
    parser.add_argument(
        "--output", type=Path, required=True, help="New task-owned output directory"
    )
    parser.add_argument(
        "--executable", help="Explicit installed Chrome/Chromium executable for run"
    )
    parser.add_argument("--headless", action="store_true")
    args = parser.parse_args()
    if args.mode == "run" and not args.executable:
        parser.error("run requires --executable")
    args.output.mkdir(parents=True, exist_ok=False)
    server = ThreadingHTTPServer(("127.0.0.1", 0), functools.partial(Handler))
    url = f"http://127.0.0.1:{server.server_port}/"
    write_new(
        args.output / "server.json",
        {"target_url": url, "fixture_sha256": file_hash(FIXTURE)},
    )
    worker = threading.Thread(target=server.serve_forever, daemon=True)
    worker.start()
    result = {"status": "blocked"}
    print(json.dumps({"target_url": url, "output": str(args.output)}), flush=True)
    try:
        if args.mode == "serve":
            worker.join()
            result = {"status": "served"}
        else:
            result = run(args.output, url, args.executable, args.headless)
    except KeyboardInterrupt:
        result = {"status": "served"}
    except Exception as error:
        result = {"status": "blocked", "error": f"{type(error).__name__}: {error}"}
    finally:
        server.shutdown()
        server.server_close()
        worker.join(timeout=5)
        result["owned_server_closed"] = not worker.is_alive()
        write_new(args.output / "smoke-result.json", result)
    print(json.dumps(result, indent=2))
    return 0 if result["status"] in ("passed", "served") else 2


if __name__ == "__main__":
    raise SystemExit(main())
