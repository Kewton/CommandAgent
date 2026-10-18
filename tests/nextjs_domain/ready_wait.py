"""Bounded wait for a child's one-line ready JSON, with staged cleanup."""

import contextlib
import json
import os
import selectors
import subprocess
import time

STEP_SEC = 0.1
DIAGNOSTIC_LIMIT = 4000


class ReadyError(AssertionError):
    """Startup failed; `kind` says how and `process` is the (reaped) child."""

    def __init__(self, kind, detail, process=None):
        super().__init__(f"{kind}: {detail}")
        self.kind = kind
        self.process = process


def parse_ready_line(line):
    try:
        port = json.loads(line)["port"]
    except (ValueError, KeyError, TypeError) as err:
        raise ReadyError("invalid-json", f"{line!r} ({err})") from err
    if not isinstance(port, int) or isinstance(port, bool) or not 0 < port < 65536:
        raise ReadyError("invalid-port", repr(port))
    return port


def wait_ready_port(read, exited, clock, timeout, step=STEP_SEC):
    """Return the port from the first complete line read before the deadline.

    `read(seconds)` returns bytes, None when nothing arrived, or b"" at EOF.
    `exited()` returns the child's exit code or None. `clock` is monotonic.
    """
    deadline = clock() + timeout
    buffer = b""
    while True:
        remaining = deadline - clock()
        if remaining <= 0:
            kind = "partial-line" if buffer else "timeout"
            raise ReadyError(
                kind, f"no complete ready line in {timeout}s; got {buffer!r}"
            )
        chunk = read(min(step, remaining))
        if chunk == b"":
            kind = "partial-eof" if buffer else "eof"
            raise ReadyError(kind, f"stdout closed before a ready line; got {buffer!r}")
        if chunk:
            buffer += chunk
            if b"\n" in buffer:
                return parse_ready_line(
                    buffer.split(b"\n", 1)[0].decode("utf-8", "replace")
                )
        elif (code := exited()) is not None:
            raise ReadyError("early-exit", f"child exited with {code} before ready")


def stop_child(process, terminate_sec=10, kill_sec=5):
    """terminate, then kill, then reap; always close the stdout pipe."""
    try:
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=terminate_sec)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=kill_sec)
    finally:
        if process.stdout:
            process.stdout.close()


def _tail(errors):
    errors.flush()
    errors.seek(0)
    return errors.read()[-DIAGNOSTIC_LIMIT:]


@contextlib.contextmanager
def ready_child(argv, cwd, errors, startup_sec=15, clock=time.monotonic, **popen):
    """Start `argv` and yield (process, port) once it announces its port."""
    process = subprocess.Popen(
        argv, cwd=cwd, stdout=subprocess.PIPE, stderr=errors, **popen
    )
    try:
        fd = process.stdout.fileno()
        with selectors.DefaultSelector() as selector:
            selector.register(fd, selectors.EVENT_READ)

            def read(seconds):
                return os.read(fd, 4096) if selector.select(seconds) else None

            try:
                port = wait_ready_port(read, process.poll, clock, startup_sec)
            except ReadyError as err:
                err.process = process
                err.args = (f"{err.args[0]}\nchild stderr: {_tail(errors)}",)
                raise
        yield process, port
    finally:
        stop_child(process)
