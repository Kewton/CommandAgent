"""Providerless CLI/config black-box cases (H-37 child C03, Issue #709).

Runs the registered cases from
``tests/corpus/apps/quality-providerless-core/fixtures/cases.json`` against this
worktree's ``commandagent`` build through ``tests/cli_cases/harness.py``, and
checks that the harness itself fails closed: a mismatch, a timeout, a leaked
process or a missing network guard can never read as a pass.

Build the binary first (``cargo build --bin commandagent``) or point
``COMMANDAGENT_BIN`` at one.
"""

from __future__ import annotations

import copy
import errno
import json
import os
import shlex
import socket
import subprocess
import sys
import time
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent))

from cli_cases import harness  # noqa: E402

# This child owns the CLI (54) and config (31) domains of the 149-case manifest.
OWNED_DOMAINS = ("cli", "config")
OWNED_TOTAL = 85

# Old audit failures whose expectation the PM has not ruled on yet. They are not
# registered as cases: the manifest lists them under pending_adjudication, and this
# set must change together with that list and with the PM's ruling, never silently.
PENDING_ADJUDICATION = frozenset(
    {
        "BB-CLI-005",
        "BB-CLI-041",
        "BB-CLI-042",
        "BB-CLI-049",
        "BB-CLI-050",
        "BB-CLI-051",
        "BB-CFG-008",
        "BB-CFG-018",
        "BB-CFG-020",
    }
)
# Strict xfail is only for current defects the PM has approved (case id, tracking
# issue and reason). None are approved.
APPROVED_XFAIL: frozenset[str] = frozenset()

MANIFEST, CASES = harness.load_cases()
CASES_BY_ID = {case.id: case for case in CASES}


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


def make_case(
    case_id: str = "BB-TST-001", command: str = "true", **overrides
) -> harness.Case:
    raw = case_dict(case_id, command, **overrides)
    return harness.parse_case(raw)


def case_dict(
    case_id: str, command: str, setup=(), timeout_sec: int = 20, **expected
) -> dict:
    exp = {
        "exit_code": 0,
        "stdout_contains": [],
        "stderr_contains": [],
        "must_not_contain": [],
        "files_exist": [],
        "files_absent": [],
    }
    exp.update(expected)
    return {
        "id": case_id,
        "area": "harness-self-test",
        "purpose": "harness self test",
        "source": "tests/test_providerless_cli.py",
        "setup": list(setup),
        "command": command,
        "timeout_sec": timeout_sec,
        "expected": exp,
    }


def assert_gone(pid: int) -> None:
    """The process is gone (ps lists no live process; a zombie awaiting init counts as gone)."""
    for _ in range(100):
        proc = subprocess.run(
            ["ps", "-o", "stat=", "-p", str(pid)],
            capture_output=True,
            text=True,
            check=False,
        )
        state = proc.stdout.strip()
        if proc.returncode != 0 or not state or state.startswith("Z"):
            return
        time.sleep(0.05)
    pytest.fail(f"process {pid} is still running")


def write_document(path: Path, cases: list[dict]) -> Path:
    doc = {
        "schema": harness.SCHEMA,
        "manifest": {
            "total": len(cases),
            "counts": {"cli": len(cases)},
            "ids": {"cli": [c["id"] for c in cases]},
            "pending_adjudication": [],
        },
        "cases": cases,
    }
    path.write_text(json.dumps(doc), encoding="utf-8")
    return path


# ------------------------------------------------------------ registration


def test_registered_cases_match_the_manifest() -> None:
    assert harness.manifest_problems(MANIFEST, CASES, OWNED_DOMAINS) == []
    ids = [case.id for case in CASES]
    assert len(ids) == len(set(ids)), "duplicate case ids"
    owned = {i for domain in OWNED_DOMAINS for i in MANIFEST["ids"][domain]}
    assert len(owned) == OWNED_TOTAL
    assert set(MANIFEST["pending_adjudication"]) == PENDING_ADJUDICATION
    assert set(ids) == owned - PENDING_ADJUDICATION
    assert len(ids) + len(PENDING_ADJUDICATION) == OWNED_TOTAL


def test_manifest_counts_cover_all_149_cases() -> None:
    assert MANIFEST["total"] == 149
    assert MANIFEST["counts"] == {
        "cli": 54,
        "config": 31,
        "doctor": 35,
        "docs": 15,
        "install": 13,
        "repl": 1,
    }
    for domain, count in MANIFEST["counts"].items():
        assert len(MANIFEST["ids"][domain]) == count


def test_expectations_toml_pins_the_same_manifest() -> None:
    expectations = harness.read_expectations()
    selector = harness.CASES_SELECTOR
    fields = expectations["json_fields"]
    assert fields[f"{selector}:schema"] == harness.SCHEMA
    assert fields[f"{selector}:manifest.total"] == str(MANIFEST["total"])
    for domain, count in MANIFEST["counts"].items():
        assert fields[f"{selector}:manifest.counts.{domain}"] == str(count)
    listed = expectations["fixture_contains"][selector]
    assert len(listed) == len(set(listed)), "duplicate ids in expectations.toml"
    assert sorted(listed) == sorted(i for ids in MANIFEST["ids"].values() for i in ids)


def test_only_approved_cases_are_strict_xfail() -> None:
    assert {case.id for case in CASES if case.xfail} == APPROVED_XFAIL


def test_cases_carry_no_audit_secrets_or_host_paths() -> None:
    text = harness.CASES_PATH.read_text(encoding="utf-8")
    for forbidden in (
        "CANARY-SECRET",
        "/Users/",
        "/Volumes/",
        "workspace/temp",
        "$CL1",
        "$Q/",
    ):
        assert forbidden not in text, forbidden
    for case in CASES:
        assert case.source, case.id
        assert case.expected.exit_code is not None


@pytest.mark.parametrize("case", list(_case_params()))
def test_case(
    case: harness.Case, binary: Path, guard: harness.NetworkGuard, case_path: str
) -> None:
    result = harness.run_case(case, binary, guard, case_path)
    assert result.guard == guard.name
    assert result.passed, (
        f"{case.id} {case.purpose}\n  "
        + "\n  ".join(result.failures)
        + (f"\n--- stdout\n{result.stdout[-800:]}\n--- stderr\n{result.stderr[-800:]}")
    )


# ----------------------------------------------------------- case file schema


def _mutations():
    def drop(key):
        return lambda raw: raw.pop(key)

    def set_exit(value):
        return lambda raw: raw["expected"].__setitem__("exit_code", value)

    return {
        "missing-command": drop("command"),
        "unknown-key": lambda raw: raw.__setitem__("runner", "x"),
        "bad-id": lambda raw: raw.__setitem__("id", "CLI-1"),
        "zero-timeout": lambda raw: raw.__setitem__("timeout_sec", 0),
        "huge-timeout": lambda raw: raw.__setitem__("timeout_sec", 100000),
        "nonzero_or_zero": set_exit("nonzero_or_zero"),
        "exit-300": set_exit(300),
        "bool-exit": set_exit(True),
        "file-outside-T": lambda raw: raw["expected"]["files_exist"].append(
            "/etc/passwd"
        ),
        "file-traversal": lambda raw: raw["expected"]["files_absent"].append("$T/../x"),
        "xfail-without-issue": lambda raw: raw.__setitem__(
            "xfail", {"issue": "soon", "reason": "r"}
        ),
        "xfail-without-reason": lambda raw: raw.__setitem__("xfail", {"issue": "#1"}),
    }


@pytest.mark.parametrize("name", sorted(_mutations()))
def test_schema_rejects_malformed_cases(name: str) -> None:
    raw = case_dict("BB-TST-001", "true")
    _mutations()[name](raw)
    with pytest.raises(harness.CaseFileError):
        harness.parse_case(raw)


def test_schema_rejects_duplicate_ids_and_wrong_schema() -> None:
    one = case_dict("BB-TST-001", "true")
    doc = {
        "schema": harness.SCHEMA,
        "manifest": {"total": 2, "counts": {}, "ids": {}, "pending_adjudication": []},
        "cases": [one, copy.deepcopy(one)],
    }
    with pytest.raises(harness.CaseFileError, match="duplicate"):
        harness.parse_document(doc)
    doc["schema"] = "other/v1"
    with pytest.raises(harness.CaseFileError, match="schema"):
        harness.parse_document(doc)


def test_manifest_check_reports_missing_duplicate_and_foreign_cases() -> None:
    cases = [make_case("BB-CLI-001"), make_case("BB-CLI-002")]
    good = {
        "total": 3,
        "counts": {"cli": 3},
        "ids": {"cli": ["BB-CLI-001", "BB-CLI-002", "BB-CLI-003"]},
        "pending_adjudication": [],
    }
    assert any(
        "BB-CLI-003" in p for p in harness.manifest_problems(good, cases, ("cli",))
    )
    dup = copy.deepcopy(good)
    dup["ids"]["cli"] = ["BB-CLI-001", "BB-CLI-001", "BB-CLI-002"]
    assert any(
        "duplicate" in p for p in harness.manifest_problems(dup, cases, ("cli",))
    )
    wrong_count = copy.deepcopy(good)
    wrong_count["counts"]["cli"] = 2
    assert any(
        "counts.cli" in p
        for p in harness.manifest_problems(wrong_count, cases, ("cli",))
    )
    foreign = [*cases, make_case("BB-DOC-001")]
    assert any(
        "outside" in p for p in harness.manifest_problems(good, foreign, ("cli",))
    )
    both = copy.deepcopy(good)
    both["pending_adjudication"] = ["BB-CLI-001"]
    assert any(
        "both registered and pending" in p
        for p in harness.manifest_problems(both, cases, ("cli",))
    )


def test_expectations_reader_rejects_sections_the_walker_rejects(
    tmp_path: Path,
) -> None:
    path = tmp_path / "expectations.toml"
    path.write_text('case_id = "x"\n[manifest]\ntotal = "1"\n', encoding="utf-8")
    with pytest.raises(harness.CaseFileError, match="unsupported"):
        harness.read_expectations(path)


# --------------------------------------------- runner exits non-zero on mismatch


def _run_cli(
    binary: Path, cases_file: Path, *extra: str
) -> subprocess.CompletedProcess:
    return subprocess.run(
        [
            sys.executable,
            "-I",
            str(Path(harness.__file__)),
            "--cases",
            str(cases_file),
            "--binary",
            str(binary),
            *extra,
        ],
        capture_output=True,
        text=True,
        timeout=120,
        check=False,
    )


def test_runner_exits_zero_when_every_case_matches(
    binary: Path, tmp_path: Path
) -> None:
    cases = [
        case_dict(
            "BB-TST-001",
            '"$BIN" --version </dev/null',
            stdout_contains=["commandagent "],
        ),
        case_dict(
            "BB-TST-002",
            '"$BIN" --bogus </dev/null',
            exit_code=2,
            stderr_contains=["unexpected argument"],
        ),
    ]
    proc = _run_cli(binary, write_document(tmp_path / "ok.json", cases))
    assert proc.returncode == 0, proc.stdout + proc.stderr
    assert "2 ok, 0 failed" in proc.stdout


@pytest.mark.parametrize(
    ("name", "command", "expected"),
    [
        ("exit-mismatch", '"$BIN" --version </dev/null', {"exit_code": 1}),
        (
            "stdout-mismatch",
            '"$BIN" --version </dev/null',
            {"stdout_contains": ["never printed by --version"]},
        ),
        (
            "forbidden-text",
            '"$BIN" --version </dev/null',
            {"must_not_contain": ["commandagent"]},
        ),
        ("nonzero-but-zero", '"$BIN" --version </dev/null', {"exit_code": "nonzero"}),
        ("missing-file", "true", {"files_exist": ["$T/ws/never-created"]}),
        ("present-file", "touch present", {"files_absent": ["$T/ws/present"]}),
    ],
)
def test_runner_exits_nonzero_on_a_mismatch(
    binary: Path, tmp_path: Path, name: str, command: str, expected: dict
) -> None:
    good = case_dict("BB-TST-001", "true")
    bad = case_dict("BB-TST-002", command, **expected)
    proc = _run_cli(binary, write_document(tmp_path / f"{name}.json", [good, bad]))
    assert proc.returncode == 1, proc.stdout + proc.stderr
    assert "FAIL  BB-TST-002" in proc.stdout
    assert "PASS  BB-TST-001" in proc.stdout


def test_runner_exits_nonzero_on_a_timeout(binary: Path, tmp_path: Path) -> None:
    cases = [case_dict("BB-TST-001", "sleep 30", timeout_sec=1, exit_code="nonzero")]
    proc = _run_cli(binary, write_document(tmp_path / "slow.json", cases))
    assert proc.returncode == 1, proc.stdout + proc.stderr
    assert "timeout after 1s" in proc.stdout


def test_runner_rejects_an_empty_or_unknown_selection(
    binary: Path, tmp_path: Path
) -> None:
    file = write_document(tmp_path / "one.json", [case_dict("BB-TST-001", "true")])
    assert _run_cli(binary, file, "--id", "BB-TST-999").returncode == 2
    empty = write_document(tmp_path / "empty.json", [])
    assert _run_cli(binary, empty).returncode == 2
    broken = tmp_path / "broken.json"
    broken.write_text("{", encoding="utf-8")
    assert _run_cli(binary, broken).returncode == 2


def test_strict_xfail_that_passes_fails_the_run(binary: Path, tmp_path: Path) -> None:
    raw = case_dict("BB-TST-001", "true")
    raw["xfail"] = {"issue": "#1", "reason": "self test"}
    proc = _run_cli(binary, write_document(tmp_path / "xpass.json", [raw]))
    assert proc.returncode == 1, proc.stdout
    assert "XPASS" in proc.stdout
    failing = case_dict("BB-TST-001", "false", exit_code=0)
    failing["xfail"] = {"issue": "#1", "reason": "self test"}
    proc = _run_cli(binary, write_document(tmp_path / "xfail.json", [failing]))
    assert proc.returncode == 0, proc.stdout
    assert "XFAIL" in proc.stdout


def test_missing_environment_is_exit_three_not_a_pass(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path, binary: Path
) -> None:
    file = write_document(tmp_path / "one.json", [case_dict("BB-TST-001", "true")])

    def no_guard() -> harness.NetworkGuard:
        raise harness.HarnessError("no working network guard")

    monkeypatch.setattr(harness, "detect_guard", no_guard)
    assert harness.main(["--cases", str(file), "--binary", str(binary)]) == 3


# --------------------------------------------------------- isolation behaviour


def test_environment_is_an_allowlist(
    monkeypatch: pytest.MonkeyPatch,
    binary: Path,
    guard: harness.NetworkGuard,
    case_path: str,
) -> None:
    for name in (
        "OPENAI_API_KEY",
        "GEMINI_API_KEY",
        "CARGO_HOME",
        "RUSTUP_HOME",
        "COMMANDAGENT_EVAL_EVENTS",
    ):
        monkeypatch.setenv(name, "leak-sentinel")
    result = harness.run_case(make_case(command="env"), binary, guard, case_path)
    assert result.passed, result.failures
    names = {
        line.split("=", 1)[0] for line in result.stdout.splitlines() if "=" in line
    }
    shell_added = {"PWD", "SHLVL", "_", "OLDPWD"}
    assert names - shell_added == set(harness.ENV_NAMES)
    assert "leak-sentinel" not in result.stdout
    env = dict(line.split("=", 1) for line in result.stdout.splitlines() if "=" in line)
    assert env["HOME"] == f"{env['T']}/home"
    assert env["XDG_STATE_HOME"] == f"{env['T']}/home/.state"


def test_home_and_workspace_are_temporary_and_removed(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    binary: Path,
    guard: harness.NetworkGuard,
    case_path: str,
) -> None:
    real_home = tmp_path / "real-home"
    real_home.mkdir()
    monkeypatch.setenv("HOME", str(real_home))
    command = (
        'echo "$T"; pwd; touch "$HOME/marker" ws-file; mkdir -p "$HOME/.commandagent"'
    )
    result = harness.run_case(make_case(command=command), binary, guard, case_path)
    assert result.passed, result.failures
    root, cwd = result.stdout.splitlines()[:2]
    assert Path(cwd) == Path(root) / "ws"
    assert not Path(root).exists(), "case directory was not removed"
    assert list(real_home.iterdir()) == [], "a case wrote into the invoking HOME"


def test_cases_get_distinct_directories(
    binary: Path, guard: harness.NetworkGuard, case_path: str
) -> None:
    case = make_case(command='echo "$T"')
    first = harness.run_case(case, binary, guard, case_path).stdout
    second = harness.run_case(case, binary, guard, case_path).stdout
    assert first != second


def test_owned_processes_are_reaped_after_the_case(
    binary: Path, guard: harness.NetworkGuard, case_path: str
) -> None:
    case = make_case(command="sleep 300 </dev/null >/dev/null 2>&1 & echo $!")
    result = harness.run_case(case, binary, guard, case_path)
    pid = int(result.stdout.strip().splitlines()[-1])
    assert result.leaked_processes
    assert any("left owned processes" in failure for failure in result.failures)
    assert_gone(pid)


def test_timeout_kills_the_whole_process_tree(
    binary: Path, guard: harness.NetworkGuard, case_path: str
) -> None:
    command = 'sleep 300 </dev/null >"$T/ws/child.out" 2>&1 & echo $! >"$T/ws/pid"; cat "$T/ws/pid"; sleep 300'
    case = make_case(command=command, timeout_sec=2, exit_code="nonzero")
    result = harness.run_case(case, binary, guard, case_path)
    assert result.timed_out
    assert result.exit_code is None
    assert any("timeout" in failure for failure in result.failures)
    assert not result.passed
    assert_gone(int(result.stdout.split()[0]))


def test_the_harness_never_signals_a_foreign_process_group() -> None:
    for pgid in (0, 1, os.getpgrp()):
        with pytest.raises(harness.HarnessError):
            harness.reap_group(pgid)


# ------------------------------------------------------------- network guard


def test_external_connections_are_refused_at_run_time(
    guard: harness.NetworkGuard,
) -> None:
    outcome = harness.require_denied(guard)
    denied = {errno.errorcode[number] for number in harness.GUARD_DENIED_ERRNOS}
    assert outcome["tcp"] in denied
    assert outcome["udp"] in denied


def test_cases_run_inside_the_verified_guard(
    binary: Path, guard: harness.NetworkGuard, case_path: str
) -> None:
    probe = shlex.join(
        [
            sys.executable,
            "-I",
            "-c",
            harness.PROBE_SOURCE,
            harness.PROBE_HOST,
            str(harness.PROBE_PORT),
        ]
    )
    result = harness.run_case(make_case(command=probe), binary, guard, case_path)
    assert result.passed, result.failures
    outcome = json.loads(result.stdout.strip().splitlines()[-1])
    denied = {errno.errorcode[number] for number in harness.GUARD_DENIED_ERRNOS}
    assert outcome["tcp"] in denied, outcome
    assert outcome["udp"] in denied, outcome


def test_a_guard_that_lets_connections_through_is_rejected() -> None:
    listener = socket.socket()
    listener.bind(("127.0.0.1", 0))
    listener.listen(1)
    try:
        noop = harness.NetworkGuard("noop", ())
        with pytest.raises(harness.HarnessError, match="did not refuse"):
            harness.require_denied(noop, "127.0.0.1", listener.getsockname()[1])
    finally:
        listener.close()


def test_no_guard_means_no_run(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(harness, "_GUARD_CACHE", [])
    monkeypatch.setattr(harness, "_guard_candidates", lambda: [])
    with pytest.raises(harness.HarnessError, match="no working network guard"):
        harness.detect_guard()
