from __future__ import annotations

import copy
import json
import tempfile
import unittest
from pathlib import Path

from eval_lib.generate_goal_verify_main_v4 import _tracked_files
from eval_lib.generate_goal_verify_main_v4_a13 import (
    _build_adapters as build_a13_adapters,
)
from eval_lib.generate_goal_verify_main_v4_a13 import (
    _build_contract as build_a13_contract,
)
from eval_lib.generate_goal_verify_main_v4_a13 import _build_tasks as build_a13_tasks
from eval_lib.goal_verify_baseline_product_v3 import _product_resource_usage
from eval_lib.goal_verify_blind_v4 import (
    build_blind_report,
    canonical_sha256,
    human_sample,
)
from eval_lib.goal_verify_live_v4 import _candidate_resource_usage, _cluster_manifest
from eval_lib.goal_verify_main_design_v4 import main_design_errors
from eval_lib.goal_verify_main_report_v4 import (
    _a13_instrument_checks,
    build_main_report,
    build_main_smoke_report,
    evaluate_main_semantic_review,
)
from eval_lib.goal_verify_resource_diagnostics_v4 import build_resource_diagnostics

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
            phase_timings_ns={
                "prompt_assembly": 500_000,
                "provider_request": 8_000_000,
            },
        )
        self.assertEqual(usage["wall_time_ms"], 9.5)
        self.assertEqual(usage["total_tokens"], 35)
        self.assertTrue(usage["token_measurement_complete"])
        self.assertEqual(
            usage["phase_timings_ms"],
            {"prompt_assembly": 0.5, "provider_request": 8.0},
        )
        self.assertEqual(usage["phase_timing_residual_ms"], 1.0)

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

    def test_a13_draft_preserves_thresholds_and_fails_closed(self):
        base = load("eval/goal_verify/v0/phase6-main-v4-contract.json")
        contract = build_a13_contract(
            status="draft",
            code_sha="",
            exact_sha_ci_evidence="",
            live_collection_authorized=False,
        )
        self.assertEqual(contract["status"], "draft")
        self.assertFalse(contract["authorization"]["live_collection_authorized"])
        self.assertEqual(contract["resource_budgets"], base["resource_budgets"])
        self.assertEqual(
            contract["main_analysis"]["threshold_mapping"],
            base["main_analysis"]["threshold_mapping"],
        )
        self.assertEqual(contract["full_experiment"], base["full_experiment"])

        adapters = build_a13_adapters(status="draft")["adapters"]
        investigation = [
            row for row in adapters if row["case_id"].startswith("phase6-main-c09-")
        ]
        self.assertTrue(investigation)
        self.assertTrue(
            all(row["executor"]["kind"] == "unavailable" for row in investigation)
        )

        tasks = build_a13_tasks(status="draft")["cases"]
        self.assertTrue(
            all(row["completion_contract"]["goal"] == row["goal"] for row in tasks)
        )

    def test_a13_instrument_checks_bind_policy_and_phase_timing(self):
        contract = build_a13_contract(
            status="draft",
            code_sha="",
            exact_sha_ci_evidence="",
            live_collection_authorized=False,
        )
        policy_sha = contract["semantic_oracle_policy"]["sha256"]
        phases = {
            phase: 0.0
            for phase in (
                "prompt_assembly",
                "provider_request",
                "raw_schema_validation",
                "canonicalization",
                "proposal_validation",
                "oracle_execution",
                "scoring",
            )
        }
        records = [
            {
                "lanes": {
                    "held_out_synthesis": {
                        "validation": {"valid": True},
                        "execution": {
                            "semantic_policy_sha256": policy_sha,
                            "execution_policy_source": "candidate_visible_prompt",
                            "evaluations": [
                                {
                                    "classification": "semantic_rejected",
                                    "execution_attempt_recorded": False,
                                    "executed": False,
                                    "result": "unverified",
                                    "semantic_policy_sha256": policy_sha,
                                }
                            ],
                        },
                        "resource_usage": {
                            "phase_timings_ms": phases,
                            "phase_timing_residual_ms": 0.1,
                        },
                    }
                }
            }
        ]

        checks = _a13_instrument_checks(contract=contract, records=records)
        self.assertTrue(all(checks.values()), checks)
        records[0]["lanes"]["held_out_synthesis"]["execution"]["evaluations"][0][
            "executed"
        ] = True
        checks = _a13_instrument_checks(contract=contract, records=records)
        self.assertFalse(checks["semantic_rejected_not_executed"])

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
        self.assertEqual(
            primary["resources"]["diagnostics"]["record_count"],
            len(records),
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

    def test_resource_diagnostics_separates_coverage_ratios_and_tails(self):
        records = [
            _resource_record(
                pair_id="pair-01",
                cell_id="cell-01",
                baseline_wall_ms=100,
                baseline_tokens=100,
                candidate_wall_ms=50,
                candidate_input_tokens=80,
                candidate_output_tokens=20,
                provider_client_ms=40,
                provider_prompt_ms=10,
                provider_output_ms=30,
                evaluations=[{"executed": True, "runtime_ms": 5}],
            ),
            _resource_record(
                pair_id="pair-02",
                cell_id="cell-02",
                baseline_wall_ms=200,
                baseline_tokens=200,
                candidate_wall_ms=20,
                candidate_input_tokens=30,
                candidate_output_tokens=10,
                provider_client_ms=10,
                provider_prompt_ms=2,
                provider_output_ms=8,
                evaluations=[{"executed": True}, {"executed": False}],
            ),
        ]
        diagnostics = build_resource_diagnostics(
            records=records,
            lane_name="held_out_synthesis",
        )
        self.assertTrue(diagnostics["provider_timing"]["complete"])
        self.assertEqual(diagnostics["provider_timing"]["attempt_count"], 2)
        runtime = diagnostics["oracle_runtime"]
        self.assertEqual(runtime["evaluation_count"], 3)
        self.assertEqual(runtime["runtime_recorded_count"], 1)
        self.assertEqual(runtime["executed_count"], 2)
        self.assertEqual(runtime["executed_runtime_missing_count"], 1)
        self.assertEqual(runtime["unexecuted_count"], 1)
        self.assertEqual(diagnostics["attribution"]["residual_ms"]["p50"], 5.0)
        self.assertFalse(diagnostics["candidate_phase_timing"]["complete"])
        self.assertEqual(diagnostics["candidate_phase_timing"]["recorded_count"], 0)
        self.assertEqual(
            diagnostics["attribution"]["overall_input_share_of_candidate_tokens_pct"],
            78.571429,
        )
        self.assertEqual(
            diagnostics["cell_median_increase_pct"]["cell-01"],
            {
                "wall_time_increase_pct": 50.0,
                "total_tokens_increase_pct": 100.0,
            },
        )
        self.assertEqual(
            diagnostics["tails"]["wall_increase_ratio_top_5pct"]["cell_counts"],
            {"cell-01": 1},
        )
        self.assertEqual(
            diagnostics["tails"]["candidate_absolute_wall_top_5pct"]["cell_counts"],
            {"cell-01": 1},
        )

    def test_resource_diagnostics_reports_incomplete_provider_timing(self):
        record = _resource_record(
            pair_id="pair-01",
            cell_id="cell-01",
            baseline_wall_ms=100,
            baseline_tokens=100,
            candidate_wall_ms=50,
            candidate_input_tokens=80,
            candidate_output_tokens=20,
            provider_client_ms=40,
            provider_prompt_ms=10,
            provider_output_ms=30,
            evaluations=[],
        )
        del record["lanes"]["held_out_synthesis"]["attempts"][0]["response"][
            "response"
        ]["eval_duration"]
        diagnostics = build_resource_diagnostics(
            records=[record],
            lane_name="held_out_synthesis",
        )
        self.assertFalse(diagnostics["provider_timing"]["complete"])
        self.assertEqual(diagnostics["provider_timing"]["complete_attempt_count"], 0)
        self.assertEqual(diagnostics["provider_timing"]["output_eval_ms"]["count"], 0)
        self.assertEqual(diagnostics["attribution"]["residual_ms"]["p50"], 10.0)

    def test_resource_diagnostics_reports_candidate_phase_timing(self):
        record = _resource_record(
            pair_id="pair-01",
            cell_id="cell-01",
            baseline_wall_ms=100,
            baseline_tokens=100,
            candidate_wall_ms=50,
            candidate_input_tokens=80,
            candidate_output_tokens=20,
            provider_client_ms=40,
            provider_prompt_ms=10,
            provider_output_ms=30,
            evaluations=[],
            phase_timings_ms={
                "prompt_assembly": 2.0,
                "provider_request": 40.0,
            },
            phase_timing_residual_ms=8.0,
        )
        diagnostics = build_resource_diagnostics(
            records=[record], lane_name="held_out_synthesis"
        )
        timing = diagnostics["candidate_phase_timing"]
        self.assertTrue(timing["complete"])
        self.assertEqual(timing["phases_ms"]["provider_request"]["p50"], 40.0)
        self.assertEqual(timing["instrumentation_residual_ms"]["p50"], 8.0)

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


def _resource_record(
    *,
    pair_id: str,
    cell_id: str,
    baseline_wall_ms: int,
    baseline_tokens: int,
    candidate_wall_ms: int,
    candidate_input_tokens: int,
    candidate_output_tokens: int,
    provider_client_ms: int,
    provider_prompt_ms: int,
    provider_output_ms: int,
    evaluations: list[dict],
    phase_timings_ms: dict[str, float] | None = None,
    phase_timing_residual_ms: float | None = None,
):
    record = {
        "pair_id": pair_id,
        "cell_id": cell_id,
        "baseline": {
            "resource_usage": {
                "wall_time_ms": baseline_wall_ms,
                "total_tokens": baseline_tokens,
            }
        },
        "lanes": {
            "held_out_synthesis": {
                "attempts": [
                    {
                        "response": {
                            "response": {
                                "client_wall_time_ns": provider_client_ms * 1_000_000,
                                "prompt_eval_duration": provider_prompt_ms * 1_000_000,
                                "eval_duration": provider_output_ms * 1_000_000,
                            }
                        }
                    }
                ],
                "resource_usage": {
                    "wall_time_ms": candidate_wall_ms,
                    "input_tokens": candidate_input_tokens,
                    "output_tokens": candidate_output_tokens,
                    "total_tokens": candidate_input_tokens + candidate_output_tokens,
                },
                "execution": {"evaluations": evaluations},
            }
        },
    }
    if phase_timings_ms is not None:
        usage = record["lanes"]["held_out_synthesis"]["resource_usage"]
        usage["phase_timings_ms"] = phase_timings_ms
        usage["phase_timing_residual_ms"] = phase_timing_residual_ms
    return record


if __name__ == "__main__":
    unittest.main()
