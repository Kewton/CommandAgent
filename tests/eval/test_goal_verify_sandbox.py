import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_sandbox import (
    run_macos_sandbox,
    sandbox_backend_status,
)
from eval_lib.goal_verify_v2 import (
    concretize_registered_command,
    evaluate_concretized_command,
)


class GoalVerifySandboxTest(unittest.TestCase):
    def test_backend_is_fail_closed(self):
        status = sandbox_backend_status()
        self.assertEqual(status["fallback"], "fail_closed")
        self.assertEqual(status["network"], "denied")
        self.assertEqual(status["writes"], "workspace_only")

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
