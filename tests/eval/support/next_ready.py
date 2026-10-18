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
                if self.process.poll() is not None:
                    raise NextStartError(
                        f"next start exited {self.process.returncode}\n" + self.tail()
                    )
                continue
            if line is None:
                self.process.wait()
                raise NextStartError(
                    f"next start exited {self.process.returncode}\n" + self.tail()
                )
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


def _stop(process: subprocess.Popen) -> None:
    if process.poll() is None:
        try:
            os.killpg(process.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            process.wait(timeout=_STOP_GRACE_SECONDS)
        except subprocess.TimeoutExpired:
            pass
    # Reap the whole group even when npm already exited and left its child behind.
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    process.wait()


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
        _stop(process)
        reader.join(timeout=5)
        process.stdout.close()
