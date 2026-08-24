#!/usr/bin/env python3
"""Record a real `commandagent` REPL session from a PTY into a timestamped cast.

The recording is a JSON document: {"cols", "rows", "command", "events": [[t, base64]]}
where `t` is seconds since start and the payload is the raw bytes the program
wrote to the terminal. Nothing is scripted on the agent side: the model,
provider, and workspace are real, and every byte comes from the binary.
"""

from __future__ import annotations

import argparse
import base64
import fcntl
import json
import os
import pty
import re
import select
import struct
import sys
import termios
import time

ANSI = re.compile(rb"\x1b\[[0-9;?]*[A-Za-z]|\x1b\][^\x07]*\x07|\x1b[()][A-Z0-9]")
CONFIRM = re.compile(rb"/confirm(sha256:[0-9a-f]{64})")
WHITESPACE = re.compile(rb"\s+")


def debug_dump(clean: bytearray, out: str) -> None:
    path = f"{out}.debug.txt"
    with open(path, "wb") as handle:
        handle.write(bytes(clean[-20000:]))
    print(f"debug text written to {path}", file=sys.stderr)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin", required=True, help="path to the commandagent binary")
    parser.add_argument("--workdir", required=True, help="trusted workspace to run in")
    parser.add_argument("--provider", default="ollama")
    parser.add_argument("--model", required=True)
    parser.add_argument("--cols", type=int, default=100)
    parser.add_argument("--rows", type=int, default=28)
    parser.add_argument("--out", required=True, help="cast JSON output path")
    parser.add_argument(
        "--goal",
        default="Create a CLI --pattern filter command",
        help="goal typed into the REPL; the boundary shell renders Gate 1 for it",
    )
    parser.add_argument("--profile", default="python-cli")
    parser.add_argument(
        "--state-dir",
        required=True,
        help="isolated --state-dir so no personal REPL history leaks into the capture",
    )
    parser.add_argument("--idle-seconds", type=float, default=20.0)
    parser.add_argument(
        "--read-seconds", type=float, default=6.0, help="pause on the Gate 1 card"
    )
    parser.add_argument("--max-seconds", type=float, default=1500.0)
    parser.add_argument(
        "--yes", action="store_true", help="pass --yes (trusted workspace only)"
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    command = [
        args.bin,
        "--provider",
        args.provider,
        "--model",
        args.model,
        "--profile",
        args.profile,
        "--state-dir",
        args.state_dir,
    ]
    if args.yes:
        command.append("--yes")

    env = {
        "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
        "HOME": os.environ.get("HOME", "/"),
        "TERM": "xterm-256color",
        "LANG": "en_US.UTF-8",
        "LC_ALL": "en_US.UTF-8",
        "COLUMNS": str(args.cols),
        "LINES": str(args.rows),
    }
    for key in ("OLLAMA_HOST", "LM_STUDIO_API_TOKEN"):
        if key in os.environ:
            env[key] = os.environ[key]

    pid, fd = pty.fork()
    if pid == 0:
        os.chdir(args.workdir)
        fcntl.ioctl(
            sys.stdout.fileno(),
            termios.TIOCSWINSZ,
            struct.pack("HHHH", args.rows, args.cols, 0, 0),
        )
        os.execvpe(command[0], command, env)

    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", args.rows, args.cols, 0, 0))
    start = time.monotonic()
    events: list[tuple[float, bytes]] = []
    clean = bytearray()
    last_output = start

    def pump(timeout: float) -> bool:
        nonlocal last_output
        ready, _, _ = select.select([fd], [], [], timeout)
        if not ready:
            return False
        try:
            data = os.read(fd, 65536)
        except OSError:
            return False
        if not data:
            return False
        now = time.monotonic()
        events.append((now - start, data))
        clean.extend(ANSI.sub(b"", data))
        last_output = now
        return True

    def wait_for(marker: bytes, limit: float) -> bool:
        deadline = time.monotonic() + limit
        while time.monotonic() < deadline:
            pump(0.2)
            if marker in clean:
                return True
        return False

    def pause(seconds: float) -> None:
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            pump(0.1)

    def type_line(text: str, delay: float = 0.045) -> None:
        for char in text:
            os.write(fd, char.encode("utf-8"))
            pause(delay)
        pause(0.4)
        os.write(fd, b"\r")

    try:
        if not wait_for(b"commandagent> ", 60):
            print("prompt never appeared", file=sys.stderr)
        pause(2.0)
        type_line("/status")
        pause(5.0)
        marker_offset = len(clean)
        # A plain request (not a slash command) makes the boundary shell render
        # the Gate 1 card; execution needs the exact /confirm <hash> afterwards.
        type_line(args.goal)
        # Read the hash from the screen like a person would. The card may wrap
        # the hash across lines, so match on squashed text.
        confirm_hash: bytes | None = None
        confirm_deadline = time.monotonic() + 180
        while time.monotonic() < confirm_deadline and confirm_hash is None:
            pump(0.2)
            squashed = WHITESPACE.sub(b"", bytes(clean[marker_offset:]))
            match = CONFIRM.search(squashed)
            if match is not None:
                confirm_hash = match.group(1)
        if confirm_hash is None:
            print("Gate 1 card never appeared", file=sys.stderr)
            debug_dump(clean, args.out)
        else:
            pause(args.read_seconds)
            type_line(f"/confirm {confirm_hash.decode('ascii')}")
        deadline = start + args.max_seconds
        while time.monotonic() < deadline:
            pump(0.5)
            since_output = time.monotonic() - last_output
            tail = clean[marker_offset:]
            ran = b"Dispatching" in tail or b"Run ID" in tail
            if ran and since_output >= args.idle_seconds:
                break
        pause(3.0)
        type_line("/exit")
        pause(2.0)
    finally:
        try:
            os.kill(pid, 15)
        except ProcessLookupError:
            pass
        try:
            os.waitpid(pid, 0)
        except ChildProcessError:
            pass
        os.close(fd)

    document = {
        "cols": args.cols,
        "rows": args.rows,
        "command": command,
        "workdir": args.workdir,
        "recorded_at": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
        "events": [
            [round(t, 4), base64.b64encode(data).decode("ascii")] for t, data in events
        ],
    }
    with open(args.out, "w", encoding="utf-8") as handle:
        json.dump(document, handle)
    print(
        f"wrote {args.out}: {len(events)} chunks, {events[-1][0] if events else 0:.1f}s"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
