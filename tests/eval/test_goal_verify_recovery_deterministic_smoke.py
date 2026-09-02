from __future__ import annotations

import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_recovery_deterministic_smoke import (
    DATA_REGRESSION_COMMANDS,
    DATA_REGRESSION_IDS,
    DATA_REPRO_COMMAND,
    ScriptedDataFixRecoveryProvider,
    ScriptedRecoveryProvider,
    build_data_fix_report,
    build_report,
)


def event(name: str, **fields):
    return {"event": name, **fields}


class DeterministicRecoverySmokeTest(unittest.TestCase):
    def test_scripted_provider_requires_failure_then_read_write_verify(self):
        provider = ScriptedRecoveryProvider()
        planner = lambda text: {"messages": [{"content": text}], "tools": []}
        execution = {"messages": [], "tools": [{"function": {"name": "Read"}}]}

        provider.response_for(planner("initial phase"))
        initial = provider.response_for(execution)
        self.assertNotIn("tool_calls", initial["message"])
        self.assertIn("no workspace edit", initial["message"]["content"])

        provider.response_for(
            planner("Inspect the current workspace before changing files")
        )
        inspect = provider.response_for(execution)
        self.assertEqual(
            inspect["message"]["tool_calls"][0]["function"]["name"], "Read"
        )

        provider.response_for(
            planner("Repair the incomplete work for the failed phase")
        )
        repair = provider.response_for(execution)
        self.assertEqual(
            repair["message"]["tool_calls"][0]["function"]["name"], "Write"
        )

        provider.response_for(
            planner("Verify the recovered output with deterministic checks")
        )
        verify = provider.response_for(execution)
        self.assertEqual(verify["message"]["tool_calls"][0]["function"]["name"], "Bash")

    def test_report_is_go_only_for_complete_promoted_transaction(self):
        rows = [
            event("recovery_prompt_saved", status="incomplete"),
            event(
                "recovery_preflight_observation",
                observation_phase="pre_recovery",
                status="fail",
                source="product_visible_completion_contract",
                external_oracle_used=False,
            ),
            event(
                "recovery_candidate_verify_commands_bound",
                source="product_visible_completion_contract",
                registered_verify_command_count=1,
                recovery_verify_command_source="completion_contract",
                external_oracle_used=False,
            ),
            event("recovery_boundary_snapshot", status="captured"),
            event("recovery_plan_auto_run_start", recovery_plan_auto_run_current=1),
            event(
                "recovery_treatment_delta",
                attempted_product_delta={"changed_paths": ["result.txt"]},
            ),
            event(
                "recovery_preflight_observation",
                observation_phase="post_recovery",
                status="pass",
                source="product_visible_completion_contract",
                external_oracle_used=False,
            ),
            event(
                "recovery_promotion_decision",
                decision="promoted",
                external_oracle_used=False,
            ),
            event(
                "recovery_plan_auto_run_complete",
                recovery_plan_auto_run_stop_reason="recovery_succeeded",
            ),
        ]
        trace = [
            {"response_kind": "Read"},
            {"response_kind": "Write"},
            {"response_kind": "Bash"},
        ]
        report = build_report(
            rows=rows,
            returncode=0,
            final_artifact="recovered\n",
            provider_trace=trace,
            binary_sha256="a" * 64,
        )

        self.assertTrue(report["instrument_ready"])
        self.assertEqual(report["go_no_go"], "GO")
        self.assertFalse(report["effect_claim_allowed"])

        rejected = [
            row for row in rows if row.get("event") != "recovery_promotion_decision"
        ]
        rejected_report = build_report(
            rows=rejected,
            returncode=0,
            final_artifact="recovered\n",
            provider_trace=trace,
            binary_sha256="a" * 64,
        )
        self.assertFalse(rejected_report["instrument_ready"])
        self.assertEqual(rejected_report["go_no_go"], "NO-GO")

    def test_data_fix_provider_reads_then_writes_the_pipeline(self):
        provider = ScriptedDataFixRecoveryProvider("corrected pipeline\n")
        planner = lambda text: {"messages": [{"content": text}], "tools": []}
        execution = {"messages": [], "tools": [{"function": {"name": "Read"}}]}

        provider.response_for(
            planner("Inspect the current workspace before changing files")
        )
        inspect = provider.response_for(execution)
        self.assertEqual(
            inspect["message"]["tool_calls"][0]["function"],
            {"name": "Read", "arguments": {"path": "pipeline/main.py"}},
        )

        provider.response_for(
            planner("Repair the incomplete work for the failed phase")
        )
        repair = provider.response_for(execution)
        self.assertEqual(
            repair["message"]["tool_calls"][0]["function"],
            {
                "name": "Write",
                "arguments": {
                    "path": "pipeline/main.py",
                    "content": "corrected pipeline\n",
                },
            },
        )

    def test_data_fix_report_requires_full_bound_regression_lineage(self):
        rows = [
            event(
                "fix_evidence_recorded",
                requirement_id="before_fails",
                binding_id=DATA_REPRO_COMMAND,
                executed=True,
                outcome="failure",
            ),
            event(
                "recovery_preflight_observation",
                observation_phase="pre_recovery",
                status="fail",
                source="product_visible_completion_contract",
                external_oracle_used=False,
            ),
            event(
                "recovery_fix_contract_resumed",
                regression_source="completion_contract",
                bound_regression_ids=list(DATA_REGRESSION_IDS),
                omitted_supplemental_ids=["pipeline_probe"],
                external_oracle_used=False,
            ),
            event(
                "recovery_observation_effect_policy_bound",
                registered_data_input_fixture="data/task-02.csv",
                source="product_visible_completion_contract",
                external_oracle_used=False,
            ),
            event(
                "recovery_treatment_delta",
                attempted_product_delta={"changed_paths": ["pipeline/main.py"]},
            ),
            event(
                "fix_evidence_recorded",
                requirement_id="after_passes",
                binding_id=DATA_REPRO_COMMAND,
                executed=True,
                outcome="success",
            ),
            *[
                event(
                    "fix_evidence_recorded",
                    requirement_id="no_regression",
                    binding_id=binding_id,
                    executed=True,
                    outcome="success",
                    reason="",
                )
                for binding_id in DATA_REGRESSION_IDS
            ],
            event(
                "ultra_final_acceptance",
                verdict="full",
                external_contract_ok=True,
                requirement_statuses={
                    "after_passes": "passed",
                    "before_fails": "passed",
                    "no_regression": "passed",
                },
            ),
            event(
                "recovery_preflight_observation",
                observation_phase="post_recovery",
                status="pass",
                source="product_visible_completion_contract",
                verify_command_count=3,
                external_oracle_used=False,
            ),
            event(
                "recovery_promotion_decision",
                decision="promoted",
                external_oracle_used=False,
            ),
            event(
                "recovery_plan_auto_run_complete",
                recovery_plan_auto_run_stop_reason="recovery_succeeded",
            ),
            event(
                "tui_command_stop",
                completion_status="complete",
                final_acceptance_status="full_success",
                assurance_level="full",
                ok=True,
            ),
        ]
        diagnostics = {
            DATA_REPRO_COMMAND: 0,
            DATA_REGRESSION_COMMANDS[0]: 0,
            DATA_REGRESSION_COMMANDS[1]: 0,
        }
        report = build_data_fix_report(
            rows=rows,
            returncode=0,
            final_pipeline='"used_rows": len(valid_rows),\n',
            provider_trace=[
                {"response_kind": "Read"},
                {"response_kind": "Write"},
            ],
            binary_sha256="a" * 64,
            diagnostic_returncodes=diagnostics,
        )

        self.assertTrue(report["instrument_ready"])
        self.assertEqual(report["go_no_go"], "GO")
        self.assertFalse(report["effect_claim_allowed"])

        rows[6]["outcome"] = "failure"
        rejected_report = build_data_fix_report(
            rows=rows,
            returncode=0,
            final_pipeline='"used_rows": len(valid_rows),\n',
            provider_trace=[
                {"response_kind": "Read"},
                {"response_kind": "Write"},
            ],
            binary_sha256="a" * 64,
            diagnostic_returncodes=diagnostics,
        )
        self.assertFalse(rejected_report["instrument_ready"])
        self.assertEqual(rejected_report["go_no_go"], "NO-GO")


if __name__ == "__main__":
    unittest.main()
