#!/usr/bin/env python3
"""Isolated runner for the providerless CLI/config black-box cases.

Each case runs in its own temporary HOME and workspace with an allowlisted
environment, inside an OS-level network guard that is verified at run time,
and every process it starts is collected afterwards. A single failing,
timed-out or unrunnable case makes the runner exit non-zero.

Exit codes of ``main``: 0 every case passed, 1 a case failed or timed out,
2 the case file or arguments are invalid, 3 the environment cannot run the
cases honestly (missing binary, missing tool, no working network guard).

Run directly::

    python3 tests/cli_cases/harness.py [--cases FILE] [--binary PATH] [--id ID ...]
"""

from __future__ import annotations

import argparse
import dataclasses
import errno
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
CORPUS_DIR = REPO_ROOT / "tests" / "corpus" / "apps" / "quality-providerless-core"
CASES_PATH = CORPUS_DIR / "fixtures" / "cases.json"
EXPECTATIONS_PATH = CORPUS_DIR / "expectations.toml"
CASES_SELECTOR = "fixtures/cases.json"

SCHEMA = "commandagent.providerless-cli-cases/v1"
ID_PATTERN = re.compile(r"^BB-[A-Z]+-\d{3}$")
ISSUE_PATTERN = re.compile(r"^#\d+$")
MAX_TIMEOUT_SEC = 600
OUTPUT_LIMIT = 64 * 1024

# Nothing is inherited from the invoking environment: the case environment is
# built from scratch and holds exactly these names.
BASE_PATH = ("/usr/bin", "/bin", "/usr/sbin", "/sbin")
ENV_NAMES = (
    "PATH",
    "HOME",
    "XDG_STATE_HOME",
    "TMPDIR",
    "LC_ALL",
    "NO_COLOR",
    "OLLAMA_UNREACHABLE",
    "BIN",
    "T",
)
REQUIRED_TOOLS = ("bash", "jq")
# Closed loopback port: a provider probe against it is refused locally.
OLLAMA_UNREACHABLE = "http://127.0.0.1:9"

# RFC 5737 TEST-NET-1 is never routed, so even a failed guard cannot reach a host.
PROBE_HOST = "192.0.2.1"
PROBE_PORT = 9
GUARD_DENIED_ERRNOS = frozenset(
    {errno.EPERM, errno.EACCES, errno.ENETUNREACH, errno.ENETDOWN}
)
PROBE_SOURCE = """
import errno, json, socket, sys
host, port = sys.argv[1], int(sys.argv[2])

def attempt(kind):
    sock = socket.socket(
        socket.AF_INET, socket.SOCK_STREAM if kind == "tcp" else socket.SOCK_DGRAM
    )
    sock.settimeout(3)
    try:
        sock.connect((host, port))
        if kind == "udp":
            sock.send(b"x")
        return "connected"
    except socket.timeout:
        return "timeout"
    except OSError as exc:
        return errno.errorcode.get(exc.errno, "errno-%s" % exc.errno)
    finally:
        sock.close()

print(json.dumps({"tcp": attempt("tcp"), "udp": attempt("udp")}))
"""
# A fresh Linux network namespace holds a loopback interface that is down, which
# would refuse the connections a case makes to its own loopback mock. This helper
# runs inside the new namespace, raises ``lo`` and then execs the real command, so
# only loopback is added; every external destination stays unreachable.
LOOPBACK_UP_SOURCE = """
import fcntl, os, socket, struct, sys
IFF_UP, SIOCGIFFLAGS, SIOCSIFFLAGS = 0x1, 0x8913, 0x8914
sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
try:
    ifreq = struct.pack("16sH22x", b"lo", 0)
    flags = struct.unpack("16sH22x", fcntl.ioctl(sock, SIOCGIFFLAGS, ifreq))[1]
    ifreq = struct.pack("16sH22x", b"lo", flags | IFF_UP)
    fcntl.ioctl(sock, SIOCSIFFLAGS, ifreq)
finally:
    sock.close()
os.execvp(sys.argv[1], sys.argv[1:])
"""
# Cases own loopback mocks, so a guard is only usable when a connection to a
# listener on 127.0.0.1 works inside it.
LOOPBACK_PROBE_SOURCE = """
import socket
server = socket.socket()
server.bind(("127.0.0.1", 0))
server.listen(1)
client = socket.create_connection(server.getsockname(), 3)
conn, _ = server.accept()
print("loopback-ok")
"""
DARWIN_SANDBOX_PROFILE = (
    "(version 1)(allow default)(deny network-outbound)"
    '(allow network-outbound (remote ip "localhost:*"))'
)


class HarnessError(RuntimeError):
    """The harness cannot run the cases honestly (never downgraded to a skip)."""


class CaseFileError(ValueError):
    """The case file does not match the schema."""


@dataclasses.dataclass(frozen=True)
class Expected:
    exit_code: int | str
    stdout_contains: tuple[str, ...]
    stderr_contains: tuple[str, ...]
    must_not_contain: tuple[str, ...]
    files_exist: tuple[str, ...]
    files_absent: tuple[str, ...]


@dataclasses.dataclass(frozen=True)
class Case:
    id: str
    area: str
    purpose: str
    source: str
    setup: tuple[str, ...]
    command: str
    timeout_sec: int
    expected: Expected
    note: str = ""
    xfail_issue: str = ""
    xfail_reason: str = ""

    @property
    def xfail(self) -> bool:
        return bool(self.xfail_issue)


@dataclasses.dataclass
class CaseResult:
    case_id: str
    failures: list[str]
    exit_code: int | None
    timed_out: bool
    stdout: str
    stderr: str
    duration_sec: float
    guard: str
    leaked_processes: bool

    @property
    def passed(self) -> bool:
        return not self.failures


@dataclasses.dataclass(frozen=True)
class NetworkGuard:
    name: str
    prefix: tuple[str, ...]


# ---------------------------------------------------------------- case file


def _require_keys(
    obj: object, required: set[str], optional: set[str], where: str
) -> dict:
    if not isinstance(obj, dict):
        raise CaseFileError(f"{where}: expected an object")
    missing = required - obj.keys()
    unknown = obj.keys() - required - optional
    if missing:
        raise CaseFileError(f"{where}: missing keys {sorted(missing)}")
    if unknown:
        raise CaseFileError(f"{where}: unknown keys {sorted(unknown)}")
    return obj


def _string(value: object, where: str, *, allow_empty: bool = False) -> str:
    if not isinstance(value, str) or (not value and not allow_empty):
        raise CaseFileError(f"{where}: expected a non-empty string")
    return value


def _strings(value: object, where: str) -> tuple[str, ...]:
    if not isinstance(value, list):
        raise CaseFileError(f"{where}: expected a list of strings")
    return tuple(_string(item, f"{where}[{index}]") for index, item in enumerate(value))


def parse_case(raw: object) -> Case:
    where = raw.get("id", "<case>") if isinstance(raw, dict) else "<case>"
    obj = _require_keys(
        raw,
        {
            "id",
            "area",
            "purpose",
            "source",
            "setup",
            "command",
            "timeout_sec",
            "expected",
        },
        {"note", "xfail"},
        str(where),
    )
    case_id = _string(obj["id"], "id")
    if not ID_PATTERN.match(case_id):
        raise CaseFileError(f"{case_id}: id must look like BB-<AREA>-<NNN>")
    timeout = obj["timeout_sec"]
    if not isinstance(timeout, int) or isinstance(timeout, bool):
        raise CaseFileError(f"{case_id}: timeout_sec must be an integer")
    if not 1 <= timeout <= MAX_TIMEOUT_SEC:
        raise CaseFileError(f"{case_id}: timeout_sec must be 1..{MAX_TIMEOUT_SEC}")
    exp = _require_keys(
        obj["expected"],
        {
            "exit_code",
            "stdout_contains",
            "stderr_contains",
            "must_not_contain",
            "files_exist",
            "files_absent",
        },
        set(),
        f"{case_id}.expected",
    )
    exit_code = exp["exit_code"]
    if isinstance(exit_code, bool) or not (
        exit_code == "nonzero" or (isinstance(exit_code, int) and 0 <= exit_code <= 255)
    ):
        raise CaseFileError(f'{case_id}: exit_code must be 0..255 or "nonzero"')
    files_exist = _strings(exp["files_exist"], f"{case_id}.files_exist")
    files_absent = _strings(exp["files_absent"], f"{case_id}.files_absent")
    for path in (*files_exist, *files_absent):
        if not path.startswith("$T/") or ".." in Path(path).parts:
            raise CaseFileError(
                f"{case_id}: file checks must stay under $T/, got {path!r}"
            )
    xfail_issue = xfail_reason = ""
    if "xfail" in obj:
        xfail = _require_keys(
            obj["xfail"], {"issue", "reason"}, set(), f"{case_id}.xfail"
        )
        xfail_issue = _string(xfail["issue"], f"{case_id}.xfail.issue")
        xfail_reason = _string(xfail["reason"], f"{case_id}.xfail.reason")
        if not ISSUE_PATTERN.match(xfail_issue):
            raise CaseFileError(f"{case_id}: xfail.issue must look like #123")
    note = obj.get("note", "")
    return Case(
        id=case_id,
        area=_string(obj["area"], f"{case_id}.area"),
        purpose=_string(obj["purpose"], f"{case_id}.purpose"),
        source=_string(obj["source"], f"{case_id}.source"),
        setup=_strings(obj["setup"], f"{case_id}.setup"),
        command=_string(obj["command"], f"{case_id}.command"),
        timeout_sec=timeout,
        expected=Expected(
            exit_code=exit_code,
            stdout_contains=_strings(
                exp["stdout_contains"], f"{case_id}.stdout_contains"
            ),
            stderr_contains=_strings(
                exp["stderr_contains"], f"{case_id}.stderr_contains"
            ),
            must_not_contain=_strings(
                exp["must_not_contain"], f"{case_id}.must_not_contain"
            ),
            files_exist=files_exist,
            files_absent=files_absent,
        ),
        note=_string(note, f"{case_id}.note", allow_empty=True),
        xfail_issue=xfail_issue,
        xfail_reason=xfail_reason,
    )


def parse_document(doc: object) -> tuple[dict, list[Case]]:
    """Validate a decoded case document; returns ``(manifest, cases)``."""
    top = _require_keys(doc, {"schema", "manifest", "cases"}, set(), "document")
    if top["schema"] != SCHEMA:
        raise CaseFileError(f"schema must be {SCHEMA!r}, got {top['schema']!r}")
    if not isinstance(top["cases"], list):
        raise CaseFileError("cases: expected a list")
    cases = [parse_case(raw) for raw in top["cases"]]
    seen: set[str] = set()
    for case in cases:
        if case.id in seen:
            raise CaseFileError(f"duplicate case id {case.id}")
        seen.add(case.id)
    manifest = _require_keys(
        top["manifest"],
        {"total", "counts", "ids", "pending_adjudication"},
        set(),
        "manifest",
    )
    return manifest, cases


def load_cases(path: Path = CASES_PATH) -> tuple[dict, list[Case]]:
    try:
        doc = json.loads(Path(path).read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise CaseFileError(f"cannot read {path}: {exc}") from exc
    return parse_document(doc)


def manifest_problems(
    manifest: dict, cases: list[Case], owned_domains: tuple[str, ...]
) -> list[str]:
    """Return every way the manifest and the registered cases disagree."""
    problems: list[str] = []
    ids = manifest["ids"]
    counts = manifest["counts"]
    if not isinstance(ids, dict) or not isinstance(counts, dict):
        return ["manifest.ids and manifest.counts must be objects"]
    flat = [case_id for domain_ids in ids.values() for case_id in domain_ids]
    if len(flat) != len(set(flat)):
        problems.append("manifest.ids contains duplicate ids")
    if set(ids) != set(counts):
        problems.append("manifest.ids and manifest.counts list different domains")
    for domain, domain_ids in ids.items():
        if counts.get(domain) != len(domain_ids):
            problems.append(
                f"manifest.counts.{domain}={counts.get(domain)} but {len(domain_ids)} ids listed"
            )
    if manifest["total"] != len(flat):
        problems.append(
            f"manifest.total={manifest['total']} but {len(flat)} ids listed"
        )
    pending = manifest["pending_adjudication"]
    if len(pending) != len(set(pending)):
        problems.append("pending_adjudication contains duplicates")
    registered = [case.id for case in cases]
    owned = [case_id for domain in owned_domains for case_id in ids.get(domain, [])]
    outside = sorted(set(registered) - set(owned))
    if outside:
        problems.append(f"registered cases outside the owned domains: {outside}")
    overlap = sorted(set(registered) & set(pending))
    if overlap:
        problems.append(f"cases both registered and pending adjudication: {overlap}")
    stray = sorted(set(pending) - set(owned))
    if stray:
        problems.append(f"pending ids outside the owned domains: {stray}")
    missing = sorted(set(owned) - set(registered) - set(pending))
    if missing:
        problems.append(f"manifest ids neither registered nor pending: {missing}")
    return problems


# ----------------------------------------------- expectations.toml (manifest)


def _strip_comment(line: str) -> str:
    in_string = escaped = False
    for index, ch in enumerate(line):
        if escaped:
            escaped = False
        elif ch == "\\" and in_string:
            escaped = True
        elif ch == '"':
            in_string = not in_string
        elif ch == "#" and not in_string:
            return line[:index]
    return line


def read_expectations(path: Path = EXPECTATIONS_PATH) -> dict[str, dict]:
    """Read the corpus walker's expectations.toml dialect.

    ``tests/corpus_regression.rs`` accepts bare ``fixture:selector`` keys that
    strict TOML rejects, so this mirrors its line-oriented reader for the
    sections this corpus uses: root keys, ``[json_fields]`` and
    ``[fixture_contains]``. Anything else is an error, as it is in the walker.
    """
    sections: dict[str, dict] = {"": {}}
    section = ""
    pending = ""
    for raw in Path(path).read_text(encoding="utf-8").splitlines():
        line = _strip_comment(raw).strip()
        if not line:
            continue
        pending = f"{pending} {line}" if pending else line
        if "=" in pending and pending.count("[") > pending.count("]"):
            continue
        entry, pending = pending, ""
        if entry.startswith("[") and entry.endswith("]") and "=" not in entry:
            section = entry.strip("[]").strip()
            if section not in ("json_fields", "fixture_contains"):
                raise CaseFileError(f"unsupported expectations section [{section}]")
            sections.setdefault(section, {})
            continue
        key, sep, value = entry.partition("=")
        if not sep:
            raise CaseFileError(f"expected key = value, got {entry!r}")
        sections[section][key.strip()] = json.loads(value.strip())
    if pending:
        raise CaseFileError("unterminated value in expectations.toml")
    return sections


# --------------------------------------------------------- binary and tools


def resolve_binary() -> Path:
    """Locate this worktree's ``commandagent`` build and refuse a stale one."""
    override = os.environ.get("COMMANDAGENT_BIN")
    if override:
        binary = Path(override)
        if not binary.is_file():
            raise HarnessError(f"COMMANDAGENT_BIN does not exist: {binary}")
        return binary
    target = os.environ.get("CARGO_TARGET_DIR")
    if not target:
        proc = subprocess.run(
            ["cargo", "metadata", "--no-deps", "--format-version", "1"],
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            timeout=120,
            check=False,
        )
        if proc.returncode != 0:
            raise HarnessError(
                f"cargo metadata failed (exit {proc.returncode}): {proc.stderr[-300:]}"
            )
        target = json.loads(proc.stdout)["target_directory"]
    binary = Path(target) / "debug" / "commandagent"
    if not binary.is_file():
        raise HarnessError(f"{binary} is missing; run `cargo build --bin commandagent`")
    newest = _newest_source_mtime()
    if binary.stat().st_mtime < newest:
        raise HarnessError(
            f"{binary} is older than the sources; run `cargo build --bin commandagent`"
        )
    return binary


def _newest_source_mtime() -> float:
    newest = 0.0
    for name in ("Cargo.toml", "Cargo.lock", "build.rs"):
        path = REPO_ROOT / name
        if path.exists():
            newest = max(newest, path.stat().st_mtime)
    for base, _dirs, files in os.walk(REPO_ROOT / "src"):
        for file in files:
            newest = max(newest, os.stat(os.path.join(base, file)).st_mtime)
    return newest


def case_path() -> str:
    """PATH for cases: the system directories plus the directory of each required tool."""
    dirs = list(BASE_PATH)
    for tool in REQUIRED_TOOLS:
        found = shutil.which(tool, path=os.pathsep.join(dirs)) or shutil.which(tool)
        if not found:
            raise HarnessError(f"required tool {tool!r} is not installed")
        parent = str(Path(found).parent)
        if parent not in dirs:
            dirs.append(parent)
    return os.pathsep.join(dirs)


def build_env(root: Path, binary: Path, path: str) -> dict[str, str]:
    home = root / "home"
    env = {
        "PATH": path,
        "HOME": str(home),
        "XDG_STATE_HOME": str(home / ".state"),
        "TMPDIR": str(root / "tmp"),
        "LC_ALL": "C",
        "NO_COLOR": "1",
        "OLLAMA_UNREACHABLE": OLLAMA_UNREACHABLE,
        "BIN": str(binary),
        "T": str(root),
    }
    assert tuple(env) == ENV_NAMES
    return env


# ------------------------------------------------------------ network guard


def _guard_candidates() -> list[NetworkGuard]:
    candidates: list[NetworkGuard] = []
    if sys.platform == "darwin":
        sandbox_exec = shutil.which("sandbox-exec")
        if sandbox_exec:
            candidates.append(
                NetworkGuard(
                    "sandbox-exec", (sandbox_exec, "-p", DARWIN_SANDBOX_PROFILE)
                )
            )
    else:
        # bwrap brings loopback up in its new network namespace itself and is
        # permitted by the distribution's AppArmor profile on hosts that restrict
        # unprivileged user namespaces, so it is tried first.
        bwrap = shutil.which("bwrap")
        if bwrap:
            candidates.append(
                NetworkGuard(
                    "bwrap-net", (bwrap, "--unshare-net", "--dev-bind", "/", "/", "--")
                )
            )
        unshare = shutil.which("unshare")
        if unshare:
            loopback = (sys.executable, "-I", "-c", LOOPBACK_UP_SOURCE)
            candidates.append(
                NetworkGuard(
                    "unshare-user-net",
                    (unshare, "--net", "--map-root-user", "--", *loopback),
                )
            )
            candidates.append(
                NetworkGuard("unshare-net", (unshare, "--net", "--", *loopback))
            )
    return candidates


def probe_guard(
    guard: NetworkGuard, host: str = PROBE_HOST, port: int = PROBE_PORT
) -> dict[str, str]:
    """Attempt outbound TCP and UDP through ``guard`` and return what happened."""
    command = [*guard.prefix, sys.executable, "-I", "-c", PROBE_SOURCE, host, str(port)]
    proc = subprocess.run(
        command,
        capture_output=True,
        text=True,
        timeout=30,
        env={"PATH": os.pathsep.join(BASE_PATH)},
        stdin=subprocess.DEVNULL,
        check=False,
    )
    try:
        return json.loads(proc.stdout.strip().splitlines()[-1])
    except (IndexError, json.JSONDecodeError) as exc:
        raise HarnessError(
            f"network guard {guard.name} probe did not run (exit {proc.returncode}): "
            f"{proc.stderr[-300:]}"
        ) from exc


def require_denied(
    guard: NetworkGuard, host: str = PROBE_HOST, port: int = PROBE_PORT
) -> dict[str, str]:
    """Raise unless an outbound connection attempt is refused by the guard."""
    outcome = probe_guard(guard, host, port)
    denied = {errno.errorcode[number] for number in GUARD_DENIED_ERRNOS}
    for kind in ("tcp", "udp"):
        if outcome.get(kind) not in denied:
            raise HarnessError(
                f"network guard {guard.name} did not refuse outbound {kind}: {outcome}"
            )
    return outcome


def require_loopback(guard: NetworkGuard) -> None:
    """Raise unless a connection to a listener on 127.0.0.1 works inside ``guard``."""
    proc = subprocess.run(
        [*guard.prefix, sys.executable, "-I", "-c", LOOPBACK_PROBE_SOURCE],
        capture_output=True,
        text=True,
        timeout=30,
        env={"PATH": os.pathsep.join(BASE_PATH)},
        stdin=subprocess.DEVNULL,
        check=False,
    )
    if "loopback-ok" not in proc.stdout:
        raise HarnessError(
            f"network guard {guard.name} does not allow loopback (exit "
            f"{proc.returncode}): {proc.stderr[-300:]}"
        )


_GUARD_CACHE: list[NetworkGuard] = []


def detect_guard() -> NetworkGuard:
    """Return the first guard whose refusal of external connections was observed."""
    if _GUARD_CACHE:
        return _GUARD_CACHE[0]
    reasons = []
    for guard in _guard_candidates():
        try:
            require_denied(guard)
            require_loopback(guard)
        except HarnessError as exc:
            reasons.append(str(exc))
            continue
        _GUARD_CACHE.append(guard)
        return guard
    detail = "; ".join(reasons) or "no sandbox-exec, unshare or bwrap on PATH"
    raise HarnessError(f"no working network guard, refusing to run the cases: {detail}")


# ------------------------------------------------------------ process control


def _signal_group(process: subprocess.Popen, sig: int) -> None:
    """Signal the case's own process group, only while its leader is unreaped.

    A reaped leader frees its pid for reuse, so the group id could then name an
    unrelated process group (same defence as tests/eval/support/next_ready.py).
    """
    if process.returncode is not None:
        return
    pgid = process.pid
    assert pgid > 1
    if pgid == os.getpgrp():
        raise HarnessError(f"refusing to signal the harness's own process group {pgid}")
    try:
        os.killpg(pgid, sig)
    except (ProcessLookupError, PermissionError):
        # macOS reports EPERM for a group that only holds zombies.
        pass


def _process_table() -> list[tuple[int, int, str]]:
    """Every process as (pid, pgid, state), read without touching any of them."""
    ps = shutil.which("ps", path=os.pathsep.join(BASE_PATH))
    if not ps:
        raise HarnessError("ps is required to watch the case process group")
    proc = subprocess.run(
        [ps, "-A", "-o", "pid=,pgid=,stat="],
        capture_output=True,
        text=True,
        timeout=30,
        stdin=subprocess.DEVNULL,
        env={"PATH": os.pathsep.join(BASE_PATH)},
        check=False,
    )
    if proc.returncode != 0:
        raise HarnessError(f"ps failed (exit {proc.returncode}): {proc.stderr[-200:]}")
    rows = []
    for line in proc.stdout.splitlines():
        fields = line.split()
        if len(fields) >= 3 and fields[0].isdigit() and fields[1].isdigit():
            rows.append((int(fields[0]), int(fields[1]), fields[2]))
    return rows


def _leader_has_exited(pid: int) -> bool:
    """True once the leader is a zombie; it stays unreaped until ``wait()``."""
    for row_pid, _pgid, state in _process_table():
        if row_pid == pid:
            return state.startswith("Z")
    return True


def _live_members(pgid: int) -> list[int]:
    """Live (non-zombie) processes of the group other than its leader."""
    return [
        pid
        for pid, group, state in _process_table()
        if group == pgid and pid != pgid and not state.startswith("Z")
    ]


def _wait_for_leader_exit(process: subprocess.Popen, timeout_sec: float) -> bool:
    """Wait for the leader to exit without reaping it; False on timeout."""
    deadline = time.monotonic() + timeout_sec
    while not _leader_has_exited(process.pid):
        if time.monotonic() >= deadline:
            return False
        time.sleep(0.02)
    return True


def _stop_group(process: subprocess.Popen, *, leader_exited: bool) -> bool:
    """Kill what remains of the case's group, then reap the leader.

    Returns True when members were still alive after the leader had exited
    (a leak). The leader stays unreaped until the final ``wait()`` so the
    group id cannot have been reused when the signal is sent.
    """
    leaked = leader_exited and bool(_live_members(process.pid))
    _signal_group(process, signal.SIGKILL)
    deadline = time.monotonic() + 3
    while _live_members(process.pid) and time.monotonic() < deadline:
        time.sleep(0.02)
    process.wait()
    return leaked


# ---------------------------------------------------------------- execution


def _clip(path: Path) -> str:
    data = path.read_bytes()
    text = data[:OUTPUT_LIMIT].decode("utf-8", errors="replace")
    return text + (
        f"\n[truncated {len(data) - OUTPUT_LIMIT} bytes]"
        if len(data) > OUTPUT_LIMIT
        else ""
    )


def judge(
    case: Case, exit_code: int | None, stdout: str, stderr: str, root: Path
) -> list[str]:
    """Compare one finished run with the case; every mismatch is a failure."""
    exp = case.expected
    failures: list[str] = []
    if exp.exit_code == "nonzero":
        if exit_code is None or exit_code <= 0:
            failures.append(f"exit {exit_code} (want nonzero)")
    elif exit_code != exp.exit_code:
        failures.append(f"exit {exit_code} (want {exp.exit_code})")
    failures += [
        f"stdout lacks {text!r}" for text in exp.stdout_contains if text not in stdout
    ]
    failures += [
        f"stderr lacks {text!r}" for text in exp.stderr_contains if text not in stderr
    ]
    failures += [
        f"contains forbidden {text!r}"
        for text in exp.must_not_contain
        if text in stdout or text in stderr
    ]
    for path in exp.files_exist:
        if not Path(path.replace("$T", str(root), 1)).exists():
            failures.append(f"missing file {path}")
    for path in exp.files_absent:
        if Path(path.replace("$T", str(root), 1)).exists():
            failures.append(f"unexpected file {path}")
    return failures


def run_case(
    case: Case, binary: Path, guard: NetworkGuard, path: str | None = None
) -> CaseResult:
    """Run one case in a throwaway HOME and workspace, inside the network guard."""
    scratch = tempfile.TemporaryDirectory(prefix="providerless-cli-")
    root = Path(scratch.name).resolve()
    try:
        for sub in ("home/.state/commandagent", "ws", "tmp"):
            (root / sub).mkdir(parents=True)
        env = build_env(root, binary, path or case_path())
        bash = shutil.which("bash", path=env["PATH"])
        if not bash:
            raise HarnessError("bash is not on the case PATH")
        script = "\n".join(case.setup) + "\n(\n" + case.command + "\n)\n"
        out_path, err_path = root / "stdout.log", root / "stderr.log"
        started = time.monotonic()
        timed_out = False
        with out_path.open("wb") as out_file, err_path.open("wb") as err_file:
            proc = subprocess.Popen(
                [*guard.prefix, bash, "-c", script],
                cwd=root / "ws",
                env=env,
                stdin=subprocess.DEVNULL,
                stdout=out_file,
                stderr=err_file,
                start_new_session=True,
            )
            timed_out = not _wait_for_leader_exit(proc, case.timeout_sec)
            leaked = _stop_group(proc, leader_exited=not timed_out)
        exit_code = None if timed_out else proc.returncode
        stdout, stderr = _clip(out_path), _clip(err_path)
        failures = judge(case, exit_code, stdout, stderr, root)
        if timed_out:
            failures.insert(0, f"timeout after {case.timeout_sec}s")
        if leaked:
            failures.append("left owned processes running (reaped by the harness)")
        return CaseResult(
            case_id=case.id,
            failures=failures,
            exit_code=exit_code,
            timed_out=timed_out,
            stdout=stdout,
            stderr=stderr,
            duration_sec=time.monotonic() - started,
            guard=guard.name,
            leaked_processes=leaked,
        )
    finally:
        scratch.cleanup()


def status(case: Case, result: CaseResult) -> str:
    """pass | fail | xfail | xpass; a strict xfail that passes (xpass) is a failure."""
    if case.xfail:
        return "xpass" if result.passed else "xfail"
    return "pass" if result.passed else "fail"


def is_ok(case_status: str) -> bool:
    return case_status in ("pass", "xfail")


def run_all(
    cases: list[Case], binary: Path, guard: NetworkGuard
) -> list[tuple[Case, CaseResult, str]]:
    path = case_path()
    rows = []
    for case in cases:
        result = run_case(case, binary, guard, path)
        rows.append((case, result, status(case, result)))
    return rows


# --------------------------------------------------------------------- main


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--cases", type=Path, default=CASES_PATH)
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--id", action="append", default=[], dest="ids")
    args = parser.parse_args(argv)
    try:
        _manifest, cases = load_cases(args.cases)
    except CaseFileError as exc:
        print(f"ERROR case file: {exc}", file=sys.stderr)
        return 2
    if args.ids:
        unknown = sorted(set(args.ids) - {case.id for case in cases})
        if unknown:
            print(f"ERROR unknown case ids: {unknown}", file=sys.stderr)
            return 2
        cases = [case for case in cases if case.id in args.ids]
    if not cases:
        print("ERROR no cases selected; an empty run is not a pass", file=sys.stderr)
        return 2
    try:
        binary = args.binary or resolve_binary()
        guard = detect_guard()
        rows = run_all(cases, binary, guard)
    except HarnessError as exc:
        print(f"ERROR environment: {exc}", file=sys.stderr)
        return 3
    bad = 0
    for case, result, case_status in rows:
        print(f"{case_status.upper():5} {case.id} {case.purpose}")
        if not is_ok(case_status):
            bad += 1
            for failure in result.failures or ["strict xfail passed (XPASS)"]:
                print(f"   - {failure}")
    print(
        f"{len(rows) - bad} ok, {bad} failed, {len(rows)} total (network guard: {guard.name})"
    )
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
