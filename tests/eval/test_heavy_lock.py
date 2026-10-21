import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from contextlib import contextmanager
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.heavy_lock import (
    DEFAULT_HEAVY_LOCK_DIR,
    HEAVY_LOCK_DIR_ENV,
    HEAVY_LOCK_HELD_ENV,
    HEAVY_LOCK_POLL_SECONDS_ENV,
    HeavyLockTimeout,
    heavy_lock,
    owner_state,
    resolve_lock_dir,
    resolve_poll_seconds,
    wait_record_path,
)
from eval_lib.run_summary import read_summary


def load_script(name: str, relative: str):
    module_path = ROOT / relative
    spec = importlib.util.spec_from_file_location(name, module_path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


@contextmanager
def heavy_env(lock_dir: Path):
    keys = (HEAVY_LOCK_DIR_ENV, HEAVY_LOCK_HELD_ENV, HEAVY_LOCK_POLL_SECONDS_ENV)
    previous = {key: os.environ.get(key) for key in keys}
    os.environ[HEAVY_LOCK_DIR_ENV] = str(lock_dir)
    os.environ.pop(HEAVY_LOCK_HELD_ENV, None)
    os.environ[HEAVY_LOCK_POLL_SECONDS_ENV] = "0.01"
    try:
        yield
    finally:
        for key, value in previous.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value


def read_records(run_dir: Path) -> list[dict]:
    path = wait_record_path(run_dir)
    if not path.exists():
        return []
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


def dead_pid() -> int | None:
    proc = subprocess.Popen([sys.executable, "-c", "pass"])
    proc.wait()
    try:
        os.kill(proc.pid, 0)
    except ProcessLookupError:
        return proc.pid
    except OSError:
        return None
    return None


class HeavyLockCoreTest(unittest.TestCase):
    def test_acquire_records_owner_shape_and_releases(self):
        with tempfile.TemporaryDirectory() as td:
            lock_dir = Path(td) / "slot.d"
            run_dir = Path(td) / "run"
            env: dict[str, str] = {}
            with heavy_lock(
                run_dir=run_dir,
                invocation="unit",
                lock_dir=lock_dir,
                poll_seconds=0.01,
                timeout_seconds=5,
                env=env,
            ) as lock:
                self.assertTrue(lock.held)
                self.assertFalse(lock.record.nested)
                self.assertTrue(lock_dir.is_dir())
                self.assertEqual((lock_dir / "pid").read_text().strip(), str(os.getpid()))
                self.assertIn("unit", (lock_dir / "cmd").read_text())
                self.assertEqual(env[HEAVY_LOCK_HELD_ENV], str(os.getpid()))
            self.assertFalse(lock_dir.exists())
            self.assertNotIn(HEAVY_LOCK_HELD_ENV, env)
            records = read_records(run_dir)
            self.assertEqual(len(records), 1)
            self.assertTrue(records[0]["held"])
            self.assertFalse(records[0]["nested"])
            self.assertGreaterEqual(records[0]["wait_sec"], 0.0)
            self.assertEqual(records[0]["invocation"], "unit")

    def test_nested_holder_skips_acquire_and_release(self):
        with tempfile.TemporaryDirectory() as td:
            lock_dir = Path(td) / "slot.d"
            run_dir = Path(td) / "run"
            env = {HEAVY_LOCK_HELD_ENV: "4242"}
            with heavy_lock(
                run_dir=run_dir,
                invocation="nested",
                lock_dir=lock_dir,
                env=env,
            ) as lock:
                self.assertFalse(lock.held)
                self.assertTrue(lock.record.nested)
                self.assertEqual(lock.record.wait_sec, 0.0)
                self.assertFalse(lock_dir.exists())
            self.assertFalse(lock_dir.exists())
            self.assertEqual(env[HEAVY_LOCK_HELD_ENV], "4242")
            records = read_records(run_dir)
            self.assertEqual(len(records), 1)
            self.assertFalse(records[0]["held"])
            self.assertTrue(records[0]["nested"])

    def test_second_holder_waits_for_first_to_release(self):
        with tempfile.TemporaryDirectory() as td:
            lock_dir = Path(td) / "slot.d"
            order: list[str] = []
            waits: dict[str, float] = {}
            first_entered = threading.Event()

            def worker(name: str, hold_sec: float):
                with heavy_lock(
                    invocation=name,
                    lock_dir=lock_dir,
                    poll_seconds=0.01,
                    timeout_seconds=10,
                    env={},
                ) as lock:
                    waits[name] = lock.record.wait_sec
                    order.append(name)
                    if name == "first":
                        first_entered.set()
                    time.sleep(hold_sec)

            first = threading.Thread(target=worker, args=("first", 0.3))
            first.start()
            self.assertTrue(first_entered.wait(timeout=5))
            second = threading.Thread(target=worker, args=("second", 0.0))
            second.start()
            first.join(timeout=10)
            second.join(timeout=10)
            self.assertEqual(order, ["first", "second"])
            self.assertGreaterEqual(waits["second"], 0.15)
            self.assertFalse(lock_dir.exists())

    def test_child_process_inherits_the_held_slot(self):
        with tempfile.TemporaryDirectory() as td:
            lock_dir = Path(td) / "slot.d"
            with heavy_env(lock_dir):
                with heavy_lock(invocation="parent"):
                    child = subprocess.run(
                        [
                            sys.executable,
                            "-c",
                            "import os; print(os.environ.get('HEAVY_LOCK_HELD', ''))",
                        ],
                        text=True,
                        capture_output=True,
                    )
                    self.assertEqual(child.returncode, 0, child.stderr)
                    self.assertEqual(child.stdout.strip(), str(os.getpid()))

    def test_child_nested_call_does_not_touch_the_slot(self):
        with tempfile.TemporaryDirectory() as td:
            lock_dir = Path(td) / "slot.d"
            code = (
                "import sys; sys.path.insert(0, sys.argv[1]);"
                "from eval_lib.heavy_lock import heavy_lock;"
                "ctx = heavy_lock(lock_dir=sys.argv[2], invocation='child');"
                "lock = ctx.__enter__();"
                "print(lock.record.nested, lock.held);"
                "ctx.__exit__(None, None, None)"
            )
            proc = subprocess.run(
                [sys.executable, "-c", code, str(ROOT / "scripts"), str(lock_dir)],
                env={**os.environ, HEAVY_LOCK_HELD_ENV: "777"},
                text=True,
                capture_output=True,
            )
            self.assertEqual(proc.returncode, 0, proc.stderr)
            self.assertEqual(proc.stdout.strip(), "True False")
            self.assertFalse(lock_dir.exists())

    def test_stale_owner_is_reclaimed(self):
        pid = dead_pid()
        if pid is None:
            self.skipTest("could not obtain a dead pid")
        with tempfile.TemporaryDirectory() as td:
            lock_dir = Path(td) / "slot.d"
            lock_dir.mkdir(parents=True)
            (lock_dir / "pid").write_text(f"{pid}\n", encoding="utf-8")
            (lock_dir / "cmd").write_text("dead holder\n", encoding="utf-8")
            with heavy_lock(
                invocation="reclaim",
                lock_dir=lock_dir,
                poll_seconds=0.01,
                timeout_seconds=5,
                env={},
            ) as lock:
                self.assertTrue(lock.held)
                self.assertEqual((lock_dir / "pid").read_text().strip(), str(os.getpid()))
            self.assertFalse(lock_dir.exists())

    def test_alive_owner_is_protected_until_timeout(self):
        with tempfile.TemporaryDirectory() as td:
            lock_dir = Path(td) / "slot.d"
            lock_dir.mkdir(parents=True)
            (lock_dir / "pid").write_text(f"{os.getpid()}\n", encoding="utf-8")
            with self.assertRaises(HeavyLockTimeout):
                with heavy_lock(
                    lock_dir=lock_dir,
                    poll_seconds=0.01,
                    timeout_seconds=0.1,
                    env={},
                ):
                    self.fail("must not enter while the slot is held")
            self.assertTrue(lock_dir.exists())
            self.assertEqual((lock_dir / "pid").read_text().strip(), str(os.getpid()))

    def test_missing_pid_before_recording_is_protected(self):
        with tempfile.TemporaryDirectory() as td:
            lock_dir = Path(td) / "slot.d"
            lock_dir.mkdir(parents=True)
            with self.assertRaises(HeavyLockTimeout):
                with heavy_lock(lock_dir=lock_dir, poll_seconds=0.01, timeout_seconds=0.1, env={}):
                    self.fail("must not enter on an unrecorded owner")
            self.assertTrue(lock_dir.exists())

    def test_corrupt_pid_is_protected(self):
        with tempfile.TemporaryDirectory() as td:
            lock_dir = Path(td) / "slot.d"
            lock_dir.mkdir(parents=True)
            (lock_dir / "pid").write_text("not-a-pid\n", encoding="utf-8")
            state, pid = owner_state(lock_dir)
            self.assertEqual(state, "unknown")
            self.assertIsNone(pid)
            with self.assertRaises(HeavyLockTimeout):
                with heavy_lock(lock_dir=lock_dir, poll_seconds=0.01, timeout_seconds=0.1, env={}):
                    self.fail("must not enter on a corrupt owner")
            self.assertTrue(lock_dir.exists())

    def test_non_positive_pid_is_protected(self):
        with tempfile.TemporaryDirectory() as td:
            lock_dir = Path(td) / "slot.d"
            lock_dir.mkdir(parents=True)
            (lock_dir / "pid").write_text("0\n", encoding="utf-8")
            with self.assertRaises(HeavyLockTimeout):
                with heavy_lock(lock_dir=lock_dir, poll_seconds=0.01, timeout_seconds=0.1, env={}):
                    self.fail("must not enter on an invalid owner")
            self.assertTrue(lock_dir.exists())

    def test_release_leaves_a_slot_it_no_longer_owns(self):
        with tempfile.TemporaryDirectory() as td:
            lock_dir = Path(td) / "slot.d"
            env: dict[str, str] = {}
            with heavy_lock(lock_dir=lock_dir, env=env) as lock:
                (lock_dir / "pid").write_text("1\n", encoding="utf-8")
                lock.release()
                self.assertTrue(lock_dir.exists())
            self.assertTrue(lock_dir.exists())
            self.assertNotIn(HEAVY_LOCK_HELD_ENV, env)

    def test_exception_releases_the_slot(self):
        with tempfile.TemporaryDirectory() as td:
            lock_dir = Path(td) / "slot.d"
            env: dict[str, str] = {}
            with self.assertRaises(ValueError):
                with heavy_lock(lock_dir=lock_dir, env=env):
                    raise ValueError("boom")
            self.assertFalse(lock_dir.exists())
            self.assertNotIn(HEAVY_LOCK_HELD_ENV, env)

    def test_keyboard_interrupt_releases_the_slot(self):
        with tempfile.TemporaryDirectory() as td:
            lock_dir = Path(td) / "slot.d"
            env: dict[str, str] = {}
            with self.assertRaises(KeyboardInterrupt):
                with heavy_lock(lock_dir=lock_dir, env=env):
                    raise KeyboardInterrupt
            self.assertFalse(lock_dir.exists())
            self.assertNotIn(HEAVY_LOCK_HELD_ENV, env)

    def test_wait_records_append_and_do_not_overwrite_on_resume(self):
        with tempfile.TemporaryDirectory() as td:
            lock_dir = Path(td) / "slot.d"
            run_dir = Path(td) / "run"
            run_dir.mkdir(parents=True)
            wait_record_path(run_dir).write_text(
                json.dumps({"event": "heavy_lock", "invocation": "previous"}) + "\n",
                encoding="utf-8",
            )
            with heavy_lock(run_dir=run_dir, invocation="one", lock_dir=lock_dir, env={}):
                pass
            with heavy_lock(run_dir=run_dir, invocation="two", lock_dir=lock_dir, env={}):
                pass
            records = read_records(run_dir)
            self.assertEqual(
                [record["invocation"] for record in records],
                ["previous", "one", "two"],
            )

    def test_many_waiters_keep_single_holder_and_all_enter(self):
        with tempfile.TemporaryDirectory() as td:
            lock_dir = Path(td) / "slot.d"
            guard = threading.Lock()
            active = 0
            max_active = 0
            entered = 0

            def worker():
                nonlocal active, max_active, entered
                with heavy_lock(
                    invocation="w",
                    lock_dir=lock_dir,
                    poll_seconds=0.005,
                    timeout_seconds=15,
                    env={},
                ):
                    with guard:
                        active += 1
                        max_active = max(max_active, active)
                        entered += 1
                    time.sleep(0.02)
                    with guard:
                        active -= 1

            threads = [threading.Thread(target=worker) for _ in range(4)]
            for thread in threads:
                thread.start()
            for thread in threads:
                thread.join(timeout=20)
            self.assertEqual(entered, 4)
            self.assertEqual(max_active, 1)
            self.assertFalse(lock_dir.exists())

    def test_dead_holder_race_yields_one_holder(self):
        pid = dead_pid()
        if pid is None:
            self.skipTest("could not obtain a dead pid")
        with tempfile.TemporaryDirectory() as td:
            lock_dir = Path(td) / "slot.d"
            lock_dir.mkdir(parents=True)
            (lock_dir / "pid").write_text(f"{pid}\n", encoding="utf-8")
            guard = threading.Lock()
            active = 0
            max_active = 0
            entered = 0

            def worker():
                nonlocal active, max_active, entered
                with heavy_lock(
                    invocation="r",
                    lock_dir=lock_dir,
                    poll_seconds=0.005,
                    timeout_seconds=15,
                    env={},
                ):
                    with guard:
                        active += 1
                        max_active = max(max_active, active)
                        entered += 1
                    time.sleep(0.02)
                    with guard:
                        active -= 1

            threads = [threading.Thread(target=worker) for _ in range(3)]
            for thread in threads:
                thread.start()
            for thread in threads:
                thread.join(timeout=20)
            self.assertEqual(entered, 3)
            self.assertEqual(max_active, 1)
            self.assertFalse(lock_dir.exists())

    def test_default_slot_matches_the_wrapper(self):
        self.assertEqual(DEFAULT_HEAVY_LOCK_DIR, "/Volumes/SSD_NX/tmp/heavy-lock/slot.d")
        self.assertEqual(resolve_lock_dir({}), Path(DEFAULT_HEAVY_LOCK_DIR))
        self.assertEqual(resolve_lock_dir({}, "/tmp/other.d"), Path("/tmp/other.d"))
        self.assertEqual(
            resolve_lock_dir({HEAVY_LOCK_DIR_ENV: "/tmp/env.d"}),
            Path("/tmp/env.d"),
        )

    def test_poll_seconds_env_is_honoured(self):
        self.assertEqual(resolve_poll_seconds({}), 2.0)
        self.assertEqual(resolve_poll_seconds({HEAVY_LOCK_POLL_SECONDS_ENV: "0.25"}), 0.25)
        self.assertEqual(resolve_poll_seconds({HEAVY_LOCK_POLL_SECONDS_ENV: "bad"}), 2.0)


class HeavyLockEntrypointTest(unittest.TestCase):
    def _assert_slot_was_released(self, lock_dir: Path, run_dir: Path):
        self.assertFalse(lock_dir.exists())
        records = read_records(run_dir)
        self.assertEqual(len(records), 1)
        self.assertTrue(records[0]["held"])
        self.assertFalse(records[0]["nested"])

    def test_goal_verify_live_entrypoint_takes_the_slot(self):
        module = load_script("cli_goal_verify_live", "scripts/eval-goal-verify-live.py")
        with tempfile.TemporaryDirectory() as td:
            lock_dir = Path(td) / "slot.d"
            run_dir = Path(td) / "run"
            observed = {}

            def fake_campaign(**kwargs):
                observed["held"] = os.environ.get(HEAVY_LOCK_HELD_ENV)
                observed["slot_exists"] = lock_dir.exists()
                return {"completed_pairs": 0, "target_pairs": 0}

            module.run_campaign = fake_campaign
            with heavy_env(lock_dir), mock.patch.object(
                sys, "argv", ["eval-goal-verify-live.py", "--run-dir", str(run_dir)]
            ):
                self.assertEqual(module.main(), 0)
            self.assertEqual(observed["held"], str(os.getpid()))
            self.assertTrue(observed["slot_exists"])
            self._assert_slot_was_released(lock_dir, run_dir)

    def test_goal_verify_live_acquires_for_every_generation(self):
        module = load_script("cli_goal_verify_live_gen", "scripts/eval-goal-verify-live.py")
        generations = {
            "v3": ("commandagent.goal_verify.phase6_preflight_contract.v3", "run_campaign_v3"),
            "v4": ("commandagent.goal_verify.phase6_preflight_contract.v4", "run_campaign_v4"),
        }
        for name, (schema_version, runner_name) in generations.items():
            with self.subTest(generation=name), tempfile.TemporaryDirectory() as td:
                lock_dir = Path(td) / "slot.d"
                run_dir = Path(td) / "run"
                contract = Path(td) / "contract.json"
                contract.write_text(
                    json.dumps({"schema_version": schema_version}), encoding="utf-8"
                )
                called = {}

                def fake_campaign(**kwargs):
                    called["held"] = os.environ.get(HEAVY_LOCK_HELD_ENV)
                    called["slot_exists"] = lock_dir.exists()
                    return {"completed_pairs": 0, "target_pairs": 0}

                setattr(module, runner_name, fake_campaign)
                argv = [
                    "eval-goal-verify-live.py",
                    "--contract",
                    str(contract),
                    "--run-dir",
                    str(run_dir),
                    "--execution-root",
                    str(Path(td) / "exec"),
                ]
                with heavy_env(lock_dir), mock.patch.object(sys, "argv", argv):
                    self.assertEqual(module.main(), 0)
                self.assertEqual(called["held"], str(os.getpid()))
                self.assertTrue(called["slot_exists"])
                self._assert_slot_was_released(lock_dir, run_dir)

    def test_goal_verify_blind_entrypoint_takes_the_slot(self):
        module = load_script("cli_goal_verify_blind", "scripts/eval-goal-verify-blind.py")
        with tempfile.TemporaryDirectory() as td:
            lock_dir = Path(td) / "slot.d"
            run_dir = Path(td) / "run"
            observed = {}

            def fake_review(**kwargs):
                observed["held"] = os.environ.get(HEAVY_LOCK_HELD_ENV)
                observed["slot_exists"] = lock_dir.exists()
                return {"reviewed_pairs": 0, "expected_pairs": 0, "complete": True}

            module.run_blind_review = fake_review
            argv = [
                "eval-goal-verify-blind.py",
                "--baseline",
                str(Path(td) / "baseline"),
                "--candidate-draft",
                str(Path(td) / "draft"),
                "--run-dir",
                str(run_dir),
            ]
            with heavy_env(lock_dir), mock.patch.object(sys, "argv", argv):
                self.assertEqual(module.main(), 0)
            self.assertEqual(observed["held"], str(os.getpid()))
            self.assertTrue(observed["slot_exists"])
            self._assert_slot_was_released(lock_dir, run_dir)

    def test_goal_verify_recovery_entrypoint_takes_the_slot(self):
        module = load_script("cli_goal_verify_recovery", "scripts/eval-goal-verify-recovery-v4.py")
        with tempfile.TemporaryDirectory() as td:
            lock_dir = Path(td) / "slot.d"
            run_dir = Path(td) / "run"
            observed = {}

            def fake_recovery(**kwargs):
                observed["held"] = os.environ.get(HEAVY_LOCK_HELD_ENV)
                observed["slot_exists"] = lock_dir.exists()
                return "recovery-summary"

            module.run_recovery_smoke = fake_recovery
            argv = [
                "eval-goal-verify-recovery-v4.py",
                "--contract",
                str(Path(td) / "contract.json"),
                "--run-dir",
                str(run_dir),
                "--execution-root",
                str(Path(td) / "exec"),
                "--commandagent-bin",
                str(Path(td) / "commandagent"),
            ]
            with heavy_env(lock_dir), mock.patch.object(sys, "argv", argv):
                self.assertEqual(module.main(), 0)
            self.assertEqual(observed["held"], str(os.getpid()))
            self.assertTrue(observed["slot_exists"])
            self._assert_slot_was_released(lock_dir, run_dir)

    def test_eval_run_matrix_takes_the_slot_and_records_wait_in_extras(self):
        module = load_script("cli_eval_run", "scripts/eval-run.py")
        with tempfile.TemporaryDirectory() as td:
            lock_dir = Path(td) / "slot.d"
            run_root = Path(td) / "run"
            observed = {}
            original_run_prepared = module.run_prepared

            def fake_run_prepared(item, timeout_sec, semaphores, total):
                index, spec, _, run_dir, workdir = item
                observed["held"] = os.environ.get(HEAVY_LOCK_HELD_ENV)
                observed["slot_exists"] = lock_dir.exists()
                return original_run_prepared(
                    (index, spec, ["true"], run_dir, workdir),
                    timeout_sec,
                    semaphores,
                    total,
                )

            module.run_prepared = fake_run_prepared
            module.write_trace_artifacts = lambda *a, **k: {
                "report_path": "report",
                "manifest_path": "manifest",
            }
            argv = [
                "eval-run.py",
                "--suite",
                "eval/suites/mvp-smoke.yaml",
                "--model-profile",
                "openai-only",
                "--modes",
                "minimal-loop",
                "--runs",
                "1",
                "--scenario",
                "docs-heading-update-small",
                "--run-root",
                str(run_root),
            ]
            with heavy_env(lock_dir), mock.patch.object(sys, "argv", argv):
                self.assertIn(module.main(), (0, 1))
            self.assertEqual(observed["held"], str(os.getpid()))
            self.assertTrue(observed["slot_exists"])
            self._assert_slot_was_released(lock_dir, run_root)
            rows = read_summary(run_root / "summary.eval.tsv")
            extras = json.loads(rows[0]["extras_json"])
            self.assertIn("heavy_lock_wait_sec", extras)
            self.assertIn("heavy_lock_nested", extras)
            self.assertFalse(extras["heavy_lock_nested"])

    def test_eval_run_dry_run_does_not_take_the_slot(self):
        module = load_script("cli_eval_run_dry", "scripts/eval-run.py")
        with tempfile.TemporaryDirectory() as td:
            lock_dir = Path(td) / "slot.d"
            run_root = Path(td) / "run"
            argv = [
                "eval-run.py",
                "--suite",
                "eval/suites/mvp-smoke.yaml",
                "--model-profile",
                "openai-only",
                "--modes",
                "minimal-loop",
                "--runs",
                "1",
                "--run-root",
                str(run_root),
                "--dry-run",
            ]
            with heavy_env(lock_dir), mock.patch.object(sys, "argv", argv):
                self.assertEqual(module.main(), 0)
            self.assertFalse(lock_dir.exists())
            self.assertEqual(read_records(run_root), [])

    def test_eval_preflight_live_provider_smoke_takes_the_slot(self):
        module = load_script("cli_eval_preflight", "scripts/eval-preflight.py")
        with tempfile.TemporaryDirectory() as td:
            lock_dir = Path(td) / "slot.d"
            run_root = Path(td) / "run"
            observed = {}

            def fake_smoke(profile, api_key):
                observed["held"] = os.environ.get(HEAVY_LOCK_HELD_ENV)
                observed["slot_exists"] = lock_dir.exists()
                return {"model": {"ok": True}}

            module.smoke_openai_models = fake_smoke
            argv = [
                "eval-preflight.py",
                "--suite",
                "eval/suites/mvp-smoke.yaml",
                "--model-profile",
                "openai-only",
                "--modes",
                "minimal-loop",
                "--runs",
                "1",
                "--offline-ok",
                "--binary",
                "python3",
                "--live-provider-smoke",
                "openai",
                "--run-root",
                str(run_root),
            ]
            with heavy_env(lock_dir), mock.patch.object(sys, "argv", argv):
                self.assertEqual(module.main(), 0)
            self.assertEqual(observed["held"], str(os.getpid()))
            self.assertTrue(observed["slot_exists"])
            self._assert_slot_was_released(lock_dir, run_root)
            payload = json.loads((run_root / "preflight.json").read_text(encoding="utf-8"))
            self.assertIn("openai", payload["live_provider_smoke"])

    def test_eval_preflight_without_smoke_does_not_take_the_slot(self):
        module = load_script("cli_eval_preflight_plain", "scripts/eval-preflight.py")
        with tempfile.TemporaryDirectory() as td:
            lock_dir = Path(td) / "slot.d"
            run_root = Path(td) / "run"
            argv = [
                "eval-preflight.py",
                "--suite",
                "eval/suites/mvp-smoke.yaml",
                "--model-profile",
                "openai-only",
                "--modes",
                "minimal-loop",
                "--runs",
                "1",
                "--offline-ok",
                "--binary",
                "python3",
                "--run-root",
                str(run_root),
            ]
            with heavy_env(lock_dir), mock.patch.object(sys, "argv", argv):
                self.assertEqual(module.main(), 0)
            self.assertFalse(lock_dir.exists())
            self.assertEqual(read_records(run_root), [])


if __name__ == "__main__":
    unittest.main()
