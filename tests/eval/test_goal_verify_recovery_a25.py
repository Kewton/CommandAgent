import copy
import json
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_recovery_a25_report import (
    RECOVERY_INNER_VERIFY_BINDING_POLICY_V2,
    _valid_inner_recovery_bindings_v2,
    build_recovery_a25_pilot_report,
    recovery_a25_contract_errors,
)


def load(relative: str) -> dict:
    return json.loads((ROOT / relative).read_text(encoding="utf-8"))


def read_only_binding() -> dict:
    return {
        "binding_mode": "read_only_inspection",
        "binding_stage": "pre_lint",
        "bound_verify_commands": [],
        "external_oracle_used": False,
        "registered_verify_commands": ["python3 app.py fixture/task-05.json"],
        "source": "product_visible_completion_contract",
    }


def final_binding() -> dict:
    command = "python3 app.py fixture/task-05.json"
    return {
        "binding_mode": "completion_contract_final_success",
        "binding_stage": "pre_lint",
        "bound_verify_commands": [command],
        "external_oracle_used": False,
        "registered_verify_commands": [command],
        "source": "product_visible_completion_contract",
    }


def rejected_attempts(*, changed_field: str | None = None) -> dict:
    product_delta = {
        "changed_paths": [],
        "added_paths": [],
        "removed_paths": [],
    }
    if changed_field is not None:
        product_delta[changed_field] = ["app.py"]
    return {
        "executed_recovery_runs": 1,
        "step_plan_contract_bindings": [read_only_binding()],
        "treatment_deltas": [
            {
                "attempted_product_delta": product_delta,
                "treatment_runtime_evidence_delta": {
                    "changed_paths": [],
                    "added_paths": [],
                    "removed_paths": [],
                },
            }
        ],
        "promotion_decisions": [
            {"decision": "rejected", "reason": "recovery_execution_failed"}
        ],
        "control_retained_count": 1,
        "control_restore_failed_count": 0,
        "terminal_stop_reason": "not_recoverable",
    }


class RecoveryA25ReportTest(unittest.TestCase):
    def test_contract_requires_exact_versioned_inner_binding_policy(self):
        contract = load(
            "eval/goal_verify/v0/phase6-recovery-v4-a24-pilot-contract.json"
        )
        self.assertIn(
            "recovery_inner_verify_binding_policy_invalid",
            recovery_a25_contract_errors(contract),
        )
        contract["smoke"]["recovery_inner_verify_binding_policy"] = copy.deepcopy(
            RECOVERY_INNER_VERIFY_BINDING_POLICY_V2
        )
        self.assertEqual(recovery_a25_contract_errors(contract), [])
        contract["smoke"]["require_registered_inner_recovery_verify_commands"] = False
        self.assertIn(
            "registered_inner_recovery_verify_commands_must_be_required",
            recovery_a25_contract_errors(contract),
        )
        contract["smoke"]["require_registered_inner_recovery_verify_commands"] = True
        contract["smoke"]["recovery_inner_verify_binding_policy"]["schema_version"] = (
            "invalid"
        )
        self.assertIn(
            "recovery_inner_verify_binding_policy_invalid",
            recovery_a25_contract_errors(contract),
        )

    def test_v2_accepts_only_honest_pre_mutation_read_only_rejection(self):
        attempts = rejected_attempts()
        self.assertTrue(
            _valid_inner_recovery_bindings_v2(
                attempts["step_plan_contract_bindings"],
                attempts,
                require_pre_lint=True,
            )
        )
        for changed_field in ("changed_paths", "added_paths", "removed_paths"):
            with self.subTest(changed_field=changed_field):
                mutated = rejected_attempts(changed_field=changed_field)
                self.assertFalse(
                    _valid_inner_recovery_bindings_v2(
                        mutated["step_plan_contract_bindings"],
                        mutated,
                        require_pre_lint=True,
                    )
                )
        promoted = rejected_attempts()
        promoted["promotion_decisions"] = [
            {"decision": "promoted", "reason": "registered_final_success_passed"}
        ]
        self.assertFalse(
            _valid_inner_recovery_bindings_v2(
                promoted["step_plan_contract_bindings"], promoted
            )
        )
        unretained = rejected_attempts()
        unretained["control_retained_count"] = 0
        self.assertFalse(
            _valid_inner_recovery_bindings_v2(
                unretained["step_plan_contract_bindings"], unretained
            )
        )

    def test_v2_requires_both_modes_after_final_binding_exists(self):
        attempts = rejected_attempts(changed_field="changed_paths")
        attempts["step_plan_contract_bindings"].append(final_binding())
        self.assertTrue(
            _valid_inner_recovery_bindings_v2(
                attempts["step_plan_contract_bindings"],
                attempts,
                require_pre_lint=True,
            )
        )
        missing_read_only = copy.deepcopy(attempts)
        missing_read_only["step_plan_contract_bindings"] = [final_binding()]
        self.assertFalse(
            _valid_inner_recovery_bindings_v2(
                missing_read_only["step_plan_contract_bindings"], missing_read_only
            )
        )
        malformed = rejected_attempts()
        malformed["step_plan_contract_bindings"][0]["registered_verify_commands"] = []
        self.assertFalse(
            _valid_inner_recovery_bindings_v2(
                malformed["step_plan_contract_bindings"], malformed
            )
        )

    def test_pilot_report_recomputes_inner_binding_and_next_design(self):
        contract = load(
            "eval/goal_verify/v0/phase6-recovery-v4-a24-pilot-contract.json"
        )
        contract["smoke"]["recovery_inner_verify_binding_policy"] = copy.deepcopy(
            RECOVERY_INNER_VERIFY_BINDING_POLICY_V2
        )
        attempts = rejected_attempts()
        record = {
            "pair_id": "generic-task-05--pair-01",
            "recovery_one": {"result": {"recovery_plan_attempts": attempts}},
            "comparison": {"effect_attribution_ready": True},
        }
        profile_readiness = {
            profile: {"executed_recovery_clusters": 1}
            for profile in contract["smoke"]["required_real_profiles"]
        }
        base = {
            "checks": {
                "registered_inner_recovery_verify_commands": False,
                "attributed_harm_zero": True,
                "regression_introduced_zero": True,
                "existing_artifact_harm_zero": True,
                "discarded_valid_treatment_zero": True,
                "transaction_control_retention": True,
                "isolated_recovery_treatment": True,
                "recovery_fix_safety_verification": True,
            },
            "diagnostics": {
                "inner_recovery_verify_command_violations": [record["pair_id"]],
                "instrumentation_unusable_pair_ids": [],
            },
            "a15_profile_smoke_checks": {"all_profiles_ready": True},
            "profile_readiness": profile_readiness,
            "instrument_ready": False,
            "effect_attribution_ready": False,
            "go_no_go": "NO-GO",
        }
        with patch(
            "eval_lib.goal_verify_recovery_a25_report.build_recovery_a15_smoke_report",
            return_value=copy.deepcopy(base),
        ):
            report = build_recovery_a25_pilot_report(
                records=[record], contract=contract
            )
        self.assertTrue(report["checks"]["registered_inner_recovery_verify_commands"])
        self.assertEqual(
            report["diagnostics"]["inner_recovery_verify_command_violations"], []
        )
        self.assertTrue(report["pilot_instrument_ready"])
        self.assertEqual(report["pilot_go_no_go"], "GO")
        self.assertEqual(report["natural_exposure_threshold_status"], "MET")
        self.assertEqual(
            report["next_design_decision"],
            "preregister_natural_exposure_confirmatory_experiment",
        )
        self.assertFalse(report["effect_claim_allowed"])


if __name__ == "__main__":
    unittest.main()
