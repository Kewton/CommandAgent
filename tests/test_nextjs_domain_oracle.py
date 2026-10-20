"""Replay frozen original handlers and a correct control through identical oracles."""

import concurrent.futures
import contextlib
import hashlib
import json
import socket
import sys
import urllib.request
from collections import namedtuple
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
sys.path.insert(0, str(ROOT / "tests/nextjs_domain"))
from nextjs_domain_oracle import Oracle, RacePreconditionError
from ready_wait import ReadyError, parse_ready_line, ready_child, wait_ready_port

CORPUS = ROOT / "tests/corpus/apps"


Replay = namedtuple("Replay", "url port pid compiler")
LOG_PREFIX = "[replay] "


def replay_events(workspace):
    """Structured events the replay wrote to its own stderr log."""
    events = []
    for line in (workspace / "server.log").read_text().splitlines():
        if line.startswith(LOG_PREFIX):
            events.append(json.loads(line[len(LOG_PREFIX) :]))
    return events


def replay_log_tail(workspace, limit=6000):
    return (workspace / "server.log").read_text()[-limit:]


@contextlib.contextmanager
def replay(fixture, workspace, correct):
    (workspace / ".issue442-oracle-scratch").touch()
    package = ROOT / "tests/nextjs_domain/node_modules/typescript"
    compiler = package / "lib/typescript.js"
    assert compiler.is_file(), (
        "run npm ci --ignore-scripts --include=dev --prefix tests/nextjs_domain"
    )
    assert json.loads((package / "package.json").read_text())["version"] == "5.9.3"
    argv = [
        "node",
        "--disable-warning=ExperimentalWarning",
        str(ROOT / "tests/nextjs_domain/replay.mjs"),
        str(fixture),
        "control" if correct else "original",
        str(compiler),
    ]
    with (workspace / "server.log").open("w+") as errors:
        with ready_child(argv, workspace, errors) as (process, port):
            yield Replay("http://127.0.0.1:" + str(port), port, process.pid, compiler)


def wire_race(oracle, required):
    oracle.arm_race = lambda: oracle.request("POST", "/__replay/race", {})
    oracle.race_report = lambda: oracle.request("GET", "/__replay/barrier")[1]
    oracle.race_required = required
    return oracle


def assert_provenance(fixture):
    provenance = json.loads((fixture / "provenance.json").read_text())
    assert provenance["campaign"] == "20260907-1317-standard10"
    actual = {
        str(p.relative_to(fixture)): hashlib.sha256(p.read_bytes()).hexdigest()
        for p in (fixture / "src").rglob("*")
        if p.is_file()
    }
    assert actual == provenance["sha256"]


@pytest.mark.parametrize("run", ["s1", "e1", "i2", "s3"])
@pytest.mark.parametrize("correct", [False, True], ids=["original", "control"])
def test_frozen_domain_oracles(run, correct, tmp_path):
    fixture = CORPUS / ("nextjs-domain-" + run)
    assert_provenance(fixture)
    adapter = json.loads((fixture / "adapter.json").read_text())
    expected = set() if correct else set(adapter["expected_failures"])
    with replay(fixture, tmp_path, correct) as served:
        oracle = wire_race(
            Oracle(served.url, tmp_path, adapter),
            required=not correct and "concurrent-create" in expected,
        )
        before = oracle.snapshot()
        try:
            results = oracle.run()
        except RacePreconditionError as error:
            raise AssertionError(
                f"{error}\nreplay log:\n{replay_log_tail(tmp_path)}"
            ) from error
        assert oracle.snapshot() == before
        report = oracle.race_report()
    failed = {result["case"] for result in results if not result["passed"]}
    detail = (
        json.dumps(results, ensure_ascii=False, indent=2)
        + "\nreplay log:\n"
        + replay_log_tail(tmp_path)
    )
    assert failed == expected, detail
    if correct:
        # The control serializes whole transactions: the barrier must not have
        # been able to overlap two requests, or the control is not serial.
        assert report["state"] == "unmet" and "timeout" in report["reason"], report
    assert len(results) >= 11
    assert_provenance(fixture)


def test_s3_candidate_provenance():
    assert_provenance(CORPUS / "nextjs-domain-s3-candidate")


def test_oracle_refuses_unmarked_workspace(tmp_path):
    with pytest.raises(ValueError, match="disposable"):
        Oracle("http://127.0.0.1:1", tmp_path, {})


def test_unavailable_server_is_not_a_success(tmp_path):
    (tmp_path / ".issue442-oracle-scratch").touch()
    adapter = json.loads((CORPUS / "nextjs-domain-s1/adapter.json").read_text())
    with pytest.raises(OSError):
        Oracle("http://127.0.0.1:1", tmp_path, adapter).run()


class FakeClock:
    def __init__(self):
        self.now = 0.0

    def __call__(self):
        return self.now

    def reader(self, chunks):
        """Each read consumes the chunk list and advances time by the wait."""
        items = list(chunks)

        def read(seconds):
            self.now += seconds
            return items.pop(0) if items else None

        return read


def wait(chunks, exit_codes=(), timeout=1.0):
    clock = FakeClock()
    codes = list(exit_codes)
    return wait_ready_port(
        clock.reader(chunks), lambda: codes.pop(0) if codes else None, clock, timeout
    )


def test_ready_wait_accepts_split_line():
    assert wait([b'{"po', b'rt": 4321}', b"\n"]) == 4321


@pytest.mark.parametrize(
    "chunks, exits, kind",
    [
        ([b'{"port": 12'], (), "partial-line"),
        ([b'{"port": 12', b""], (), "partial-eof"),
        ([b""], (), "eof"),
        ([None, None], (3,), "early-exit"),
        ([b"not json\n"], (), "invalid-json"),
        ([b'{"other": 1}\n'], (), "invalid-json"),
        ([b'{"port": 0}\n'], (), "invalid-port"),
        ([b'{"port": "80"}\n'], (), "invalid-port"),
        ([b'{"port": true}\n'], (), "invalid-port"),
        ([], (), "timeout"),
    ],
)
def test_ready_wait_rejects(chunks, exits, kind):
    with pytest.raises(ReadyError) as raised:
        wait(chunks, exits)
    assert raised.value.kind == kind


def test_parse_ready_line_returns_port():
    assert parse_ready_line('{"port": 8080}') == 8080


CHILDREN = {
    "unterminated": "import sys,time; sys.stdout.write('{\"port\": 12'); sys.stdout.flush(); time.sleep(60)",
    "silent": "import time; time.sleep(60)",
    "early-exit": "import sys; sys.stderr.write('boom'); sys.exit(3)",
    "invalid-json": "import sys,time; print('nope', flush=True); time.sleep(60)",
}


@pytest.mark.parametrize(
    "name, kind",
    [
        ("unterminated", "partial-line"),
        ("silent", "timeout"),
        ("early-exit", "eof"),
        ("invalid-json", "invalid-json"),
    ],
)
def test_ready_child_failures_reap_the_child(name, kind, tmp_path):
    with (tmp_path / "err.log").open("w+") as errors:
        with pytest.raises(ReadyError) as raised:
            with ready_child(
                [sys.executable, "-c", CHILDREN[name]],
                tmp_path,
                errors,
                startup_sec=0.7,
            ):
                raise AssertionError("must not become ready")
    assert raised.value.kind == kind
    assert raised.value.process.poll() is not None
    assert raised.value.process.stdout.closed
    if name == "early-exit":
        assert "boom" in str(raised.value)


# --- replay diagnostics and race-barrier controls (Issue #780) -------------
# Not diagnosed: the single failure seen once under concurrent verification was
# never reproduced and its exact failing case set was not captured, so nothing
# here claims a cause. These tests add observations and fix what is measured.

UNSAFE_ROUTE = """
import fs from 'node:fs/promises';
const FILE = 'data/staff.json';
async function load() {
  try { return JSON.parse(await fs.readFile(FILE, 'utf8')); } catch (e) { return []; }
}
let chain = Promise.resolve();
async function create(row, serial) {
  await load();                       // seed-style read that does not feed the write
  const rows = await load();          // the read-modify-write read
  rows.push(row);
  await fs.mkdir('data', { recursive: true });
  await fs.writeFile(FILE, JSON.stringify(rows));
  return row;
}
export async function POST(request) {
  const row = await request.json();
  __SERIAL__
}
export async function GET() { return Response.json(await load()); }
"""
UNSAFE = UNSAFE_ROUTE.replace(
    "__SERIAL__", "return Response.json(await create(row), { status: 201 });"
)
SERIAL = UNSAFE_ROUTE.replace(
    "__SERIAL__",
    "const run = chain.then(() => create(row)); chain = run.catch(() => {});"
    " return Response.json(await run, { status: 201 });",
)


def synthetic_fixture(root, route):
    """A disposable fixture with a controlled multi-read create route."""
    (root / "src/app/api/staff").mkdir(parents=True)
    (root / "src/app/api/staff/route.ts").write_text(route)
    (root / "adapter.json").write_text(
        json.dumps({"parent": "staff", "child": "shifts"})
    )
    return root


def post_json(url, body):
    request = urllib.request.Request(
        url,
        data=json.dumps(body).encode(),
        method="POST",
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(request, timeout=10) as response:
        return response.status, json.loads(response.read())


def get_json(url):
    with urllib.request.urlopen(url, timeout=10) as response:
        return json.loads(response.read())


def run_controlled_race(tmp_path, route):
    fixture = synthetic_fixture(tmp_path / "fixture", route)
    workspace = tmp_path / "work"
    workspace.mkdir()
    with replay(fixture, workspace, False) as served:
        post_json(served.url + "/__replay/race", {})
        with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
            replies = list(
                pool.map(
                    lambda i: post_json(served.url + "/api/staff", {"id": str(i)}),
                    range(2),
                )
            )
        rows = json.loads((workspace / "data/staff.json").read_text())
        report = get_json(served.url + "/__replay/barrier")
    return replies, rows, report, workspace


def test_unsafe_multi_read_writer_loses_an_update(tmp_path):
    replies, rows, report, workspace = run_controlled_race(tmp_path, UNSAFE)
    assert [status for status, _ in replies] == [201, 201]
    assert len(rows) == 1, ("lost update must be detected", rows)
    # The barrier aligned BOTH reads of the create path (seed then RMW), not
    # just the first two reads it saw.
    assert report["state"] == "measured", report
    assert [level["ordinal"] for level in report["levels"]] == [1, 2], report
    assert {level["released_by"] for level in report["levels"]} == {"rendezvous"}
    ordinals = [
        (e["request"], e["ordinal"])
        for e in replay_events(workspace)
        if e["event"] == "read"
    ]
    assert sorted(ordinals) == [(1, 1), (1, 2), (2, 1), (2, 2)], ordinals


def test_serial_multi_read_writer_keeps_both_rows(tmp_path):
    replies, rows, report, workspace = run_controlled_race(tmp_path, SERIAL)
    assert [status for status, _ in replies] == [201, 201]
    assert sorted(row["id"] for row in rows) == ["0", "1"], rows
    # A serial writer cannot rendezvous: that is reported, never "measured".
    assert (
        report["state"] == "unmet" and "timeout at read ordinal 1" in report["reason"]
    ), report


def test_oracle_fails_explicitly_when_the_barrier_is_unmet(tmp_path):
    # A correct serial writer cannot satisfy a race precondition; requiring it
    # is a readable failure, not an expected-failure case result.
    fixture = CORPUS / "nextjs-domain-s1"
    adapter = json.loads((fixture / "adapter.json").read_text())
    with replay(fixture, tmp_path, True) as served:
        oracle = wire_race(Oracle(served.url, tmp_path, adapter), required=True)
        with pytest.raises(RacePreconditionError, match="did not measure the race"):
            oracle.run()


def test_replay_log_records_identity_without_the_environment(tmp_path):
    fixture = CORPUS / "nextjs-domain-e1"
    with replay(fixture, tmp_path, False) as served:
        post_json(served.url + "/__replay/race", {})
        events = replay_events(tmp_path)
    start = next(e for e in events if e["event"] == "start")
    listen = next(e for e in events if e["event"] == "listen")
    assert start["pid"] == listen["pid"] == served.pid
    assert Path(start["cwd"]) == tmp_path.resolve()
    assert Path(start["fixture"]) == fixture.resolve()
    assert Path(start["compiler"]) == Path(served.compiler).resolve()
    assert listen["port"] == served.port
    assert any(e["event"] == "barrier-arm" for e in events)
    text = (tmp_path / "server.log").read_text()
    assert "PATH" not in text and "HOME" not in text and "env" not in start


def port_is_closed(port):
    with socket.socket() as probe:
        probe.settimeout(1)
        return probe.connect_ex(("127.0.0.1", port)) != 0


def test_two_independent_replays_run_concurrently_and_are_reaped(tmp_path):
    fixture = CORPUS / "nextjs-domain-e1"
    adapter = json.loads((fixture / "adapter.json").read_text())
    dirs = [tmp_path / "a", tmp_path / "b"]
    for directory in dirs:
        directory.mkdir()
    with contextlib.ExitStack() as stack:
        served = [stack.enter_context(replay(fixture, d, False)) for d in dirs]
        assert served[0].port != served[1].port
        assert served[0].pid != served[1].pid
        post_json(
            served[0].url + "/api/departments",
            {"name": "only-in-a", "monthlyBudget": 1},
        )
        assert (dirs[0] / "data/departments.json").is_file()
        assert not (dirs[1] / "data/departments.json").exists(), (
            "write leaked across replays"
        )

        def run(index):
            oracle = wire_race(
                Oracle(served[index].url, dirs[index], adapter), required=True
            )
            return {
                r["case"] for r in oracle.run() if not r["passed"]
            }, oracle.race_report()

        with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
            outcomes = list(pool.map(run, range(2)))
        for index, (failed, report) in enumerate(outcomes):
            assert failed == set(adapter["expected_failures"]), (index, failed)
            assert report["state"] == "measured", (index, report)
            start = next(e for e in replay_events(dirs[index]) if e["event"] == "start")
            assert Path(start["cwd"]) == dirs[index].resolve()
            assert start["pid"] == served[index].pid
    assert all(port_is_closed(s.port) for s in served), "a replay server was not reaped"
