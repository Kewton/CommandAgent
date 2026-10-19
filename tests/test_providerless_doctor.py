"""Providerless doctor/direct-action black-box cases (H-37 child C04, Issue #710).

Runs the registered cases from
``tests/corpus/apps/quality-providerless-doctor/fixtures/cases.json`` against this
worktree's ``commandagent`` build through the isolated runner in
``tests/cli_cases/harness.py`` (temporary HOME and workspace, allowlisted
environment, verified outbound-network guard, process-group collection). The
harness is reused as it is.

The only provider stand-in is a loopback mock owned by the case itself (see
``MOCK_*`` below): it binds port 0, announces readiness on a pipe after it
listens, bounds every wait with a finite deadline, and is stopped and collected
before the case ends. A mock left running fails the case.

Build the binary first (``cargo build --bin commandagent``) or point
``COMMANDAGENT_BIN`` at one.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent))

from cli_cases import harness  # noqa: E402

CORPUS_DIR = (
    Path(__file__).resolve().parent / "corpus" / "apps" / "quality-providerless-doctor"
)
CASES_PATH = CORPUS_DIR / "fixtures" / "cases.json"
EXPECTATIONS_PATH = CORPUS_DIR / "expectations.toml"

# This child owns the doctor domain (35 cases) of the 149-case manifest that the
# CLI/config child (#709) pinned in quality-providerless-core.
OWNED_DOMAINS = ("doctor",)
OWNED_TOTAL = 35

# The PM ruled (2026-10) that the five cases whose audit expectation no longer matched
# the specification are rewritten to the current specification and registered, none as
# an xfail: BB-DOC-003 (cli-reference.md:70, doctor checks the provider host),
# BB-DOC-026/028/029 (headless.md:14-17, no summary for --ux-demo or an empty --runs)
# and BB-DOC-030 (a failed --run-plan run is recorded and listed by --runs).
REWRITTEN_CASES = frozenset(
    {"BB-DOC-003", "BB-DOC-026", "BB-DOC-028", "BB-DOC-029", "BB-DOC-030"}
)
PENDING_ADJUDICATION: frozenset[str] = frozenset()
# Strict xfail is only for current defects the PM has approved (case id, tracking
# issue and reason). None are approved.
APPROVED_XFAIL: frozenset[str] = frozenset()

MANIFEST, CASES = harness.load_cases(CASES_PATH)
CORE_MANIFEST, _CORE_CASES = harness.load_cases()
CASES_BY_ID = {case.id: case for case in CASES}

# ------------------------------------------------------------ loopback mock

# One mock server per case. It listens on an OS-assigned port (port 0) and only
# then prints "<port> <pid>" on its stdout pipe, which is the ready notification.
# It prints "hit" for every accepted connection, stops on SIGTERM (or by itself
# when its deadline in seconds passes) and prints "hits=<n>" before it exits.
MOCK_SOURCE = """
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
        print("hit", flush=True)
except (SystemExit, socket.timeout):
    pass
finally:
    print("hits=%d" % hits, flush=True)
"""
assert "'" not in MOCK_SOURCE


def mock_start(deadline_sec: int = 30, ready_sec: int = 20) -> str:
    """Start the mock on fd 3 and read its ready line into PORT and MPID."""
    return (
        f"exec 3< <(python3 -I -c '{MOCK_SOURCE}' {deadline_sec})\n"
        f"read -r -t {ready_sec} -u 3 PORT MPID || {{ echo MOCK_NOT_READY; exit 1; }}"
    )


def mock_wait_hit(wait_sec: int = 20) -> str:
    """Wait (with a deadline) for the mock to have accepted a connection."""
    return f"read -r -t {wait_sec} -u 3 HIT || echo MOCK_NO_HIT"


def mock_stop(wait_sec: int = 20) -> str:
    """Stop the mock, print its report, and wait for its pipe to close."""
    return (
        'kill -TERM "$MPID"\n'
        f"read -r -t {wait_sec} -u 3 REPORT || echo MOCK_NO_REPORT\n"
        f"read -r -t {wait_sec} -u 3 _ || true\n"
        'echo "$REPORT"'
    )


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


def mock_case(command: str, **expected) -> harness.Case:
    exp = {
        "exit_code": 0,
        "stdout_contains": [],
        "stderr_contains": [],
        "must_not_contain": ["MOCK_NOT_READY", "MOCK_NO_HIT", "MOCK_NO_REPORT"],
        "files_exist": [],
        "files_absent": [],
    }
    exp.update(expected)
    return harness.parse_case(
        {
            "id": "BB-TST-001",
            "area": "mock-self-test",
            "purpose": "loopback mock self test",
            "source": "tests/test_providerless_doctor.py",
            "setup": [],
            "command": command,
            "timeout_sec": 60,
            "expected": exp,
        }
    )


CONNECT = (
    "python3 -I -c 'import socket, sys; "
    'socket.create_connection(("127.0.0.1", int(sys.argv[1])), 10).close()\' "$PORT"'
)

# ------------------------------------------------------------ registration


def test_registered_cases_match_the_core_manifest() -> None:
    assert harness.manifest_problems(MANIFEST, CASES, OWNED_DOMAINS) == []
    core_ids = CORE_MANIFEST["ids"]["doctor"]
    assert len(core_ids) == OWNED_TOTAL == CORE_MANIFEST["counts"]["doctor"]
    assert len(core_ids) == len(set(core_ids)), "duplicate doctor ids in core"
    assert MANIFEST["ids"] == {"doctor": core_ids}
    assert MANIFEST["counts"] == {"doctor": OWNED_TOTAL}
    assert MANIFEST["total"] == OWNED_TOTAL
    registered = [case.id for case in CASES]
    assert len(registered) == len(set(registered)), "duplicate registered ids"
    assert set(MANIFEST["pending_adjudication"]) == PENDING_ADJUDICATION
    assert len(registered) == OWNED_TOTAL
    assert set(registered) | PENDING_ADJUDICATION == set(core_ids)
    assert not set(registered) & PENDING_ADJUDICATION
    assert len(registered) == OWNED_TOTAL - len(PENDING_ADJUDICATION)


def test_core_manifest_still_covers_all_149_cases() -> None:
    assert CORE_MANIFEST["total"] == 149
    assert sum(CORE_MANIFEST["counts"].values()) == 149


def test_rewritten_cases_are_registered_with_the_current_specification() -> None:
    assert not MANIFEST["pending_adjudication"]
    for case_id in REWRITTEN_CASES:
        case = CASES_BY_ID[case_id]
        assert not case.xfail
        assert case.note, case_id
    doctor = CASES_BY_ID["BB-DOC-003"]
    assert mock_start() in doctor.setup
    assert mock_wait_hit() in doctor.command
    assert mock_stop() in doctor.command
    assert "hits=0" in doctor.expected.must_not_contain
    for case_id in ("BB-DOC-028", "BB-DOC-029"):
        forbidden = CASES_BY_ID[case_id].expected.must_not_contain
        assert "headless-summary" in forbidden
        assert "SUMMARY_JSON_PRESENT" in forbidden
    assert CASES_BY_ID["BB-DOC-026"].expected.stdout_contains == (
        "No runs found for this workspace.",
        "ONLY_NO_RUNS_LINE",
    )
    assert "run_plan_rc=1" in CASES_BY_ID["BB-DOC-030"].expected.stdout_contains


def test_expectations_toml_pins_the_same_manifest() -> None:
    expectations = harness.read_expectations(EXPECTATIONS_PATH)
    selector = harness.CASES_SELECTOR
    fields = expectations[""]
    assert fields["case_id"] == "quality-providerless-doctor"
    fields = expectations["json_fields"]
    assert fields[f"{selector}:schema"] == harness.SCHEMA
    assert fields[f"{selector}:manifest.total"] == str(MANIFEST["total"])
    assert fields[f"{selector}:manifest.counts.doctor"] == str(OWNED_TOTAL)
    listed = expectations["fixture_contains"][selector]
    assert len(listed) == len(set(listed)), "duplicate ids in expectations.toml"
    assert sorted(listed) == sorted(MANIFEST["ids"]["doctor"])


def test_only_approved_cases_are_strict_xfail() -> None:
    assert {case.id for case in CASES if case.xfail} == APPROVED_XFAIL


def test_cases_carry_no_audit_secrets_or_host_paths() -> None:
    text = CASES_PATH.read_text(encoding="utf-8")
    for forbidden in (
        "CANARY-SECRET",
        "/Users/",
        "/Volumes/",
        "workspace/temp",
        "$CL1",
        "$Q/",
        "sk-",
    ):
        assert forbidden not in text, forbidden
    for case in CASES:
        assert case.source, case.id
        assert case.expected.exit_code is not None


FORBIDDEN_TIMING_AND_PORTS = (
    (re.compile(r"\bsleep\b"), "a fixed sleep"),
    (re.compile(r"(127\.0\.0\.1|localhost|0\.0\.0\.0):\d+"), "a fixed port in a URL"),
    (re.compile(r"bind\(\([^)]*,\s*[1-9]\d*\)\)"), "a fixed bind port"),
)


def test_no_fixed_sleep_or_fixed_port_in_cases_or_the_mock() -> None:
    texts = [CASES_PATH.read_text(encoding="utf-8"), MOCK_SOURCE]
    texts += [mock_start(), mock_wait_hit(), mock_stop()]
    for text in texts:
        for pattern, what in FORBIDDEN_TIMING_AND_PORTS:
            assert not pattern.search(text), what
    assert 'bind(("127.0.0.1", 0))' in MOCK_SOURCE


@pytest.mark.parametrize("case", list(_case_params()))
def test_case(
    case: harness.Case, binary: Path, guard: harness.NetworkGuard, case_path: str
) -> None:
    result = harness.run_case(case, binary, guard, case_path)
    assert result.guard == guard.name
    assert not result.leaked_processes, f"{case.id} left owned processes running"
    assert result.passed, (
        f"{case.id} {case.purpose}\n  "
        + "\n  ".join(result.failures)
        + (f"\n--- stdout\n{result.stdout[-800:]}\n--- stderr\n{result.stderr[-800:]}")
    )


# ------------------------------------------------------- loopback mock design


def test_mock_binds_an_os_assigned_port_and_counts_a_connection(
    binary: Path, guard: harness.NetworkGuard, case_path: str
) -> None:
    command = "\n".join(
        [
            mock_start(),
            'echo "port=$PORT"',
            CONNECT,
            mock_wait_hit(),
            mock_stop(),
        ]
    )
    result = harness.run_case(
        mock_case(command, stdout_contains=["port=", "hits=1"]),
        binary,
        guard,
        case_path,
    )
    assert result.passed, result.failures + [result.stdout, result.stderr]
    assert not result.leaked_processes
    port = int(re.search(r"port=(\d+)", result.stdout).group(1))
    assert 1024 <= port <= 65535


def test_mock_stops_by_itself_at_its_deadline(
    binary: Path, guard: harness.NetworkGuard, case_path: str
) -> None:
    command = "\n".join(
        [
            mock_start(deadline_sec=1),
            'read -r -t 20 -u 3 REPORT || echo MOCK_NO_REPORT; echo "$REPORT"',
            "read -r -t 20 -u 3 _ || true",
        ]
    )
    result = harness.run_case(
        mock_case(command, stdout_contains=["hits=0"]), binary, guard, case_path
    )
    assert result.passed, result.failures + [result.stdout, result.stderr]
    assert not result.leaked_processes


def test_a_mock_left_running_fails_the_case(
    binary: Path, guard: harness.NetworkGuard, case_path: str
) -> None:
    command = mock_start() + '\necho "mock=$MPID"'
    result = harness.run_case(mock_case(command), binary, guard, case_path)
    assert result.leaked_processes
    assert not result.passed
    assert any("left owned processes" in failure for failure in result.failures)


def test_an_unready_mock_is_a_failure_not_a_hang(
    binary: Path, guard: harness.NetworkGuard, case_path: str
) -> None:
    command = "exec 3< <(python3 -I -c 'import sys; sys.exit(3)')\n" + (
        "read -r -t 20 -u 3 PORT MPID || { echo MOCK_NOT_READY; exit 1; }"
    )
    result = harness.run_case(mock_case(command), binary, guard, case_path)
    assert not result.passed
    assert "MOCK_NOT_READY" in result.stdout
    assert not result.timed_out


def test_cases_run_with_external_connections_refused(
    guard: harness.NetworkGuard,
) -> None:
    outcome = harness.require_denied(guard)
    assert outcome["tcp"] != "connected"
    assert outcome["udp"] != "connected"
