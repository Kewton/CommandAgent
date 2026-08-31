import copy
import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.generate_goal_verify_recovery_v4_a15_a1 import build_contract
from eval_lib.goal_verify_recovery_a15_report import (
    build_recovery_a15_full_report,
    build_recovery_a15_smoke_report,
)
from eval_lib.goal_verify_recovery_experiment_v4 import (
    recovery_contract_errors,
    validate_a14_oracle_semantics,
)
from eval_lib.goal_verify_task_contracts_v4 import (
    bind_task_contract,
    task_contract_registry_errors,
)
from eval_lib.goal_verify_workspaces_v3 import workspace_file_hashes


def load(relative: str) -> dict:
    return json.loads((ROOT / relative).read_text(encoding="utf-8"))


def run(cwd: Path, *argv: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        list(argv), cwd=cwd, text=True, capture_output=True, check=False
    )


class GoalVerifyRecoveryA15InputsTest(unittest.TestCase):
    def test_a15_a1_inherits_the_frozen_smoke_without_changing_design(self):
        base = load("eval/goal_verify/v0/phase6-recovery-v4-a15-smoke-contract.json")
        evidence = base["exact_sha_ci_evidence"]
        amended = build_contract(
            code_sha=base["code_sha"],
            exact_sha_ci_evidence=evidence,
            authorized=True,
        )
        self.assertEqual(recovery_contract_errors(amended), [])
        self.assertEqual(
            amended["smoke"]["selected_pair_ids"],
            base["smoke"]["selected_pair_ids"],
        )
        for field in (
            "expected_pair_count",
            "minimum_executed_recovery_pairs",
            "minimum_executed_recovery_pairs_per_real_profile",
            "required_real_profiles",
            "typed_fix_reproducer_commands",
        ):
            self.assertEqual(amended["smoke"][field], base["smoke"][field])
        self.assertEqual(amended["frozen_input_sha256"], base["frozen_input_sha256"])
        self.assertEqual(amended["supersedes_contract"], base["contract_id"])
        self.assertEqual(
            amended["pre_live_amendments"][-1]["amendment_id"], "v4-A15-A1"
        )

    def test_real_profile_contracts_bind_and_freeze_executable_oracles(self):
        corpus = load("eval/goal_verify/v0/phase6-recovery-v4-a15-corpus.json")
        tasks = load("eval/goal_verify/v0/phase6-task-contracts-v4-a15.json")
        adapters = load(
            "eval/goal_verify/v0/phase6-command-adapters-v4-a15.json"
        )["adapters"]
        self.assertEqual(len(corpus["cases"]), 60)
        self.assertEqual(len(tasks["cases"]), 60)
        self.assertEqual(task_contract_registry_errors(tasks), [])
        by_case = {row["case_id"]: row for row in corpus["cases"]}
        for cell in (13, 14):
            for task in range(1, 11):
                case_id = f"phase6-main-c{cell:02d}-task-{task:02d}"
                bound = bind_task_contract(by_case[case_id], tasks)
                self.assertEqual(
                    bound["task_contract"]["completion_contract"]["profile"],
                    "data" if cell == 13 else "nextjs",
                )
                semantics = validate_a14_oracle_semantics(
                    case_id=case_id, intent="fix", adapters=adapters
                )
                self.assertTrue(semantics["valid"], semantics)

    def test_smoke_and_full_population_are_additive_and_valid(self):
        smoke = load("eval/goal_verify/v0/phase6-recovery-v4-a15-smoke-contract.json")
        full = load("eval/goal_verify/v0/phase6-recovery-v4-a15-full-contract.json")
        self.assertEqual(recovery_contract_errors(smoke), [])
        self.assertEqual(recovery_contract_errors(full), [])
        self.assertNotIn("full_experiment", smoke)
        self.assertEqual(len(smoke["smoke"]["selected_pair_ids"]), 14)
        design = full["full_experiment"]
        self.assertEqual(design["eligible_pair_count"], 120)
        self.assertEqual(design["sentinel_pair_count"], 20)
        self.assertEqual(
            design["profile_cells"],
            {
                "cell-05": "cli",
                "cell-07": "generic",
                "cell-13": "data",
                "cell-14": "nextjs",
            },
        )
        self.assertIn(
            "profile-specific 95% CI lower bound above zero", design["go_rule"]
        )

    def test_new_workspace_hashes_match_generated_fixtures(self):
        registry = load("eval/goal_verify/v0/phase6-real-workspaces-v4-a15.json")
        for case_id in (
            "a15-fix-data-reconciliation",
            "a15-fix-nextjs-route-label",
        ):
            workspace = next(
                row for row in registry["workspaces"] if row["case_id"] == case_id
            )
            self.assertEqual(
                workspace_file_hashes(ROOT, workspace),
                workspace["frozen_file_sha256"],
            )

    def test_data_and_next_reference_repairs_reverse_the_exact_reproducer(self):
        fixture_root = ROOT / "tests/fixtures/goal_verify_v4/a15"
        with tempfile.TemporaryDirectory() as temporary:
            temporary_root = Path(temporary)
            for family, command_prefix, fixture_pattern in (
                (
                    "fix-data-reconciliation",
                    [sys.executable, "scripts/repro.py"],
                    "data/task-{task:02d}.csv",
                ),
                (
                    "fix-nextjs-route-label",
                    ["node", "scripts/repro.mjs"],
                    "fixture/task-{task:02d}.json",
                ),
            ):
                before = temporary_root / family / "before"
                after = temporary_root / family / "after"
                shutil.copytree(fixture_root / family / "before", before)
                shutil.copytree(fixture_root / family / "after", after)
                for task in range(1, 11):
                    command = [*command_prefix, fixture_pattern.format(task=task)]
                    self.assertEqual(run(before, *command).returncode, 1)
                    self.assertEqual(run(after, *command).returncode, 0)
            data_before = temporary_root / "fix-data-reconciliation/before"
            data_after = temporary_root / "fix-data-reconciliation/after"
            for workspace in (data_before, data_after):
                self.assertEqual(
                    run(workspace, sys.executable, "-m", "pytest", "-q", "tests").returncode,
                    0,
                )
                self.assertEqual(
                    run(workspace, sys.executable, "scripts/contract_check.py").returncode,
                    0,
                )
            for workspace in (
                temporary_root / "fix-nextjs-route-label/before",
                temporary_root / "fix-nextjs-route-label/after",
            ):
                self.assertEqual(
                    run(workspace, "node", "scripts/regression.mjs").returncode, 0
                )


def a15_contract() -> dict:
    eligible = [
        f"{cell}-task-{task:02d}--pair-{sample:02d}"
        for cell in ("cell-05", "cell-07", "cell-13", "cell-14")
        for task in range(1, 11)
        for sample in range(1, 4)
    ]
    return {
        "full_experiment": {
            "effect_claim_allowed": True,
            "eligible_pair_ids": eligible,
            "sentinel_pair_ids": [],
            "eligible_cell_ids": ["cell-05", "cell-07", "cell-13", "cell-14"],
            "minimum_clusters_per_cell": 10,
            "pairs_per_eligible_cluster": 3,
            "minimum_executed_recovery_pairs": 40,
            "minimum_executed_recovery_pairs_per_profile": 5,
            "bootstrap_samples": 2000,
            "bootstrap_seed": 3991515,
            "primary_estimand": "four-profile paired effect",
            "profile_cells": {
                "cell-05": "cli",
                "cell-07": "generic",
                "cell-13": "data",
                "cell-14": "nextjs",
            },
            "resource_budgets": {
                "wall_time_ms": {"p50": 240000, "p95": 600000},
                "total_tokens": {"p50": 60000, "p95": 120000},
            },
        }
    }


def a15_records(contract: dict) -> list[dict]:
    records = []
    for pair_id in contract["full_experiment"]["eligible_pair_ids"]:
        cluster, sample = pair_id.rsplit("--pair-", 1)
        cell_id = cluster.split("-task-", 1)[0]
        records.append(
            {
                "pair_id": pair_id,
                "cell_id": cell_id,
                "source_task_id": cluster,
                "sample_index": int(sample),
                "eligibility": {
                    "preregistered": {"eligible": True},
                    "runtime": {"category": "recoverable_candidate"},
                },
                "comparison": {
                    "quality_transition": "improved",
                    "executed_recovery_runs": 1,
                    "regression_introduced": False,
                    "resource_delta": {
                        "wall_time_ms": 100000,
                        "total_tokens": 40000,
                    },
                },
            }
        )
    return records


class GoalVerifyRecoveryA15ReportTest(unittest.TestCase):
    @patch(
        "eval_lib.goal_verify_recovery_a15_report.build_recovery_report",
        return_value={"instrument_ready": True},
    )
    def test_smoke_requires_recovery_and_usable_oracles_in_each_profile(self, _base):
        contract = {
            "smoke": {
                "required_real_profiles": ["cli", "generic", "data", "nextjs"],
                "minimum_pairs_per_real_profile": 3,
                "minimum_executed_recovery_pairs_per_real_profile": 1,
            }
        }
        records = []
        for profile in contract["smoke"]["required_real_profiles"]:
            for sample in range(1, 4):
                records.append(
                    {
                        "pair_id": f"{profile}-{sample}",
                        "profile": profile,
                        "eligibility": {
                            "preregistered": {"eligible": True},
                            "runtime": {"category": "recoverable_candidate"},
                        },
                        "comparison": {
                            "quality_transition": "improved",
                            "executed_recovery_runs": 1 if sample == 1 else 0,
                        },
                    }
                )
        report = build_recovery_a15_smoke_report(
            records=records, contract=contract
        )
        self.assertEqual(report["go_no_go"], "GO")
        self.assertTrue(all(report["a15_profile_smoke_checks"].values()))

        no_next_recovery = copy.deepcopy(records)
        for record in no_next_recovery:
            if record["profile"] == "nextjs":
                record["comparison"]["executed_recovery_runs"] = 0
        failed = build_recovery_a15_smoke_report(
            records=no_next_recovery, contract=contract
        )
        self.assertEqual(failed["go_no_go"], "NO-GO")
        self.assertFalse(
            failed["a15_profile_smoke_checks"][
                "recovery_executed_in_every_real_profile"
            ]
        )

    @patch(
        "eval_lib.goal_verify_recovery_a15_report.build_recovery_full_report",
        return_value={
            "instrument_ready": True,
            "effect_attribution_ready": True,
            "effect_claim_ready": True,
        },
    )
    def test_all_profiles_must_have_positive_ci_and_usable_records(self, _base):
        contract = a15_contract()
        records = a15_records(contract)
        report = build_recovery_a15_full_report(records=records, contract=contract)
        self.assertEqual(report["go_no_go"], "GO")
        self.assertTrue(report["all_profiles_quality_improved_claim_ready"])
        self.assertTrue(all(report["a15_profile_checks"].values()))
        self.assertEqual(set(report["profile_effects"]), {"cli", "generic", "data", "nextjs"})

        no_data_gain = copy.deepcopy(records)
        for record in no_data_gain:
            if record["cell_id"] == "cell-13":
                record["comparison"]["quality_transition"] = "unchanged_fail"
        no_gain_report = build_recovery_a15_full_report(
            records=no_data_gain, contract=contract
        )
        self.assertEqual(no_gain_report["go_no_go"], "NO-GO")
        self.assertFalse(
            no_gain_report["profile_effects"]["data"]["ci_lower_above_zero"]
        )

        unusable = copy.deepcopy(records)
        unusable[0]["eligibility"]["runtime"]["category"] = (
            "instrumentation_unavailable"
        )
        unusable_report = build_recovery_a15_full_report(
            records=unusable, contract=contract
        )
        self.assertFalse(
            unusable_report["a15_profile_checks"]["instrumentation_unusable_zero"]
        )
        self.assertEqual(unusable_report["go_no_go"], "NO-GO")


if __name__ == "__main__":
    unittest.main()
