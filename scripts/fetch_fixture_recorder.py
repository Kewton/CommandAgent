#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import http.client
import json
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any

EVENT_BODY = b"<html><body><article>fixture event</article></body></html>\n"
CONTRACT_ROBOTS_URL = "https://events.example.test/robots.txt"
CONTRACT_CONTENT_URL = "https://events.example.test/events.html"


class FixtureHandler(BaseHTTPRequestHandler):
    def do_GET(self) -> None:
        if self.path == "/robots.txt":
            self.send_response(404)
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        if self.path == "/events.html":
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.send_header("Content-Length", str(len(EVENT_BODY)))
            self.end_headers()
            self.wfile.write(EVENT_BODY)
            return
        self.send_response(404)
        self.send_header("Content-Length", "0")
        self.end_headers()

    def log_message(self, _format: str, *args: object) -> None:
        del args


def sha256(body: bytes) -> str:
    return hashlib.sha256(body).hexdigest()


def fetch_localhost(port: int, path: str) -> tuple[int, bytes]:
    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=3)
    try:
        connection.request("GET", path, headers={"User-Agent": "fixture-recorder/1"})
        response = connection.getresponse()
        return response.status, response.read()
    finally:
        connection.close()


def record() -> dict[str, Any]:
    server = ThreadingHTTPServer(("127.0.0.1", 0), FixtureHandler)
    port = server.server_address[1]
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        robots_status, robots_body = fetch_localhost(port, "/robots.txt")
        content_status, content_body = fetch_localhost(port, "/events.html")
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=3)
    return {
        "schema_version": "commandagent.fetch-recording/v0",
        "provenance": {
            "kind": "localhost_fixture",
            "recorder": "scripts/fetch_fixture_recorder.py",
            "source_origin": f"http://127.0.0.1:{port}",
        },
        "exchanges": [
            {
                "url": CONTRACT_ROBOTS_URL,
                "http_status": robots_status,
                "body": robots_body.decode("utf-8"),
                "body_sha256": sha256(robots_body),
                "elapsed_ms": 0,
                "remote_ip": "8.8.8.8",
                "redirect_location": None,
            },
            {
                "url": CONTRACT_CONTENT_URL,
                "http_status": content_status,
                "body": content_body.decode("utf-8"),
                "body_sha256": sha256(content_body),
                "elapsed_ms": 0,
                "remote_ip": "8.8.8.8",
                "redirect_location": None,
            },
        ],
    }


def comparable(recording: dict[str, Any]) -> list[dict[str, Any]]:
    return [
        {
            key: exchange[key]
            for key in ("url", "http_status", "body", "body_sha256", "redirect_location")
        }
        for exchange in recording["exchanges"]
    ]


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Record or verify the self-hosted fetch fixture; never contacts the internet."
    )
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--verify", type=Path)
    mode.add_argument("--output", type=Path)
    args = parser.parse_args()
    observed = record()
    if args.verify is not None:
        expected = json.loads(args.verify.read_text(encoding="utf-8"))
        if comparable(observed) != comparable(expected):
            raise SystemExit("localhost recording differs from the committed fixture")
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(
            json.dumps(observed, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )
    print(
        json.dumps(
            {
                "status": "pass",
                "source_origin": observed["provenance"]["source_origin"],
                "exchanges": len(observed["exchanges"]),
                "content_sha256": observed["exchanges"][1]["body_sha256"],
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
