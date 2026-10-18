"""Replay frozen original handlers and a correct control through identical oracles."""

import contextlib
import hashlib
import json
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
sys.path.insert(0, str(ROOT / "tests/nextjs_domain"))
from nextjs_domain_oracle import Oracle
from ready_wait import ReadyError, parse_ready_line, ready_child, wait_ready_port

CORPUS = ROOT / "tests/corpus/apps"


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
        with ready_child(argv, workspace, errors) as (_process, port):
            yield "http://127.0.0.1:" + str(port)


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
    with replay(fixture, tmp_path, correct) as url:
        oracle = Oracle(url, tmp_path, adapter)
        oracle.arm_race = lambda: oracle.request("POST", "/__replay/race", {})
        before = oracle.snapshot()
        results = oracle.run()
        assert oracle.snapshot() == before
    failed = {result["case"] for result in results if not result["passed"]}
    expected = set() if correct else set(adapter["expected_failures"])
    assert failed == expected, json.dumps(results, ensure_ascii=False, indent=2)
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
