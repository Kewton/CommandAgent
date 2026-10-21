from __future__ import annotations

import json
import os
import shutil
import sys
import time
from contextlib import contextmanager
from dataclasses import dataclass
from pathlib import Path

HEAVY_LOCK_DIR_ENV = "HEAVY_LOCK_DIR"
HEAVY_LOCK_HELD_ENV = "HEAVY_LOCK_HELD"
HEAVY_LOCK_POLL_SECONDS_ENV = "HEAVY_LOCK_POLL_SECONDS"
DEFAULT_HEAVY_LOCK_DIR = "/Volumes/SSD_NX/tmp/heavy-lock/slot.d"
DEFAULT_POLL_SECONDS = 2.0
WAIT_RECORD_FILE = "heavy-lock-waits.jsonl"
WAIT_RECORD_EVENT = "heavy_lock"

OWNER_MISSING = "missing"
OWNER_UNKNOWN = "unknown"
OWNER_DEAD = "dead"
OWNER_ALIVE = "alive"


class HeavyLockTimeout(RuntimeError):
    """The machine-wide heavy slot was not acquired within the wait budget."""


@dataclass(frozen=True)
class HeavyLockRecord:
    invocation: str
    lock_dir: str
    held: bool
    nested: bool
    wait_sec: float
    pid: int

    def as_event(self) -> dict[str, object]:
        return {
            "event": WAIT_RECORD_EVENT,
            "invocation": self.invocation,
            "lock_dir": self.lock_dir,
            "held": self.held,
            "nested": self.nested,
            "wait_sec": self.wait_sec,
            "pid": self.pid,
        }


def resolve_lock_dir(
    env: dict[str, str] | None = None,
    override: str | os.PathLike[str] | None = None,
) -> Path:
    if override is not None:
        return Path(override)
    env = os.environ if env is None else env
    value = env.get(HEAVY_LOCK_DIR_ENV)
    return Path(value) if value else Path(DEFAULT_HEAVY_LOCK_DIR)


def resolve_poll_seconds(env: dict[str, str] | None = None) -> float:
    env = os.environ if env is None else env
    raw = env.get(HEAVY_LOCK_POLL_SECONDS_ENV)
    if not raw:
        return DEFAULT_POLL_SECONDS
    try:
        value = float(raw)
    except ValueError:
        return DEFAULT_POLL_SECONDS
    return value if value > 0 else DEFAULT_POLL_SECONDS


def wait_record_path(run_dir: str | os.PathLike[str]) -> Path:
    return Path(run_dir) / WAIT_RECORD_FILE


def owner_state(lock_dir: Path) -> tuple[str, int | None]:
    """Classify the slot owner without guessing about a missing or broken pid.

    ``OWNER_DEAD`` is the only state that may be reclaimed: the pid file must
    hold a positive pid whose process no longer exists. A pid file that has not
    been written yet, holds a non-numeric value, or cannot be read is reported
    as ``OWNER_MISSING`` or ``OWNER_UNKNOWN`` and left in place.
    """
    try:
        raw = (lock_dir / "pid").read_text(encoding="utf-8").strip()
    except FileNotFoundError:
        return OWNER_MISSING, None
    except OSError:
        return OWNER_UNKNOWN, None
    if not raw:
        return OWNER_MISSING, None
    try:
        pid = int(raw)
    except ValueError:
        return OWNER_UNKNOWN, None
    if pid <= 0:
        return OWNER_UNKNOWN, None
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return OWNER_DEAD, pid
    except PermissionError:
        return OWNER_ALIVE, pid
    except OSError:
        return OWNER_UNKNOWN, None
    return OWNER_ALIVE, pid


def _write_owner(lock_dir: Path, invocation: str) -> None:
    (lock_dir / "pid").write_text(f"{os.getpid()}\n", encoding="utf-8")
    label = invocation or Path(sys.argv[0]).name
    (lock_dir / "cmd").write_text(f"{Path.cwd()} :: {label}\n", encoding="utf-8")


def _reclaim_path(lock_dir: Path) -> Path:
    return lock_dir.with_name(f"{lock_dir.name}.reclaim")


def _clear_dead_reclaimer(path: Path) -> None:
    try:
        pid = int(path.read_text(encoding="utf-8").strip())
    except (OSError, ValueError):
        return
    if pid <= 0:
        return
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        try:
            path.unlink()
        except OSError:
            pass
    except OSError:
        pass


def _enter_reclaim(lock_dir: Path) -> bool:
    path = _reclaim_path(lock_dir)
    try:
        fd = os.open(path, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o644)
    except FileExistsError:
        _clear_dead_reclaimer(path)
        return False
    try:
        os.write(fd, f"{os.getpid()}\n".encode("utf-8"))
    finally:
        os.close(fd)
    return True


def _leave_reclaim(lock_dir: Path) -> None:
    try:
        _reclaim_path(lock_dir).unlink()
    except OSError:
        pass


def _reclaim_stale(lock_dir: Path, dead_pid: int) -> bool:
    """Remove a slot whose recorded owner is gone, one reclaimer at a time.

    The re-check and the removal happen while holding an atomic sibling mutex,
    so two waiters cannot both delete a freshly re-acquired slot.
    """
    if not _enter_reclaim(lock_dir):
        return False
    try:
        state, pid = owner_state(lock_dir)
        if state != OWNER_DEAD or pid != dead_pid:
            return False
        shutil.rmtree(lock_dir, ignore_errors=True)
        return True
    finally:
        _leave_reclaim(lock_dir)


def _acquire(
    lock_dir: Path,
    *,
    poll_seconds: float,
    timeout_seconds: float | None,
    invocation: str,
    monotonic=time.monotonic,
    sleep=time.sleep,
) -> float:
    lock_dir.parent.mkdir(parents=True, exist_ok=True)
    start = monotonic()
    while True:
        try:
            lock_dir.mkdir()
        except FileExistsError:
            pass
        else:
            _write_owner(lock_dir, invocation)
            return max(0.0, monotonic() - start)
        state, pid = owner_state(lock_dir)
        if state == OWNER_DEAD and pid is not None and _reclaim_stale(lock_dir, pid):
            continue
        elapsed = monotonic() - start
        if timeout_seconds is not None and elapsed >= timeout_seconds:
            raise HeavyLockTimeout(
                f"heavy lock at {lock_dir} still held by a {state} owner after {elapsed:.3f}s"
            )
        sleep(poll_seconds)


class HeavyLock:
    """Context manager for the machine-wide heavy slot.

    The slot is the same ``mkdir`` directory the ``with-heavy-lock`` wrapper
    uses, so Python live evals exclude against cargo tests and any other holder.
    Nested callers (``HEAVY_LOCK_HELD`` already set) neither acquire nor release.
    """

    def __init__(
        self,
        *,
        run_dir: str | os.PathLike[str] | None = None,
        invocation: str = "",
        lock_dir: str | os.PathLike[str] | None = None,
        poll_seconds: float | None = None,
        timeout_seconds: float | None = None,
        env: dict[str, str] | None = None,
    ) -> None:
        self._env = os.environ if env is None else env
        self.run_dir = Path(run_dir) if run_dir is not None else None
        self.invocation = invocation or "unknown"
        self.lock_dir = resolve_lock_dir(self._env, lock_dir)
        self.poll_seconds = (
            poll_seconds if poll_seconds is not None else resolve_poll_seconds(self._env)
        )
        self.timeout_seconds = timeout_seconds
        self.record: HeavyLockRecord | None = None
        self._acquired = False
        self._had_previous_held = False
        self._previous_held: str | None = None

    @property
    def held(self) -> bool:
        return self._acquired

    def __enter__(self) -> HeavyLock:
        if self._env.get(HEAVY_LOCK_HELD_ENV):
            self.record = HeavyLockRecord(
                self.invocation, str(self.lock_dir), False, True, 0.0, os.getpid()
            )
            self._write_record()
            return self
        wait_sec = _acquire(
            self.lock_dir,
            poll_seconds=self.poll_seconds,
            timeout_seconds=self.timeout_seconds,
            invocation=self.invocation,
        )
        self._had_previous_held = HEAVY_LOCK_HELD_ENV in self._env
        self._previous_held = self._env.get(HEAVY_LOCK_HELD_ENV)
        self._env[HEAVY_LOCK_HELD_ENV] = str(os.getpid())
        self._acquired = True
        self.record = HeavyLockRecord(
            self.invocation, str(self.lock_dir), True, False, round(wait_sec, 6), os.getpid()
        )
        self._write_record()
        return self

    def __exit__(self, exc_type, exc, tb) -> bool:
        self.release()
        return False

    def release(self) -> None:
        if not self._acquired:
            return
        self._release_own()
        self._restore_env()
        self._acquired = False

    def _release_own(self) -> None:
        state, pid = owner_state(self.lock_dir)
        if state == OWNER_ALIVE and pid == os.getpid():
            shutil.rmtree(self.lock_dir, ignore_errors=True)

    def _restore_env(self) -> None:
        if self._had_previous_held:
            self._env[HEAVY_LOCK_HELD_ENV] = self._previous_held or ""
        else:
            self._env.pop(HEAVY_LOCK_HELD_ENV, None)

    def _write_record(self) -> None:
        if self.record is None or self.run_dir is None:
            return
        path = wait_record_path(self.run_dir)
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open("a", encoding="utf-8") as handle:
            handle.write(
                json.dumps(self.record.as_event(), ensure_ascii=False, sort_keys=True) + "\n"
            )


@contextmanager
def heavy_lock(
    run_dir: str | os.PathLike[str] | None = None,
    invocation: str = "",
    *,
    lock_dir: str | os.PathLike[str] | None = None,
    poll_seconds: float | None = None,
    timeout_seconds: float | None = None,
    env: dict[str, str] | None = None,
):
    lock = HeavyLock(
        run_dir=run_dir,
        invocation=invocation,
        lock_dir=lock_dir,
        poll_seconds=poll_seconds,
        timeout_seconds=timeout_seconds,
        env=env,
    )
    with lock as entered:
        yield entered
