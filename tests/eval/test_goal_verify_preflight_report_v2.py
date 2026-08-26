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
)
from eval_lib.goal_verify_preflight_report_v2 import build_preflight_report


class GoalVerifyPreflightReportV2Test(unittest.TestCase):
    def test_registered_same_scope_fixture_passes_contract_integration_report(self):
        corpus = json.loads(
            (ROOT / "eval/goal_verify/v0/corpus.json").read_text(encoding="utf-8")
        )
        contract = json.loads(
            (ROOT / "eval/goal_verify/v0/phase6-preflight-v2-contract.json").read_text(
                encoding="utf-8"
            )
        )
        snapshots = json.loads(
            (ROOT / "eval/goal_verify/v0/phase6-artifact-snapshots-v2.json").read_text(
                encoding="utf-8"
            )
        )
        adapters = json.loads(
            (ROOT / "eval/goal_verify/v0/phase6-command-adapters-v2.json").read_text(
                encoding="utf-8"
            )
        )["adapters"]
        cases = {
            case["case_id"]: case
            for case in corpus["cases"]
            if case["intent"] in {"create", "fix"}
        }

        def fake_runner(plan):
            stdout = "5\n" if plan["argv"][-2:] == ["2", "3"] else "0\n"
            return {
                "exit_code": 0,
                "stdout": stdout,
                "stderr": "",
                "timed_out": False,
                "runtime_ms": 1,
            }

        records = []
        with tempfile.TemporaryDirectory(dir=ROOT) as temporary:
            run_root = Path(temporary)
            for snapshot_case in snapshots["cases"]:
                case_id = snapshot_case["case_id"]
                workspace = prepare_snapshot_workspace(
                    root=ROOT,
                    snapshot_case=snapshot_case,
                    destination=run_root / case_id,
                )
                spec = build_registered_baseline_spec(
                    case=cases[case_id], adapters=adapters
                )
                for sample in range(1, 6):
                    result = evaluate_spec_against_snapshot(
                        case_id=case_id,
                        spec=spec,
                        adapters=adapters,
                        workspace=workspace,
                        sandbox_runner=fake_runner,
                    )
                    candidate_rows = [
                        {**row, "arm": "candidate"} for row in result["evaluations"]
                    ]
                    baseline_rows = [
                        {**row, "arm": "baseline"} for row in result["evaluations"]
                    ]
                    records.append(
                        {
                            "pair_id": f"{case_id}--pair-{sample:02d}",
                            "validation": {"valid": True, "errors": []},
                            "oracle_evaluations": candidate_rows,
                            "baseline_oracle_evaluations": baseline_rows,
                        }
                    )
        report = build_preflight_report(
            records=records, contract=contract, adapters=adapters
        )
        self.assertEqual(report["record_count"], 40)
        self.assertEqual(report["schema_passes"], 40)
        self.assertEqual(report["command_oracle"]["denominator"], 10)
        self.assertEqual(report["command_oracle"]["success_rate"], 1.0)
        self.assertEqual(report["fix_adapter_contract"]["integrity_rate"], 1.0)
        self.assertTrue(report["ready_for_full_experiment_design"])
        self.assertTrue(all(report["checks"].values()))

    def test_arm_copy_or_missing_records_fail_closed(self):
        contract = json.loads(
            (ROOT / "eval/goal_verify/v0/phase6-preflight-v2-contract.json").read_text(
                encoding="utf-8"
            )
        )
        adapters = json.loads(
            (ROOT / "eval/goal_verify/v0/phase6-command-adapters-v2.json").read_text(
                encoding="utf-8"
            )
        )["adapters"]
        report = build_preflight_report(
            records=[
                {
                    "pair_id": "one",
                    "validation": {"valid": True, "errors": []},
                    "oracle_evaluations": [{"arm": "baseline"}],
                    "baseline_oracle_evaluations": [{"arm": "baseline"}],
                }
            ],
            contract=contract,
            adapters=adapters,
        )
        self.assertFalse(report["checks"]["record_integrity"])
        self.assertFalse(report["checks"]["baseline_observation_copy"])
        self.assertFalse(report["ready_for_full_experiment_design"])


if __name__ == "__main__":
    unittest.main()
