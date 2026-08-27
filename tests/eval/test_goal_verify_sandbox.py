import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_sandbox import (
    _remaining_timeout_ms,
    _sandboxed_command,
    run_macos_sandbox,
    run_macos_sandbox_web_probe,
    sandbox_backend_status,
)
from eval_lib.goal_verify_v2 import (
    _plan_hash,
    concretize_registered_command,
    evaluate_concretized_command,
)


class GoalVerifySandboxTest(unittest.TestCase):
    def test_web_prepare_and_probe_share_one_timeout_budget(self):
        with mock.patch(
            "eval_lib.goal_verify_sandbox.time.monotonic_ns",
            return_value=1_250_000_000,
        ):
            self.assertEqual(_remaining_timeout_ms(1_000_000_000, 1_000), 750)

    def test_web_prepare_keeps_network_denied(self):
        with tempfile.TemporaryDirectory(dir=ROOT) as temporary:
            workspace = Path(temporary)
            plan = {
                "source": "host_validated_candidate_web_v4",
                "raw_provider_argv_used": False,
                "workspace_root": str(workspace),
                "cwd": str(workspace),
                "prepare_argv": ["npx", "next", "build"],
                "server_argv": ["npx", "next", "start", "-p", "3000"],
                "timeout_ms": 1000,
                "port": 3000,
                "ready_path": "/",
            }
            plan["plan_sha256"] = _plan_hash(plan)
            completed = mock.Mock(returncode=1, stdout=b"", stderr=b"")
            with (
                mock.patch(
                    "eval_lib.goal_verify_sandbox.sandbox_backend_status",
                    return_value={"available": True},
                ),
                mock.patch(
                    "eval_lib.goal_verify_sandbox._sandboxed_command",
                    wraps=_sandboxed_command,
                ) as command,
                mock.patch(
                    "eval_lib.goal_verify_sandbox.subprocess.run",
                    return_value=completed,
                ),
            ):
                result = run_macos_sandbox_web_probe(plan)
            self.assertEqual(result["reason"], "server_prepare_failed")
            self.assertFalse(command.call_args.kwargs["loopback"])

    def test_sandbox_signals_are_limited_to_same_sandbox(self):
        with tempfile.TemporaryDirectory(dir=ROOT) as temporary:
            profile = _sandboxed_command(
                Path(temporary), ["python3", "-c", "pass"], loopback=False
            )[2]
        self.assertIn("(allow signal (target same-sandbox))", profile)
        self.assertNotIn("(allow signal)\n", profile)

    def test_backend_is_fail_closed(self):
        status = sandbox_backend_status()
        self.assertEqual(status["fallback"], "fail_closed")
        self.assertEqual(status["network"], "denied")
        self.assertEqual(status["writes"], "workspace_only")

    @unittest.skipUnless(
        sandbox_backend_status()["available"], "macOS sandbox unavailable"
    )
    def test_loopback_profile_allows_local_bind(self):
        with tempfile.TemporaryDirectory(dir=ROOT) as temporary:
            workspace = Path(temporary)
            completed = subprocess.run(
                _sandboxed_command(
                    workspace,
                    [
                        "python3",
                        "-c",
                        "import socket; s=socket.socket(); s.bind(('127.0.0.1', 0)); s.close()",
                    ],
                    loopback=True,
                ),
                cwd=workspace,
                capture_output=True,
                check=False,
            )
            self.assertEqual(completed.returncode, 0, completed.stderr.decode())

    @unittest.skipUnless(
        sandbox_backend_status()["available"], "macOS sandbox unavailable"
    )
    def test_registered_command_runs_in_workspace_sandbox(self):
        with tempfile.TemporaryDirectory(dir=ROOT) as temporary:
            workspace = Path(temporary)
            oracle = {"id": "printf", "strategy": "stdout", "timeout_ms": 1000}
            adapter = {
                "oracle_id": "printf",
                "argv": ["printf", "sandbox-ok"],
                "observation": {"kind": "stdout", "expected": "sandbox-ok"},
            }
            plan = concretize_registered_command(
                oracle=oracle, adapter=adapter, workspace_root=workspace
            )
            evaluation = evaluate_concretized_command(plan, runner=run_macos_sandbox)
            self.assertEqual(evaluation["result"], "pass", evaluation)

    @unittest.skipUnless(
        sandbox_backend_status()["available"], "macOS sandbox unavailable"
    )
    def test_sandbox_denies_write_outside_workspace(self):
        with tempfile.TemporaryDirectory(dir=ROOT) as temporary:
            run_root = Path(temporary)
            workspace = run_root / "workspace"
            workspace.mkdir()
            outside = run_root / "outside.txt"
            script = (
                "from pathlib import Path\n"
                "try:\n"
                f" Path({str(outside)!r}).write_text('forbidden')\n"
                "except OSError as error:\n"
                " print(error.errno, end='')\n"
            )
            oracle = {"id": "deny-write", "strategy": "stdout", "timeout_ms": 1000}
            adapter = {
                "oracle_id": "deny-write",
                "argv": ["python3", "-c", script],
                "observation": {"kind": "stdout", "expected": "1"},
            }
            plan = concretize_registered_command(
                oracle=oracle, adapter=adapter, workspace_root=workspace
            )
            evaluation = evaluate_concretized_command(plan, runner=run_macos_sandbox)
            self.assertEqual(evaluation["result"], "pass", evaluation)
            self.assertFalse(outside.exists())


if __name__ == "__main__":
    unittest.main()
