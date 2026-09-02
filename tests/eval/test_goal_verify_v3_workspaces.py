"""Frozen real workspaces for the Phase 6 contract v3 preflight (decision v3-D4).

Every ``must_contain`` line of ``phase6-real-workspaces-v3.json`` is asserted
here for the python cells. The two Next.js cells are checked statically by
default; set ``GOAL_VERIFY_V3_NEXT=1`` (with ``node_modules`` provisioned) to
also build and serve them, and ``GOAL_VERIFY_V3_PLAYWRIGHT=1`` to drive the
counter in a headless browser.
"""

import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import unittest
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_executors_v3 import execute_registered
from eval_lib.goal_verify_workspaces_v3 import prepare_workspace_stage

FIXTURES = ROOT / "tests/fixtures/goal_verify_v3"
REGISTRY = ROOT / "eval/goal_verify/v0/phase6-real-workspaces-v3.json"
ADAPTERS = ROOT / "eval/goal_verify/v0/phase6-command-adapters-v3.json"
CAPABILITIES = ROOT / "eval/goal_verify/v0/phase6-execution-capabilities-v3.json"
PYTEST = [sys.executable, "-m", "pytest", "-q", "-p", "no:cacheprovider", "tests"]


def _load(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def _copy_stage(case_id: str, stage: str) -> Path:
    source = FIXTURES / case_id / stage
    target = Path(tempfile.mkdtemp(prefix=f"gv3-{case_id}-{stage}-")) / stage
    shutil.copytree(source, target, ignore=shutil.ignore_patterns("node_modules", ".next", "__pycache__"))
    return target


def _run(cwd: Path, *argv: str, timeout: int = 120) -> subprocess.CompletedProcess:
    env = dict(os.environ, PYTHONDONTWRITEBYTECODE="1", NEXT_TELEMETRY_DISABLED="1")
    return subprocess.run(
        list(argv),
        cwd=cwd,
        capture_output=True,
        text=True,
        timeout=timeout,
        env=env,
        check=False,
    )


class RegistryShapeTest(unittest.TestCase):
    def setUp(self) -> None:
        self.registry = _load(REGISTRY)

    def test_every_workspace_has_its_stage_directories(self):
        for workspace in self.registry["workspaces"]:
            root = ROOT / workspace["root"]
            self.assertTrue(root.is_dir(), workspace["case_id"])
            for stage in workspace["stages"]:
                self.assertTrue((root / stage).is_dir(), f"{workspace['case_id']}/{stage}")
            self.assertIn(workspace["candidate_oracle_execution_stage"], workspace["stages"])
            self.assertIn(workspace["product_run"]["initial_stage"], workspace["stages"])

    def test_primary_cells_are_exactly_the_seven_registered_workspaces(self):
        capabilities = _load(CAPABILITIES)
        primary = sorted(case["case_id"] for case in capabilities["cases"] if case["lane"] == "primary")
        registered = sorted(workspace["case_id"] for workspace in self.registry["workspaces"])
        self.assertEqual(primary, registered)
        unavailable = sorted(case["case_id"] for case in capabilities["cases"] if case["lane"] != "primary")
        self.assertEqual(unavailable, sorted(c["case_id"] for c in self.registry["executor_unavailable_cells"]))

    def test_adapters_reference_existing_workspace_stages_and_no_snapshot_executors(self):
        adapters = _load(ADAPTERS)["adapters"]
        by_case = {w["case_id"]: w for w in self.registry["workspaces"]}
        for adapter in adapters:
            executor = adapter["executor"]
            self.assertFalse(executor["kind"].startswith("snapshot"), adapter["adapter_id"])
            if executor["kind"] == "unavailable":
                self.assertNotIn(adapter["case_id"], by_case)
                continue
            workspace = by_case[executor["workspace"]]
            self.assertEqual(workspace["case_id"], adapter["case_id"])
            self.assertIn(executor["stage"], workspace["stages"], adapter["adapter_id"])
            self.assertTrue((ROOT / workspace["root"] / executor["stage"]).is_dir())

    def test_next_cells_declare_offline_provisioning_with_reserved_ports(self):
        ports = set()
        for adapter in _load(ADAPTERS)["adapters"]:
            server = adapter["executor"].get("server")
            if server:
                self.assertTrue(server["loopback_only"])
                ports.add((adapter["case_id"], server["port"]))
        self.assertEqual(ports, {("create-build-only-functional", 4174), ("create-ui-copy-style-port-path", 4173)})
        for workspace in self.registry["workspaces"]:
            if workspace["profile"] == "nextjs" and workspace["intent"] == "create":
                self.assertEqual(workspace["provisioning"]["mode"], "vendored_tarball")


class CliCellTest(unittest.TestCase):
    def test_known_values(self):
        reference = _copy_stage("create-cli-known-multiple-inputs", "reference")
        self.assertEqual(_run(reference, sys.executable, "sum_cli.py", "2", "3").stdout, "5\n")
        self.assertEqual(_run(reference, sys.executable, "sum_cli.py", "-1", "1").stdout, "0\n")
        initial = FIXTURES / "create-cli-known-multiple-inputs/initial"
        self.assertEqual([p.name for p in initial.iterdir()], [".gitkeep"])


class FixReproducedAfterRegressionTest(unittest.TestCase):
    def test_before_fails_after_passes_regressions_hold(self):
        before = _copy_stage("fix-reproduced-after-regression", "before")
        after = _copy_stage("fix-reproduced-after-regression", "after")
        self.assertEqual(_run(before, sys.executable, "cli.py", "7").returncode, 2)
        self.assertEqual(_run(after, sys.executable, "cli.py", "7").returncode, 0)
        self.assertEqual(_run(before, *PYTEST).returncode, 0)
        self.assertEqual(_run(after, *PYTEST).returncode, 0)
        self.assertEqual(_run(after, sys.executable, "scripts/contract_check.py").returncode, 0)
        readme = (after / "README.md").read_text(encoding="utf-8")
        self.assertIn("pytest-tests", readme)
        self.assertIn("contract-check", readme)


class FixBaselineNotReproducedSubstitutionTest(unittest.TestCase):
    def test_registered_fixture_crashes_before_and_passes_after(self):
        before = _copy_stage("fix-baseline-not-reproduced-substitution", "before")
        after = _copy_stage("fix-baseline-not-reproduced-substitution", "after")
        crash = _run(before, sys.executable, "app.py", "fixture/a.json")
        self.assertEqual(crash.returncode, 1)
        self.assertIn("Traceback", crash.stderr)
        self.assertEqual(_run(before, sys.executable, "app.py", "fixture/b.json").returncode, 0)
        self.assertEqual(_run(after, sys.executable, "app.py", "fixture/a.json").returncode, 0)
        a_before = (before / "fixture/a.json").read_bytes()
        a_after = (after / "fixture/a.json").read_bytes()
        self.assertEqual(a_before, a_after, "fixtures must be identical across stages")
        self.assertNotEqual(a_before, (before / "fixture/b.json").read_bytes())


class FixAfterNotRunRegressionShrunkTest(unittest.TestCase):
    def test_reproducer_and_full_regression_set(self):
        before = _copy_stage("fix-after-not-run-regression-shrunk", "before")
        after = _copy_stage("fix-after-not-run-regression-shrunk", "after")
        self.assertEqual(_run(before, sys.executable, "repro.py").returncode, 1)
        self.assertEqual(_run(after, sys.executable, "repro.py").returncode, 0)
        self.assertEqual(_run(after, *PYTEST).returncode, 0)
        self.assertEqual(_run(after, sys.executable, "scripts/contract_check.py").returncode, 0)


class FixExistingTestsOnlyDependencyMissingTest(unittest.TestCase):
    def test_tests_pass_while_reproducer_dependency_is_absent(self):
        for stage in ("before", "after"):
            workspace = _copy_stage("fix-existing-tests-only-dependency-missing", stage)
            self.assertEqual(_run(workspace, *PYTEST).returncode, 0, stage)
            probe = _run(workspace, sys.executable, "-c", "import repro_dep")
            self.assertEqual(probe.returncode, 1, stage)
            self.assertIn("ModuleNotFoundError", probe.stderr, stage)
            repro = _run(workspace, sys.executable, "repro.py")
            self.assertEqual(repro.returncode, 1, stage)
            self.assertIn("ModuleNotFoundError", repro.stderr, stage)


class NextCellsStaticTest(unittest.TestCase):
    def test_initial_stages_contain_only_a_named_package(self):
        for case_id in ("create-build-only-functional", "create-ui-copy-style-port-path"):
            initial = FIXTURES / case_id / "initial"
            self.assertEqual([p.name for p in initial.iterdir()], ["package.json"])
            self.assertEqual(sorted(_load(initial / "package.json")), ["name", "private"])

    def test_reference_sources_carry_the_required_markers(self):
        counter = FIXTURES / "create-build-only-functional/reference"
        page = (counter / "app/page.js").read_text(encoding="utf-8")
        self.assertIn('id="increment"', page)
        self.assertIn('id="count"', page)
        self.assertIn("useState(0)", page)
        play = FIXTURES / "create-ui-copy-style-port-path/reference"
        self.assertIn("開始", (play / "app/play/page.js").read_text(encoding="utf-8"))
        self.assertIn('backgroundColor: "blue"', (play / "app/layout.js").read_text(encoding="utf-8"))
        for reference in (counter, play):
            package = _load(reference / "package.json")
            self.assertEqual(package["scripts"], {"build": "next build", "start": "next start"})
            self.assertEqual(sorted(package["dependencies"]), ["next", "react", "react-dom"])
            self.assertTrue((reference / "package-lock.json").is_file())


def _free_port_available(port: int) -> bool:
    with socket.socket() as sock:
        return sock.connect_ex(("127.0.0.1", port)) != 0


@unittest.skipUnless(os.environ.get("GOAL_VERIFY_V3_NEXT") == "1", "set GOAL_VERIFY_V3_NEXT=1 with provisioned node_modules")
class NextCellsBuildAndServeTest(unittest.TestCase):
    def _build_and_get(self, case_id: str, port: int, route: str) -> str:
        reference = FIXTURES / case_id / "reference"
        self.assertTrue((reference / "node_modules/next").is_dir(), "node_modules not provisioned")
        self.assertTrue(_free_port_available(port), f"port {port} busy")
        build = _run(reference, "npm", "run", "build", timeout=600)
        self.assertEqual(build.returncode, 0, build.stdout[-2000:] + build.stderr[-2000:])
        server = subprocess.Popen(["npm", "run", "start", "--", "-p", str(port)], cwd=reference, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, env=dict(os.environ, NEXT_TELEMETRY_DISABLED="1"))
        try:
            body = ""
            for _ in range(60):
                try:
                    with urllib.request.urlopen(f"http://127.0.0.1:{port}{route}", timeout=2) as response:
                        self.assertEqual(response.status, 200)
                        body = response.read().decode("utf-8")
                        break
                except (OSError, urllib.error.URLError):
                    time.sleep(1)
            self.assertTrue(body, "server did not answer")
            if os.environ.get("GOAL_VERIFY_V3_PLAYWRIGHT") == "1" and case_id == "create-build-only-functional":
                script = (
                    "const { chromium } = require('playwright');"
                    "(async () => { const b = await chromium.launch(); const p = await b.newPage();"
                    f"await p.goto('http://127.0.0.1:{port}/'); await p.click('#increment'); await p.click('#increment');"
                    "console.log(await p.textContent('#count')); await b.close(); })();"
                )
                probe = _run(reference, "node", "-e", script, timeout=120)
                self.assertEqual(probe.stdout.strip(), "2", probe.stderr[-500:])
            return body
        finally:
            server.terminate()
            try:
                server.wait(timeout=10)
            except subprocess.TimeoutExpired:
                server.kill()

    def test_counter_app_serves_increment_and_count(self):
        body = self._build_and_get("create-build-only-functional", 4174, "/")
        self.assertIn('id="increment"', body)
        self.assertIn('id="count"', body)

    def test_play_app_serves_heading_and_blue_background(self):
        body = self._build_and_get("create-ui-copy-style-port-path", 4173, "/play")
        self.assertIn("開始", body)
        self.assertIn("background-color:blue", body)


@unittest.skipUnless(
    os.environ.get("GOAL_VERIFY_V3_EXECUTOR_SMOKE") == "1",
    "set GOAL_VERIFY_V3_EXECUTOR_SMOKE=1 with provisioned tarballs",
)
class RegisteredBrowserExecutorSmokeTest(unittest.TestCase):
    def test_registered_browser_executors_pass_on_frozen_reference_stages(self):
        provisioned = Path(os.environ["GOAL_VERIFY_V3_PROVISIONED"])
        registry = _load(REGISTRY)
        adapters = _load(ADAPTERS)["adapters"]
        workspaces = {row["case_id"]: row for row in registry["workspaces"]}
        browser_adapters = [
            row for row in adapters if row["executor"]["kind"] == "playwright_script"
        ]
        self.assertEqual(len(browser_adapters), 2)
        for adapter in browser_adapters:
            case_id = adapter["case_id"]
            with tempfile.TemporaryDirectory(prefix=f"gv3-browser-{case_id}-") as tmp:
                workspace = prepare_workspace_stage(
                    root=ROOT,
                    workspace=workspaces[case_id],
                    stage="reference",
                    destination=Path(tmp) / "reference",
                    provisioned_root=provisioned,
                )
                build = _run(workspace, "npm", "run", "build", timeout=600)
                self.assertEqual(build.returncode, 0, build.stderr[-2000:])
                result = execute_registered(adapter["executor"], workspace=workspace)
                self.assertEqual(result["result"], "pass", result)

    @unittest.skipUnless(
        os.environ.get("GOAL_VERIFY_V3_EXECUTOR_PLAIN_DIAGNOSTIC") == "1",
        "diagnostic only: runs trusted registered browser command without sandbox",
    )
    def test_browser_bundle_and_playwright_are_compatible_without_outer_sandbox(self):
        provisioned = Path(os.environ["GOAL_VERIFY_V3_PROVISIONED"])
        registry = _load(REGISTRY)
        adapters = _load(ADAPTERS)["adapters"]
        workspaces = {row["case_id"]: row for row in registry["workspaces"]}

        def plain_runner(argv, cwd, timeout_ms):
            completed = subprocess.run(
                argv,
                cwd=cwd,
                capture_output=True,
                text=True,
                timeout=timeout_ms / 1000,
                check=False,
            )
            return {
                "exit_code": completed.returncode,
                "stdout": completed.stdout,
                "stderr": completed.stderr,
                "runtime_ms": 0,
            }

        for adapter in [
            row for row in adapters if row["executor"]["kind"] == "playwright_script"
        ]:
            case_id = adapter["case_id"]
            with tempfile.TemporaryDirectory(prefix=f"gv3-plain-{case_id}-") as tmp:
                workspace = prepare_workspace_stage(
                    root=ROOT,
                    workspace=workspaces[case_id],
                    stage="reference",
                    destination=Path(tmp) / "reference",
                    provisioned_root=provisioned,
                )
                build = _run(workspace, "npm", "run", "build", timeout=600)
                self.assertEqual(build.returncode, 0, build.stderr[-2000:])
                result = execute_registered(
                    adapter["executor"], workspace=workspace, runner=plain_runner
                )
                self.assertEqual(result["result"], "pass", result)


if __name__ == "__main__":
    unittest.main()
