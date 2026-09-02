from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib import goal_verify_recovery_deterministic_pair as instrument
from eval_lib.goal_verify_recovery_confirmatory import (
    SOURCE_PATHS,
    _task_prepare,
    build_pair_row,
    contract_errors,
    sha256_file,
    task_corpus_sha256,
)


def _design():
    return json.loads(
        (
            ROOT
            / "tests/fixtures/goal_verify_recovery_confirmatory/design-v1.json"
        ).read_text(encoding="utf-8")
    )


class ConfirmatoryRunnerTest(unittest.TestCase):
    def test_task_prepare_binds_distinct_source_to_registered_path(self):
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary)
            prepare = _task_prepare("01")
            prepare(
                instrument.SCENARIOS["generic-fix"],
                workspace,
                workspace / "unused-node-modules",
            )
            self.assertEqual(
                (workspace / "fixture/task-02.json").read_bytes(),
                (workspace / "fixture/task-01.json").read_bytes(),
            )

    def test_contract_validation_accepts_bound_sources(self):
        contract = {
            "schema_version": "commandagent.goal_verify.recovery_confirmatory.v1",
            "status": "frozen",
            "design": _design(),
            "authorization": {"confirmatory_collection_authorized": True},
            "conditional_effect_claim_allowed": True,
            "generalization_claim_allowed": False,
            "default_rollout_allowed": False,
            "task_corpus_sha256": task_corpus_sha256(_design()["task_ids"]),
            "authoritative_source_sha256": {
                relative: sha256_file(ROOT / relative) for relative in SOURCE_PATHS
            },
        }
        self.assertEqual(contract_errors(contract), [])

    def test_pair_row_detects_discarded_valid_treatment(self):
        base_checks = {
            "registered_regressions_passed": True,
            "protected_paths_unchanged": True,
        }
        control = {
            "arm_valid": True,
            "registered_endpoint_success": False,
            "input_snapshot_sha256": "input",
            "boundary_signature_sha256": "boundary",
            "checks": base_checks,
        }
        treatment = {
            "arm_valid": False,
            "registered_endpoint_success": True,
            "input_snapshot_sha256": "input",
            "boundary_signature_sha256": "boundary",
            "checks": {
                "all_registered_commands_passed": True,
                "protected_paths_unchanged": True,
                "treatment_promoted": False,
            },
        }
        with tempfile.TemporaryDirectory() as temporary:
            events = Path(temporary) / "events.jsonl"
            events.write_text(
                json.dumps(
                    {
                        "event": "tui_command_stop",
                        "ok": True,
                        "failure_kind": "",
                        "stop_reason": "completed",
                    }
                )
                + "\n",
                encoding="utf-8",
            )
            row = build_pair_row(
                "generic--pair-01", control, treatment, events
            )
        self.assertEqual(row["discarded_valid_treatment_count"], 1)
        self.assertFalse(row["pair_valid"])


if __name__ == "__main__":
    unittest.main()
