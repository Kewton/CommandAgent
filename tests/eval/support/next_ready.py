"""Start ``npm start`` on an OS-assigned port and read the real port from Next.js.

The server is launched with ``-p 0``; Next.js binds the listener itself and
announces the actual port on stdout (``Local: http://localhost:<port>``). The
port is never guessed, probed or reserved by the test.
"""

import os
import queue
import re
import signal
import subprocess
import threading
import time
from contextlib import contextmanager
from pathlib import Path

_LOCAL_URL = re.compile(r"Local:\s+http://[^\s:/]+:(\d+)")
_STOP_GRACE_SECONDS = 10.0


class NextStartError(AssertionError):
    pass


class NextServer:
    def __init__(
        self, process: subprocess.Popen, lines: "queue.Queue[str | None]"
    ) -> None:
        self.process = process
        self.port = 0
        self.output: list[str] = []
        self._lines = lines

    def wait_for_port(self, timeout: float) -> int:
        deadline = time.monotonic() + timeout
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise NextStartError(
                    "next start did not announce a port\n" + self.tail()
                )
            try:
                line = self._lines.get(timeout=min(remaining, 0.5))
            except queue.Empty:
                continue
            if line is None:
                # EOF: every holder of the pipe is gone. The leader stays
                # unreaped here so the group can still be signalled safely.
                raise NextStartError("next start exited\n" + self.tail())
            self.output.append(line)
            match = _LOCAL_URL.search(line)
            if match and int(match.group(1)) > 0:
                self.port = int(match.group(1))
                return self.port

    def tail(self) -> str:
        return "".join(self.output)[-2000:]


def _pump(stream, lines: "queue.Queue[str | None]") -> None:
    try:
        for line in stream:
            lines.put(line)
    finally:
        lines.put(None)


def _signal_group(process: subprocess.Popen, sig: int) -> None:
    """Signal the group only while its leader is unreaped.

    A reaped leader frees its pid for reuse, so the group id could then name an
    unrelated process group.
    """
    if process.returncode is not None:
        return
    pgid = process.pid
    assert pgid > 1
    try:
        os.killpg(pgid, sig)
    except (ProcessLookupError, PermissionError):
        # macOS reports EPERM for a group whose only member is a zombie leader.
        pass


def _stop(process: subprocess.Popen, reader: threading.Thread) -> None:
    _signal_group(process, signal.SIGTERM)
    # EOF on the pipe means every group member that held it has exited.
    reader.join(timeout=_STOP_GRACE_SECONDS)
    # Still before the final wait(): reap the rest of the group, then the leader.
    _signal_group(process, signal.SIGKILL)
    process.wait()
    reader.join(timeout=5)


@contextmanager
def next_server(cwd: Path, *, timeout: float = 60.0, argv: list[str] | None = None):
    env = dict(os.environ, NEXT_TELEMETRY_DISABLED="1")
    process = subprocess.Popen(
        argv or ["npm", "run", "start", "--", "-p", "0"],
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        env=env,
        start_new_session=True,
    )
    lines: queue.Queue[str | None] = queue.Queue()
    reader = threading.Thread(target=_pump, args=(process.stdout, lines), daemon=True)
    reader.start()
    server = NextServer(process, lines)
    try:
        server.wait_for_port(timeout)
        yield server
    finally:
        _stop(process, reader)
        process.stdout.close()
