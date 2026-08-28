from __future__ import annotations

import copy
import json
import tempfile
import unittest
from pathlib import Path

from eval_lib.generate_goal_verify_main_v4 import _tracked_files
from eval_lib.goal_verify_baseline_product_v3 import _product_resource_usage
from eval_lib.goal_verify_blind_v4 import (
    build_blind_report,
    canonical_sha256,
    human_sample,
)
from eval_lib.goal_verify_live_v4 import _candidate_resource_usage, _cluster_manifest
from eval_lib.goal_verify_main_design_v4 import main_design_errors
from eval_lib.goal_verify_main_report_v4 import (
    build_main_report,
    build_main_smoke_report,
    evaluate_main_semantic_review,
)

ROOT = Path(__file__).resolve().parents[2]


def load(relative: str):
    return json.loads((ROOT / relative).read_text(encoding="utf-8"))


class GoalVerifyMainV4Test(unittest.TestCase):
    def test_generated_workspace_file_list_ignores_runtime_caches(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            stage = root / "before"
            (stage / "__pycache__").mkdir(parents=True)
            (stage / ".pytest_cache").mkdir()
            (stage / "keep.py").write_text("pass\n", encoding="utf-8")
            (stage / "__pycache__/keep.pyc").write_bytes(b"cache")
            (stage / ".pytest_cache/state").write_text("cache", encoding="utf-8")
            (stage / ".DS_Store").write_bytes(b"cache")
            self.assertEqual(_tracked_files(root, "before"), ["before/keep.py"])

    def test_product_resource_usage_is_frozen_from_terminal_time_profile(self):
        with tempfile.TemporaryDirectory() as temporary:
            run_dir = Path(temporary)
            rows = [
                {
                    "event": "time_profile",
                    "profile": {
                        "total_ms": 1234,
                        "prompt_eval_count": 40,
                        "eval_count": 2,
                    },
                }
            ]
            (run_dir / "events.jsonl").write_text(
                "\n".join(json.dumps(row) for row in rows) + "\n",
                encoding="utf-8",
            )
            self.assertEqual(
                _product_resource_usage(run_dir),
                {
                    "wall_time_ms": 1234,
                    "input_tokens": 40,
                    "output_tokens": 2,
                    "total_tokens": 42,
                },
            )

    def test_candidate_resource_usage_covers_full_lane_wall_and_all_attempt_tokens(
        self,
    ):
        usage = _candidate_resource_usage(
            attempts=[
                {"response": {"response": {"prompt_eval_count": 10, "eval_count": 2}}},
                {"response": {"response": {"prompt_eval_count": 20, "eval_count": 3}}},
            ],
            wall_time_ns=9_500_000,
        )
        self.assertEqual(usage["wall_time_ms"], 9.5)
        self.assertEqual(usage["total_tokens"], 35)
        self.assertTrue(usage["token_measurement_complete"])

    def test_generated_main_design_is_12_by_10_by_3(self):
        corpus = load("eval/goal_verify/v0/phase6-main-corpus-v4.json")
        contract = load("eval/goal_verify/v0/phase6-main-v4-contract.json")
        matrix = load("eval/goal_verify/v0/phase6-matrix.json")
        self.assertEqual(
            main_design_errors(corpus=corpus, contract=contract, matrix=matrix), []
        )
        self.assertEqual(len(corpus["cases"]), 120)
        self.assertEqual(len(contract["selected_cells"]), 120)
        self.assertEqual(contract["samples_per_cell"], 3)

    def test_main_design_rejects_pseudoreplicated_task_binding(self):
        corpus = load("eval/goal_verify/v0/phase6-main-corpus-v4.json")
        contract = load("eval/goal_verify/v0/phase6-main-v4-contract.json")
        matrix = load("eval/goal_verify/v0/phase6-matrix.json")
        corpus["cases"][1]["goal"] = corpus["cases"][0]["goal"]
        errors = main_design_errors(
            corpus=corpus,
            contract=contract,
            matrix=matrix,
        )
        self.assertIn("main_cell_goals_not_distinct:cell-01", errors)

    def test_main_cluster_manifest_distinguishes_smoke_from_population(self):
        corpus = load("eval/goal_verify/v0/phase6-main-corpus-v4.json")
        selected = corpus["cases"]
        smoke_pairs = [
            f"{case['case_id']}--pair-01"
            for case in selected
            if case["case_id"].endswith("-task-01")
        ]
        manifest = _cluster_manifest(
            selected=selected,
            samples_per_task=3,
            selected_pair_ids=smoke_pairs,
        )["cluster_design"]
        self.assertEqual(manifest["population_cell_count"], 12)
        self.assertEqual(manifest["population_source_task_count"], 120)
        self.assertEqual(manifest["population_pair_count"], 360)
        self.assertEqual(manifest["selected_cell_count"], 12)
        self.assertEqual(manifest["selected_source_task_count"], 12)
        self.assertEqual(manifest["selected_pair_count"], 12)

    def test_main_report_uses_cluster_bootstrap_and_fixed_budgets(self):
        contract = load("eval/goal_verify/v0/phase6-main-v4-contract.json")
        contract = copy.deepcopy(contract)
        contract["main_analysis"]["bootstrap_samples"] = 20
        config = load("eval/goal_verify/v0/baseline-config.json")
        records = _synthetic_records()
        report = build_main_report(
            contract=contract,
            config=config,
            records=records,
            semantic_review_complete=True,
            semantic_review_evaluation={"pass": True},
        )
        self.assertEqual(report["final_decision"], "GO")
        self.assertTrue(report["checks"]["cluster_design"])
        primary = report["lane_reports"]["held_out_synthesis"]
        self.assertEqual(
            primary["bootstrap"]["strong_binding"]["method"],
            "hierarchical_cluster_paired_percentile",
        )
        self.assertEqual(
            primary["resources"]["percentiles"]["p50_wall_time_increase_pct"],
            7.0,
        )
        records[0].pop("source_task_id")
        no_go = build_main_report(
            contract=contract,
            config=config,
            records=records,
            semantic_review_complete=True,
            semantic_review_evaluation={"pass": True},
        )
        self.assertEqual(no_go["final_decision"], "NO-GO")
        self.assertFalse(no_go["checks"]["cluster_design"])

    def test_main_semantic_review_safety_rule_is_authoritative(self):
        contract = load("eval/goal_verify/v0/phase6-main-v4-contract.json")
        review = {
            "review_count": 36,
            "verdict_counts": {
                "acceptable": 30,
                "needs_revision": 6,
                "unusable": 0,
            },
            "axis_pass_counts": {
                "false_positive_or_overconstraint_risk_acceptable": 36,
            },
        }
        passed = evaluate_main_semantic_review(
            contract=contract,
            blind_report={"calibration_review": review},
            semantic_review_complete=True,
        )
        self.assertTrue(passed["pass"])
        review["verdict_counts"]["unusable"] = 1
        failed = evaluate_main_semantic_review(
            contract=contract,
            blind_report={"calibration_review": review},
            semantic_review_complete=True,
        )
        self.assertFalse(failed["pass"])
        self.assertFalse(failed["checks"]["unusable_count"])

    def test_main_smoke_report_checks_frozen_instrument_only(self):
        contract = load("eval/goal_verify/v0/phase6-main-v4-contract.json")
        expected_pair_ids = contract["smoke"]["pair_ids"]
        records = [
            record
            for record in _synthetic_records()
            if record["source_task_id"].endswith("-01")
            and record["pair_id"].endswith("-r01")
        ]
        for record, pair_id in zip(records, expected_pair_ids, strict=True):
            record["pair_id"] = pair_id
            record["snapshot_manifests"] = {"product": {"snapshot_sha256": "a" * 64}}
            record["baseline"].update(
                {
                    "completion_contract_bound": True,
                    "completion_verify_attempt_recorded": True,
                    "product_run_dir": "/frozen/product/run",
                    "recovery_plan_auto_runs": 0,
                }
            )
            for lane in record["lanes"].values():
                lane["validation"].update(
                    {"valid_before_host_repairs": True, "host_repairs": []}
                )
        manifest = {
            "campaign_role": "preregistered_smoke",
            "request_namespace": contract["smoke"]["request_namespace"],
            "selected_pair_ids": expected_pair_ids,
            "target_pairs": len(expected_pair_ids),
            "cluster_design": {
                "cluster_unit": "source_task_id",
                "population_cell_count": 12,
                "population_source_task_count": 120,
                "runs_per_source_task": 3,
                "population_pair_count": 360,
                "selected_cell_count": 12,
                "selected_source_task_count": 12,
                "selected_pair_count": 12,
            },
        }
        report = build_main_smoke_report(
            contract=contract,
            records=records,
            manifest=manifest,
        )
        self.assertEqual(report["final_decision"], "GO")
        self.assertTrue(report["checks"]["resource_measurement_complete"])
        records[0]["baseline"]["recovery_plan_auto_runs"] = 1
        no_go = build_main_smoke_report(
            contract=contract,
            records=records,
            manifest=manifest,
        )
        self.assertEqual(no_go["final_decision"], "NO-GO")
        self.assertFalse(no_go["checks"]["recovery_plan_auto_runs_zero"])

    def test_main_review_selects_three_distinct_tasks_per_cell(self):
        items = []
        mapping = {}
        for cell in range(1, 13):
            for task in range(1, 11):
                item_id = f"item-{cell:02d}-{task:02d}"
                items.append({"item_id": item_id})
                mapping[item_id] = {
                    "source_case_id": f"case-{cell:02d}-{task:02d}",
                    "source_lane": "held_out_synthesis",
                    "cell_id": f"cell-{cell:02d}",
                    "source_task_id": f"task-{cell:02d}-{task:02d}",
                }
        selected = human_sample(
            items=items,
            mapping=mapping,
            sample_spec={
                "size": 36,
                "items_per_cell": 3,
                "source_lane": "held_out_synthesis",
            },
        )
        self.assertEqual(len(selected), 36)
        self.assertEqual(len(set(selected)), 36)

    def test_main_review_does_not_require_inadmissible_model_evidence(self):
        item = {
            "item_id": "item-1",
            "item_sha256": "a" * 64,
            "goal": "g",
            "intent": "create",
            "profile": "cli",
            "required_claims": [],
            "group_kind": "empty_proposal",
            "raw_claim": None,
            "raw_oracles": [],
        }
        document = {
            "items_sha256": canonical_sha256([item]),
            "human_items_sha256": canonical_sha256([item]),
            "reviewer_id": "human",
            "reviewer_type": "human",
            "contract_authoring_involvement": False,
            "independence_confirmed": True,
            "item_ids": ["item-1"],
            "reviews": [
                {
                    "item_id": "item-1",
                    "verdict": "unusable",
                    "axes": {
                        "requirement_clear": False,
                        "input_observation_expected_specific": False,
                        "executable_from_visible_information": False,
                        "false_positive_or_overconstraint_risk_acceptable": False,
                        "semantic_duplication_absent": True,
                    },
                    "reason_codes": ["empty"],
                    "rationale": "No proposal was produced.",
                }
            ],
        }
        report = build_blind_report(
            items=[item],
            model_documents=[],
            human_document=document,
            human_items=[item],
            review_contract={
                "model_reviews_required": False,
                "main_sample": {"size": 1},
            },
        )
        self.assertTrue(report["semantic_review_complete"])
        self.assertFalse(report["model_reviews_required"])


def _synthetic_records():
    records = []
    for cell in range(1, 13):
        for task in range(1, 11):
            for run in range(1, 4):
                lanes = {}
                for lane_name in ("contract_conformance", "held_out_synthesis"):
                    lanes[lane_name] = {
                        "validation": {"valid": True},
                        "execution": {
                            "same_snapshot": True,
                            "reference_fallback_count": 0,
                            "gold_used_for_execution_count": 0,
                        },
                        "attempts": [
                            {
                                "response": {
                                    "response": {
                                        "client_wall_time_ns": 5_000_000,
                                        "prompt_eval_count": 3,
                                        "eval_count": 2,
                                    }
                                }
                            }
                        ],
                        "resource_usage": {
                            "wall_time_ms": 7.0,
                            "input_tokens": 4,
                            "output_tokens": 2,
                            "total_tokens": 6,
                            "token_measurement_complete": True,
                        },
                        "additive_comparison": {
                            "baseline_failure_overridden": False,
                            "shadow_verdict": "failure",
                            "combined_score": {
                                "claims": [{"status": "strong"}],
                            },
                            "paired_delta": {
                                "required_claim_recall": 0.20,
                                "strong_binding": 0.20,
                                "unverified_rate": -0.20,
                            },
                        },
                    }
                records.append(
                    {
                        "pair_id": f"c{cell:02d}-t{task:02d}-r{run:02d}",
                        "cell_id": f"cell-{cell:02d}",
                        "source_task_id": f"task-{cell:02d}-{task:02d}",
                        "baseline": {
                            "resource_usage": {
                                "wall_time_ms": 100,
                                "total_tokens": 100,
                            }
                        },
                        "lanes": lanes,
                    }
                )
    return records


if __name__ == "__main__":
    unittest.main()
