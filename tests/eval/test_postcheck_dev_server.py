import shlex
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib import postcheck
from eval_lib.postcheck import load_postcheck_events, run_postcheck

SERVER = ROOT / "tests/eval/support/postcheck_ready_server.py"
READY_TIMEOUT_SEC = 10


class ReadyNotAnnounced(AssertionError):
    pass


def read_ready_port(proc, ready_file, timeout_sec):
    """Wait a finite time for the child's port; reap the child on failure."""
    deadline = time.monotonic() + timeout_sec
    while time.monotonic() < deadline:
        if ready_file.exists():
            text = ready_file.read_text(encoding="utf-8").strip()
            if text.isdigit() and int(text) > 0:
                return int(text)
        if proc.poll() is not None:
            break
        time.sleep(0.02)
    code = proc.poll()
    if code is None:
        proc.kill()
    proc.wait(timeout=5)
    stderr = Path(proc.stderr_path).read_text(encoding="utf-8")
    raise ReadyNotAnnounced(f"no ready port (exit {code}): {stderr}")


class ReadyPopenFactory:
    """Wraps only Popen: starts the real child, reads its port, then returns it."""

    def __init__(self, dev_by_ready_file, timeout_sec=READY_TIMEOUT_SEC):
        self.real_popen = subprocess.Popen
        self.dev_by_ready_file = dev_by_ready_file
        self.timeout_sec = timeout_sec
        self.procs = []
        self.ports = []
        self.lock = threading.Lock()

    def __call__(self, command, **kwargs):
        proc = self.real_popen(command, **kwargs)
        with self.lock:
            self.procs.append(proc)
        ready_file = Path(command[-1])
        proc.stderr_path = kwargs["stderr"].name
        port = read_ready_port(proc, ready_file, self.timeout_sec)
        dev = self.dev_by_ready_file[str(ready_file)]
        dev["port"] = port
        dev["readiness"]["url"] = f"http://127.0.0.1:{port}/"
        with self.lock:
            self.ports.append(port)
        return proc


def make_scenario(base, name, command_tail=None):
    workdir = base / name / "work"
    workdir.mkdir(parents=True)
    (workdir / "package.json").write_text("{}", encoding="utf-8")
    ready_file = base / name / "ready.port"
    tail = command_tail or [str(SERVER), str(ready_file)]
    dev = {
        "command": shlex.join([sys.executable, *tail[:-1], str(ready_file)]),
        "port": 0,
        "readiness": {"url": "", "expect_status": 200, "timeout_sec": 10},
        "shutdown": "signal",
    }
    scenario = {
        "expected_artifacts": ["package.json"],
        "postcheck": {"commands": [], "dev_server": dev},
    }
    return scenario, dev, workdir, base / name / "out", ready_file


class PostcheckDevServerTest(unittest.TestCase):
    def run_with_factory(self, jobs, factory):
        results = {}

        def run(name, scenario, workdir, out):
            results[name] = run_postcheck(scenario, workdir, out)

        with mock.patch.object(postcheck.subprocess, "Popen", factory):
            threads = [threading.Thread(target=run, args=job) for job in jobs]
            for thread in threads:
                thread.start()
            for thread in threads:
                thread.join(timeout=60)
                self.assertFalse(thread.is_alive())
        return results

    def test_dev_server_readiness_and_shutdown(self):
        with tempfile.TemporaryDirectory() as td:
            scenario, dev, workdir, out, ready = make_scenario(Path(td), "one")
            factory = ReadyPopenFactory({str(ready): dev})
            result = self.run_with_factory([("one", scenario, workdir, out)], factory)[
                "one"
            ]
            self.assertTrue(result["ok"])
            self.assertTrue((out / "dev-server.stdout.log").exists())
            (proc,) = factory.procs
            self.assertIsNotNone(proc.returncode)
            event = [
                e
                for e in load_postcheck_events(Path(result["events_path"]))
                if e["event"] == "dev_server"
            ][0]
            self.assertTrue(event["ready"])
            self.assertEqual(event["status"], 200)
            self.assertEqual(event["port"], factory.ports[0])
            self.assertGreater(event["port"], 0)
            self.assertEqual(event["shutdown"], "signal")

    def test_two_concurrent_servers_get_distinct_real_ports(self):
        with tempfile.TemporaryDirectory() as td:
            jobs, devs = [], {}
            for name in ("a", "b"):
                scenario, dev, workdir, out, ready = make_scenario(Path(td), name)
                devs[str(ready)] = dev
                jobs.append((name, scenario, workdir, out))
            factory = ReadyPopenFactory(devs)
            results = self.run_with_factory(jobs, factory)
            self.assertTrue(all(r["ok"] for r in results.values()))
            self.assertEqual(len(set(factory.ports)), 2)
            self.assertTrue(all(p.returncode is not None for p in factory.procs))

    def assert_negative_fails_and_reaps(self, tail, timeout_sec, expect_text):
        with tempfile.TemporaryDirectory() as td:
            scenario, dev, workdir, out, ready = make_scenario(Path(td), "neg", tail)
            factory = ReadyPopenFactory({str(ready): dev}, timeout_sec)
            with self.assertRaises(ReadyNotAnnounced) as raised:
                with mock.patch.object(postcheck.subprocess, "Popen", factory):
                    run_postcheck(scenario, workdir, out)
            self.assertIn(expect_text, str(raised.exception))
            (proc,) = factory.procs
            self.assertIsNotNone(proc.poll())

    def test_ready_never_announced_fails_and_reaps_child(self):
        self.assert_negative_fails_and_reaps(
            ["-c", "import time; time.sleep(60)", "ready"], 0.5, "exit None"
        )

    def test_child_early_exit_fails_with_stderr_and_reaps_child(self):
        tail = [
            "-c",
            "import sys; sys.stderr.write('startup boom'); sys.exit(3)",
            "ready",
        ]
        self.assert_negative_fails_and_reaps(tail, READY_TIMEOUT_SEC, "startup boom")


if __name__ == "__main__":
    unittest.main()
