"""Replay frozen original handlers and a correct control through identical oracles."""

import contextlib
import hashlib
import json
import selectors
import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from nextjs_domain_oracle import Oracle

CORPUS = ROOT / "tests/corpus/apps"


@contextlib.contextmanager
def replay(fixture, workspace, correct):
    (workspace / ".issue442-oracle-scratch").touch()
    package = ROOT / "tests/nextjs_domain/node_modules/typescript"
    compiler = package / "lib/typescript.js"
    assert compiler.is_file(), "run npm ci --ignore-scripts --include=dev --prefix tests/nextjs_domain"
    assert json.loads((package / "package.json").read_text())["version"] == "5.9.3"
    with (workspace / "server.log").open("w+") as errors:
        process = subprocess.Popen(
            ["node", "--disable-warning=ExperimentalWarning", str(ROOT / "tests/nextjs_domain/replay.mjs"),
             str(fixture), "control" if correct else "original", str(compiler)],
            cwd=workspace, stdout=subprocess.PIPE, stderr=errors, text=True,
        )
        try:
            with selectors.DefaultSelector() as selector:
                selector.register(process.stdout, selectors.EVENT_READ)
                assert selector.select(15), "handler replay startup timed out"
            line = process.stdout.readline()
            errors.flush()
            errors.seek(0)
            assert line, errors.read()
            port = json.loads(line)["port"]
            yield "http://127.0.0.1:" + str(port)
        finally:
            process.terminate()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
            process.stdout.close()


def assert_provenance(fixture):
    provenance = json.loads((fixture / "provenance.json").read_text())
    assert provenance["campaign"] == "20260907-1317-standard10"
    actual = {str(p.relative_to(fixture)): hashlib.sha256(p.read_bytes()).hexdigest()
              for p in (fixture / "src").rglob("*") if p.is_file()}
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
