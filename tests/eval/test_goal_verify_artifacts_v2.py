import copy
import json
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_artifacts_v2 import (
    build_registered_baseline_spec,
    evaluate_spec_against_snapshot,
    prepare_snapshot_workspace,
    validate_adapter_registry,
    validate_snapshot_registry,
)
from eval_lib.goal_verify_sandbox import run_macos_sandbox, sandbox_backend_status


class GoalVerifyArtifactsV2Test(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.corpus = json.loads(
            (ROOT / "eval/goal_verify/v0/corpus.json").read_text(encoding="utf-8")
        )
        cls.snapshots = json.loads(
            (ROOT / "eval/goal_verify/v0/phase6-artifact-snapshots-v2.json").read_text(
                encoding="utf-8"
            )
        )
        cls.adapter_registry = json.loads(
            (ROOT / "eval/goal_verify/v0/phase6-command-adapters-v2.json").read_text(
                encoding="utf-8"
            )
        )

    def test_checked_in_snapshot_and_adapter_registries_are_complete(self):
        self.assertEqual(
            validate_snapshot_registry(
                root=ROOT, registry=self.snapshots, corpus=self.corpus
            ),
            [],
        )
        self.assertEqual(
            validate_adapter_registry(
                adapters=self.adapter_registry, corpus=self.corpus
            ),
            [],
        )

    def test_registered_baseline_uses_same_snapshot_boundary_without_blocks(self):
        cases = {
            case["case_id"]: case
            for case in self.corpus["cases"]
            if case["intent"] in {"create", "fix"}
        }
        adapters = self.adapter_registry["adapters"]

        def fake_runner(plan):
            argv = plan["argv"]
            stdout = "5\n" if argv[-2:] == ["2", "3"] else "0\n"
            return {
                "exit_code": 0,
                "stdout": stdout,
                "stderr": "",
                "timed_out": False,
                "runtime_ms": 1,
            }

        with tempfile.TemporaryDirectory(dir=ROOT) as temporary:
            root = Path(temporary)
            results = []
            for snapshot_case in self.snapshots["cases"]:
                case_id = snapshot_case["case_id"]
                workspace = prepare_snapshot_workspace(
                    root=ROOT,
                    snapshot_case=snapshot_case,
                    destination=root / case_id,
                )
                spec = build_registered_baseline_spec(
                    case=cases[case_id], adapters=adapters
                )
                result = evaluate_spec_against_snapshot(
                    case_id=case_id,
                    spec=spec,
                    adapters=adapters,
                    workspace=workspace,
                    sandbox_runner=fake_runner,
                )
                results.extend(result["evaluations"])
                self.assertEqual(result["unmatched_candidate_oracle_ids"], [])
            self.assertFalse(
                [row for row in results if row["result"] == "blocked"], results
            )
            self.assertTrue([row for row in results if row["result"] == "fail"])
            self.assertTrue([row for row in results if row["result"] == "pass"])

    def test_candidate_oracle_contract_mismatch_fails_concretization(self):
        case = next(
            case
            for case in self.corpus["cases"]
            if case["case_id"] == "create-build-only-functional"
        )
        snapshot_case = next(
            row for row in self.snapshots["cases"] if row["case_id"] == case["case_id"]
        )
        spec = build_registered_baseline_spec(
            case=case, adapters=self.adapter_registry["adapters"]
        )
        spec = copy.deepcopy(spec)
        spec["oracles"][0]["strategy"] = "command"
        with tempfile.TemporaryDirectory(dir=ROOT) as temporary:
            workspace = prepare_snapshot_workspace(
                root=ROOT,
                snapshot_case=snapshot_case,
                destination=Path(temporary) / "workspace",
            )
            result = evaluate_spec_against_snapshot(
                case_id=case["case_id"],
                spec=spec,
                adapters=self.adapter_registry["adapters"],
                workspace=workspace,
                sandbox_runner=lambda plan: self.fail(f"unexpected execution: {plan}"),
            )
        self.assertEqual(result["evaluations"][0]["result"], "blocked")
        self.assertEqual(
            result["evaluations"][0]["reason"],
            "candidate_oracle_contract_not_matched",
        )

    def test_prepared_snapshot_is_resumable_and_detects_file_tampering(self):
        snapshot_case = next(
            row
            for row in self.snapshots["cases"]
            if row["case_id"] == "create-cli-known-multiple-inputs"
        )
        with tempfile.TemporaryDirectory(dir=ROOT) as temporary:
            destination = Path(temporary) / "workspace"
            first = prepare_snapshot_workspace(
                root=ROOT, snapshot_case=snapshot_case, destination=destination
            )
            second = prepare_snapshot_workspace(
                root=ROOT, snapshot_case=snapshot_case, destination=destination
            )
            self.assertEqual(first, second)
            (destination / "sum_cli.py").write_text("tampered", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "file mismatch"):
                prepare_snapshot_workspace(
                    root=ROOT, snapshot_case=snapshot_case, destination=destination
                )

    @unittest.skipUnless(
        sandbox_backend_status()["available"], "macOS sandbox unavailable"
    )
    def test_cli_snapshot_executes_inside_macos_sandbox(self):
        case = next(
            case
            for case in self.corpus["cases"]
            if case["case_id"] == "create-cli-known-multiple-inputs"
        )
        snapshot_case = next(
            row for row in self.snapshots["cases"] if row["case_id"] == case["case_id"]
        )
        with tempfile.TemporaryDirectory(dir=ROOT) as temporary:
            workspace = prepare_snapshot_workspace(
                root=ROOT,
                snapshot_case=snapshot_case,
                destination=Path(temporary) / "workspace",
            )
            spec = build_registered_baseline_spec(
                case=case, adapters=self.adapter_registry["adapters"]
            )
            result = evaluate_spec_against_snapshot(
                case_id=case["case_id"],
                spec=spec,
                adapters=self.adapter_registry["adapters"],
                workspace=workspace,
                sandbox_runner=run_macos_sandbox,
            )
        self.assertEqual(
            [row["result"] for row in result["evaluations"]], ["pass", "pass"]
        )


if __name__ == "__main__":
    unittest.main()
