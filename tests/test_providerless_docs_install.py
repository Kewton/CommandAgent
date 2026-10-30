"""Providerless docs/install/REPL black-box cases (H-37 child C05, Issue #711).

Runs the registered cases from
``tests/corpus/apps/quality-providerless-distribution/fixtures/cases.json``
against this worktree's ``commandagent`` build through the isolated runner in
``tests/cli_cases/harness.py`` (temporary HOME and workspace, allowlisted
environment, verified outbound-network guard, process-group collection). The
harness is reused as it is.

Isolation of the install cases: ``curl``, the installation prefix and ``PATH``
belong to the case. Every install case puts a case-owned ``$T/shim`` first on
``PATH`` and installs only below ``$T``; the user's install directory, the real
network and a real download are never reached. Two kinds of case are kept
apart:

* read-only cases (``--help``, option errors, the ``setup.sh`` checks) get a
  tripwire ``curl`` that records any call and fails;
* mock-HTTP cases get a ``curl`` shim that rewrites the two GitHub URL prefixes
  the installer uses to a loopback mock owned by the case and refuses every
  other URL. The mock binds port 0, announces readiness on a pipe after it
  listens, bounds every wait with a finite deadline, and is stopped and
  collected before the case ends; no fixed sleep or fixed port is used.

The repository root reaches the cases as the shell variable ``SRC`` (set by
``with_repo_root``) so that no host path is stored in the fixture.

Build the binary first (``cargo build --bin commandagent``) or point
``COMMANDAGENT_BIN`` at one.
"""

from __future__ import annotations

import dataclasses
import re
import shlex
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent))

from cli_cases import harness  # noqa: E402

TESTS_DIR = Path(__file__).resolve().parent
CORPUS_DIR = TESTS_DIR / "corpus" / "apps" / "quality-providerless-distribution"
CASES_PATH = CORPUS_DIR / "fixtures" / "cases.json"
EXPECTATIONS_PATH = CORPUS_DIR / "expectations.toml"
DOCTOR_CASES_PATH = (
    TESTS_DIR
    / "corpus"
    / "apps"
    / "quality-providerless-doctor"
    / "fixtures"
    / "cases.json"
)
CI_SCRIPT = harness.REPO_ROOT / "scripts" / "ci.sh"

# This child owns the docs (15), install (13) and REPL (1) domains of the 149-case
# manifest that the CLI/config child (#709) pinned in quality-providerless-core.
OWNED_DOMAINS = ("docs", "install", "repl")
OWNED_COUNTS = {"docs": 15, "install": 13, "repl": 1}
OWNED_TOTAL = 29
MANIFEST_TOTAL = 149
CORE_TOTAL, DOCTOR_TOTAL = 85, 35

# The PM ruled (2026-10-19) that the three audit failures whose expectation no longer
# matched the current behaviour and specification are fixed and enabled, none as an
# xfail: BB-DOCS-001 (67 flags, cli-reference.md:25), BB-DOCS-004 (--workflow is in the
# Actions group and list, cli-reference.md:13) and BB-DOCS-012 (a ${ENV} expansion is
# redacted in --doctor, configuration.md:113-115).
REWRITTEN_CASES = frozenset({"BB-DOCS-001", "BB-DOCS-004", "BB-DOCS-012"})
PENDING_ADJUDICATION: frozenset[str] = frozenset()
# Strict xfail is only for current defects the PM has approved (case id, tracking
# issue and reason). None are approved.
APPROVED_XFAIL: frozenset[str] = frozenset()

MANIFEST, CASES = harness.load_cases(CASES_PATH)
CORE_MANIFEST, CORE_CASES = harness.load_cases()
DOCTOR_MANIFEST, DOCTOR_CASES = harness.load_cases(DOCTOR_CASES_PATH)
CASES_BY_ID = {case.id: case for case in CASES}

# ------------------------------------------------------------ loopback mocks

# Listener mock (docs busy-port cases): listens on an OS-assigned port and only then
# prints "<port> <pid>" on its stdout pipe, which is the ready notification. It stops
# on SIGTERM (or by itself when its deadline in seconds passes) and prints "hits=<n>".
LISTENER_SOURCE = """
import os, signal, socket, sys

hits = 0

def stop(*_):
    raise SystemExit

signal.signal(signal.SIGTERM, stop)
server = socket.socket()
server.bind(("127.0.0.1", 0))
server.listen(8)
server.settimeout(float(sys.argv[1]))
print(server.getsockname()[1], os.getpid(), flush=True)
try:
    while True:
        conn, _ = server.accept()
        hits += 1
        conn.close()
except (SystemExit, socket.timeout):
    pass
finally:
    print("hits=%d" % hits, flush=True)
"""

# HTTP mock (install cases): serves files below argv[1] (404 for anything else),
# appends "GET /<path> <status>" to the log argv[2], stops on SIGTERM or at its
# deadline argv[3] and prints "requests=<n>".
HTTP_MOCK_SOURCE = """
import http.server, os, signal, sys, time

root = os.path.realpath(sys.argv[1])
log = sys.argv[2]
deadline = float(sys.argv[3])
requests = 0

class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        global requests
        requests += 1
        rel = self.path.split("?", 1)[0].lstrip("/")
        full = os.path.realpath(os.path.join(root, rel))
        found = full.startswith(root + os.sep) and os.path.isfile(full)
        with open(log, "a") as fh:
            fh.write("GET /%s %d\\n" % (rel, 200 if found else 404))
        if not found:
            self.send_error(404)
            return
        with open(full, "rb") as fh:
            body = fh.read()
        self.send_response(200)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *args):
        pass

def stop(*_):
    raise SystemExit

signal.signal(signal.SIGTERM, stop)
server = http.server.HTTPServer(("127.0.0.1", 0), Handler)
print(server.server_address[1], os.getpid(), flush=True)
end = time.monotonic() + deadline
try:
    while time.monotonic() < end:
        server.timeout = max(0.1, end - time.monotonic())
        server.handle_request()
except SystemExit:
    pass
finally:
    print("requests=%d" % requests, flush=True)
"""
assert "'" not in LISTENER_SOURCE
assert "'" not in HTTP_MOCK_SOURCE


def _mock_start(source: str, args: str, deadline_sec: int, ready_sec: int) -> str:
    return (
        f"exec 3< <(python3 -I -c '{source}' {args}{deadline_sec})\n"
        f"read -r -t {ready_sec} -u 3 PORT MPID || {{ echo MOCK_NOT_READY; exit 1; }}"
    )


def listener_start(deadline_sec: int = 60, ready_sec: int = 20) -> str:
    return _mock_start(LISTENER_SOURCE, "", deadline_sec, ready_sec)


def http_start(deadline_sec: int = 60, ready_sec: int = 20) -> str:
    args = '"$T/release" "$T/mock-requests.log" '
    return _mock_start(HTTP_MOCK_SOURCE, args, deadline_sec, ready_sec)


def mock_stop(wait_sec: int = 20) -> str:
    return (
        'kill -TERM "$MPID"\n'
        f"read -r -t {wait_sec} -u 3 REPORT || echo MOCK_NO_REPORT\n"
        f"read -r -t {wait_sec} -u 3 _ || true\n"
        'echo "$REPORT"'
    )


PIN_PATH = 'export PATH="$T/shim:$PATH"'

# ----------------------------------------------------------------- fixtures


@pytest.fixture(scope="session")
def binary() -> Path:
    try:
        return harness.resolve_binary()
    except harness.HarnessError as exc:
        pytest.fail(
            f"cannot find this worktree's commandagent build: {exc}", pytrace=False
        )


@pytest.fixture(scope="session")
def guard() -> harness.NetworkGuard:
    try:
        return harness.detect_guard()
    except harness.HarnessError as exc:
        pytest.fail(str(exc), pytrace=False)


@pytest.fixture(scope="session")
def case_path() -> str:
    try:
        return harness.case_path()
    except harness.HarnessError as exc:
        pytest.fail(str(exc), pytrace=False)


def with_repo_root(case: harness.Case) -> harness.Case:
    """Give the case the repository root as the shell variable ``SRC``."""
    root = shlex.quote(str(harness.REPO_ROOT))
    return dataclasses.replace(case, setup=(f"SRC={root}", *case.setup))


def derived(case_id: str, command: str, **expected) -> harness.Case:
    """A case that reuses a registered case's setup with a different command."""
    base = CASES_BY_ID[case_id]
    exp = dataclasses.replace(
        base.expected,
        exit_code=0,
        stdout_contains=(),
        stderr_contains=(),
        must_not_contain=(),
        files_exist=(),
        files_absent=(),
    )
    exp = dataclasses.replace(exp, **expected)
    return dataclasses.replace(
        base, id="BB-TST-001", command=command, expected=exp, timeout_sec=60
    )


def _case_params():
    for case in CASES:
        marks = []
        if case.xfail:
            marks.append(
                pytest.mark.xfail(
                    strict=True, reason=f"{case.xfail_issue}: {case.xfail_reason}"
                )
            )
        yield pytest.param(case, id=case.id, marks=marks)


# ------------------------------------------------------------ registration


def test_registered_cases_match_the_core_manifest() -> None:
    assert harness.manifest_problems(MANIFEST, CASES, OWNED_DOMAINS) == []
    ids = MANIFEST["ids"]
    assert set(ids) == set(OWNED_DOMAINS)
    for domain in OWNED_DOMAINS:
        core_ids = CORE_MANIFEST["ids"][domain]
        assert len(core_ids) == len(set(core_ids)), f"duplicate {domain} ids in core"
        assert ids[domain] == core_ids
        assert MANIFEST["counts"][domain] == OWNED_COUNTS[domain] == len(core_ids)
    assert MANIFEST["total"] == OWNED_TOTAL == sum(OWNED_COUNTS.values())
    registered = [case.id for case in CASES]
    assert len(registered) == len(set(registered)), "duplicate registered ids"
    assert set(MANIFEST["pending_adjudication"]) == PENDING_ADJUDICATION
    assert not set(registered) & PENDING_ADJUDICATION
    owned = {i for domain in OWNED_DOMAINS for i in ids[domain]}
    assert set(registered) | PENDING_ADJUDICATION == owned
    assert len(registered) == OWNED_TOTAL - len(PENDING_ADJUDICATION)


def test_three_children_cover_all_149_cases_without_gap_or_overlap() -> None:
    assert CORE_MANIFEST["total"] == MANIFEST_TOTAL
    assert sum(CORE_MANIFEST["counts"].values()) == MANIFEST_TOTAL
    core_ids = [i for ids in CORE_MANIFEST["ids"].values() for i in ids]
    assert len(core_ids) == len(set(core_ids)) == MANIFEST_TOTAL

    core_registered = {case.id for case in CORE_CASES}
    doctor_registered = {case.id for case in DOCTOR_CASES}
    here_registered = {case.id for case in CASES}
    assert len(core_registered) == len(CORE_CASES) == CORE_TOTAL
    assert len(doctor_registered) == len(DOCTOR_CASES) == DOCTOR_TOTAL
    assert len(here_registered) == len(CASES)

    pending = {
        pid
        for manifest in (CORE_MANIFEST, DOCTOR_MANIFEST, MANIFEST)
        for pid in manifest["pending_adjudication"]
    }
    parts = [core_registered, doctor_registered, here_registered, pending]
    assert sum(len(part) for part in parts) == MANIFEST_TOTAL, "overlap between parts"
    assert set().union(*parts) == set(core_ids), "a manifest id is missing"
    # 85 + 35 + 29 = 149 with the pending ids counted as this child's.
    assert CORE_TOTAL + DOCTOR_TOTAL + OWNED_TOTAL == MANIFEST_TOTAL


def test_rewritten_cases_are_registered_with_the_current_specification() -> None:
    assert not MANIFEST["pending_adjudication"]
    for case_id in REWRITTEN_CASES:
        case = CASES_BY_ID[case_id]
        assert not case.xfail
        assert "Old expectation" in case.note, case_id
    assert CASES_BY_ID["BB-DOCS-001"].expected.stdout_contains == ("COUNT=68",)
    assert "for f in --workflow --prompt " in CASES_BY_ID["BB-DOCS-004"].command
    doctor = CASES_BY_ID["BB-DOCS-012"].expected
    assert "<redacted> (source=preset" in doctor.stdout_contains
    assert "detail=preset:team_base" in doctor.stdout_contains
    for expanded in ("alice-exec", "team-plan"):
        assert expanded in doctor.must_not_contain


def test_expectations_toml_pins_the_same_manifest() -> None:
    expectations = harness.read_expectations(EXPECTATIONS_PATH)
    selector = harness.CASES_SELECTOR
    fields = expectations[""]
    assert fields["case_id"] == "quality-providerless-distribution"
    fields = expectations["json_fields"]
    assert fields[f"{selector}:schema"] == harness.SCHEMA
    assert fields[f"{selector}:manifest.total"] == str(MANIFEST["total"])
    for domain, count in OWNED_COUNTS.items():
        assert fields[f"{selector}:manifest.counts.{domain}"] == str(count)
    listed = expectations["fixture_contains"][selector]
    assert len(listed) == len(set(listed)), "duplicate ids in expectations.toml"
    assert sorted(listed) == sorted(case.id for case in CASES)


def test_only_approved_cases_are_strict_xfail() -> None:
    assert {case.id for case in CASES if case.xfail} == APPROVED_XFAIL


def test_cases_carry_no_audit_secrets_or_host_paths() -> None:
    text = CASES_PATH.read_text(encoding="utf-8")
    for forbidden in (
        "CANARY-SECRET",
        "/Users/",
        "/Volumes/",
        "workspace/temp",
        "workspace/tmp",
        "$CL1",
        "$Q/",
        "sk-",
        "bb-env.sh",
    ):
        assert forbidden not in text, forbidden
    for case in CASES:
        assert case.source, case.id
        assert case.expected.exit_code is not None
        assert not case.xfail


FORBIDDEN_TIMING_AND_PORTS = (
    (re.compile(r"\bsleep\b"), "a fixed sleep"),
    (
        # port 9 is the harness's closed OLLAMA_UNREACHABLE port, refused locally
        re.compile(r"(127\.0\.0\.1|localhost|0\.0\.0\.0):(?!9\b)\d+"),
        "a fixed port in a URL",
    ),
    (re.compile(r"bind\(\([^)]*,\s*[1-9]\d*\)\)"), "a fixed bind port"),
)


def test_no_fixed_sleep_or_fixed_port_in_cases_or_the_mocks() -> None:
    # Commands and setup are scanned; expected output may quote a documented default.
    texts = [text for case in CASES for text in (*case.setup, case.command)]
    texts += [LISTENER_SOURCE, HTTP_MOCK_SOURCE, listener_start(), http_start()]
    texts += [mock_stop()]
    for text in texts:
        for pattern, what in FORBIDDEN_TIMING_AND_PORTS:
            assert not pattern.search(text), what
    assert LISTENER_SOURCE.count('bind(("127.0.0.1", 0))') == 1
    assert 'HTTPServer(("127.0.0.1", 0), Handler)' in HTTP_MOCK_SOURCE


def test_busy_port_cases_own_a_listener_on_an_os_assigned_port() -> None:
    for case_id in ("BB-DOCS-016", "BB-DOCS-017"):
        case = CASES_BY_ID[case_id]
        assert listener_start() in case.setup
        assert mock_stop() in case.command
        assert 'port "$PORT"' in case.command
        assert "MOCK_NOT_READY" in case.expected.must_not_contain
        assert "MOCK_NO_REPORT" in case.expected.must_not_contain


def test_docs_cases_read_the_repository_only_through_src() -> None:
    for case in CASES:
        text = case.command + "\n".join(case.setup)
        if "$SRC" in text:
            assert case.setup[0].startswith(': "${SRC:?'), case.id


# ------------------------------------------------------ install isolation


def _install_cases() -> list[harness.Case]:
    return [CASES_BY_ID[i] for i in MANIFEST["ids"]["install"]]


READ_ONLY_INSTALL = (
    "BB-INS-001",
    "BB-INS-002",
    "BB-INS-003",
    "BB-INS-004",
    "BB-INS-008",
)
SETUP_SCRIPT_CASES = (
    "BB-INS-012",
    "BB-INS-013",
    "BB-INS-014",
    "BB-INS-015",
    "BB-INS-016",
)
MOCK_HTTP_INSTALL = ("BB-INS-005", "BB-INS-006", "BB-INS-007")


def test_install_cases_are_classified_exactly_once() -> None:
    groups = (READ_ONLY_INSTALL, SETUP_SCRIPT_CASES, MOCK_HTTP_INSTALL)
    classified = [case_id for group in groups for case_id in group]
    assert sorted(classified) == sorted(MANIFEST["ids"]["install"])


def test_every_install_case_pins_path_to_its_own_shim_and_prefix() -> None:
    for case in _install_cases():
        setup = "\n".join(case.setup)
        assert PIN_PATH in case.setup, case.id
        assert 'mkdir -p "$T/shim"' in setup, case.id
        text = setup + "\n" + case.command
        for forbidden in ("~/", "$HOME/.local", "/.local/bin", "/.commandagent"):
            assert forbidden not in text, case.id
        if "install.sh" in case.command and "--help" not in case.command:
            for part in case.command.splitlines():
                if "install.sh" in part and "--prefix" in part:
                    assert '--prefix "$T/' in part or "--prefix ''" in part, case.id


def test_read_only_install_cases_use_a_tripwire_curl() -> None:
    for case_id in READ_ONLY_INSTALL:
        case = CASES_BY_ID[case_id]
        setup = "\n".join(case.setup)
        assert "exit 97" in setup, case_id
        assert "http://" not in setup, case_id
        assert "exec 3<" not in setup, case_id
        assert "$T/curl-calls.log" in case.expected.files_absent, case_id


def test_setup_script_cases_use_stub_tools_and_a_refusing_curl() -> None:
    for case_id in SETUP_SCRIPT_CASES:
        setup = "\n".join(CASES_BY_ID[case_id].setup)
        assert "exit 7" in setup, case_id
        for tool in ("rustc", "cargo", "git"):
            assert f'"$T/shim/{tool}"' in setup, (case_id, tool)
        assert "exec 3<" not in setup, case_id


def test_mock_http_cases_start_and_collect_a_loopback_mock() -> None:
    for case_id in MOCK_HTTP_INSTALL:
        case = CASES_BY_ID[case_id]
        assert http_start() in case.setup, case_id
        assert 'echo "$PORT" > "$T/mock.port"' in case.setup, case_id
        assert any('"$T/shim/uname"' in line for line in case.setup), case_id
        assert mock_stop() in case.command, case_id
        assert "$T/curl-refused.log" in case.expected.files_absent, case_id
        assert "$T/home/.local/bin/commandagent" in case.expected.files_absent
        assert "CURL_IS_CASE_SHIM" in case.expected.stdout_contains, case_id
        for guard_text in ("MOCK_NOT_READY", "MOCK_NO_REPORT"):
            assert guard_text in case.expected.must_not_contain, case_id


def test_the_tripwire_curl_records_a_call_and_fails(
    binary: Path, guard: harness.NetworkGuard, case_path: str
) -> None:
    case = derived(
        "BB-INS-001",
        'curl -fsSL https://example.com/release; echo "curl_rc=$?"; cat "$T/curl-calls.log"',
        stdout_contains=("curl_rc=97", "https://example.com/release"),
    )
    result = harness.run_case(with_repo_root(case), binary, guard, case_path)
    assert result.passed, result.failures + [result.stdout, result.stderr]


def test_the_mock_curl_refuses_every_url_outside_the_two_github_prefixes(
    binary: Path, guard: harness.NetworkGuard, case_path: str
) -> None:
    command = "\n".join(
        [
            'curl -fsSL https://example.com/release; echo "curl_rc=$?"',
            'curl -fsSL https://github.com/other/repo/x; echo "other_rc=$?"',
            'curl -fsSL http://192.0.2.1/x; echo "ip_rc=$?"',
            'cat "$T/curl-refused.log"',
            mock_stop(),
        ]
    )
    case = derived(
        "BB-INS-006",
        command,
        stdout_contains=(
            "curl_rc=7",
            "other_rc=7",
            "ip_rc=7",
            "https://example.com/release",
            "http://192.0.2.1/x",
            "requests=0",
        ),
        must_not_contain=("MOCK_NOT_READY", "MOCK_NO_REPORT"),
    )
    result = harness.run_case(with_repo_root(case), binary, guard, case_path)
    assert result.passed, result.failures + [result.stdout, result.stderr]
    assert not result.leaked_processes


def test_the_mock_curl_answers_only_from_the_case_owned_mock(
    binary: Path, guard: harness.NetworkGuard, case_path: str
) -> None:
    command = "\n".join(
        [
            "curl -fsSL https://github.com/Kewton/CommandAgent/releases/download/"
            'nothing/here; echo "missing_rc=$?"',
            mock_stop(),
            'cat "$T/mock-requests.log"',
        ]
    )
    case = derived(
        "BB-INS-006",
        command,
        stdout_contains=(
            "missing_rc=22",
            "requests=1",
            "GET /download/nothing/here 404",
        ),
        must_not_contain=("MOCK_NOT_READY", "MOCK_NO_REPORT"),
    )
    result = harness.run_case(with_repo_root(case), binary, guard, case_path)
    assert result.passed, result.failures + [result.stdout, result.stderr]
    assert not result.leaked_processes


def test_a_case_without_the_repository_root_fails_closed(
    binary: Path, guard: harness.NetworkGuard, case_path: str
) -> None:
    result = harness.run_case(CASES_BY_ID["BB-INS-001"], binary, guard, case_path)
    assert not result.passed
    assert result.exit_code != 0


# --------------------------------------------------------------- CI wiring


def test_ci_script_runs_the_three_pytest_modules_after_building_the_binary() -> None:
    text = CI_SCRIPT.read_text(encoding="utf-8")
    build = text.index("cargo build --bin commandagent")
    match = re.search(r"python3 -m pytest (?:[^\n\\]|\\\n)*", text)
    assert match, "ci.sh must run pytest, not the runner alone"
    assert build < match.start(), "the binary must be built before the pytest modules"
    command = match.group(0)
    for module in (
        "tests/test_providerless_cli.py",
        "tests/test_providerless_doctor.py",
        "tests/test_providerless_docs_install.py",
    ):
        assert command.count(module) == 1, module
        assert (harness.REPO_ROOT / module).is_file(), module
    assert "--junitxml" in command, "the run must leave a count of executed tests"
    summary = text[match.end() :]
    assert 'c.find("failure")' in summary and 'findall("skipped")' in summary
    assert "failed or skipped" in summary, "a skipped test must not count as a pass"


# ------------------------------------------------------------- the cases


@pytest.mark.parametrize("case", list(_case_params()))
def test_case(
    case: harness.Case, binary: Path, guard: harness.NetworkGuard, case_path: str
) -> None:
    result = harness.run_case(with_repo_root(case), binary, guard, case_path)
    assert result.guard == guard.name
    assert not result.leaked_processes, f"{case.id} left owned processes running"
    assert result.passed, (
        f"{case.id} {case.purpose}\n  "
        + "\n  ".join(result.failures)
        + (f"\n--- stdout\n{result.stdout[-800:]}\n--- stderr\n{result.stderr[-800:]}")
    )


# ------------------------------------------------------- Linux guard design


def test_cases_run_with_external_connections_refused(
    guard: harness.NetworkGuard,
) -> None:
    outcome = harness.require_denied(guard)
    assert outcome["tcp"] != "connected"
    assert outcome["udp"] != "connected"


def test_loopback_works_inside_the_guard(guard: harness.NetworkGuard) -> None:
    harness.require_loopback(guard)


def test_a_guard_without_loopback_is_rejected(monkeypatch: pytest.MonkeyPatch) -> None:
    class Done:
        returncode = 1
        stdout = ""
        stderr = "connection refused"

    monkeypatch.setattr(harness.subprocess, "run", lambda *a, **k: Done())
    with pytest.raises(harness.HarnessError, match="does not allow loopback"):
        harness.require_loopback(harness.NetworkGuard("lo-down", ()))


def test_linux_guards_prefer_bwrap_and_bring_loopback_up_in_unshare(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    tools = {"bwrap": "/x/bwrap", "unshare": "/x/unshare"}
    monkeypatch.setattr(harness.sys, "platform", "linux")
    monkeypatch.setattr(harness.shutil, "which", lambda name, *a, **k: tools.get(name))
    candidates = harness._guard_candidates()
    assert [guard.name for guard in candidates] == [
        "bwrap-net",
        "unshare-user-net",
        "unshare-net",
    ]
    assert "--unshare-net" in candidates[0].prefix
    for guard in candidates[1:]:
        assert "--net" in guard.prefix
        assert harness.LOOPBACK_UP_SOURCE in guard.prefix
        assert guard.prefix[-4:] == (
            sys.executable,
            "-I",
            "-c",
            harness.LOOPBACK_UP_SOURCE,
        )
    monkeypatch.setattr(harness.shutil, "which", lambda name, *a, **k: None)
    assert harness._guard_candidates() == []


def test_the_macos_guard_is_unchanged(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(harness.sys, "platform", "darwin")
    monkeypatch.setattr(harness.shutil, "which", lambda name, *a, **k: "/x/" + name)
    (guard,) = harness._guard_candidates()
    assert guard.name == "sandbox-exec"
    assert guard.prefix == ("/x/sandbox-exec", "-p", harness.DARWIN_SANDBOX_PROFILE)
