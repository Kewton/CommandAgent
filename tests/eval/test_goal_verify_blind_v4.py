import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from eval_lib.goal_verify_blind_v4 import (
    AXES,
    authorized_ai_reviewer_template,
    build_blind_report,
    canonical_sha256,
    human_sample,
    independent_human_template,
    prepare_semantic_items,
    validate_model_review,
)


def record(pair_id="case-a--pair-01", case_id="case-a"):
    raw = """{
      "generation":{"provider":"secret","model":"secret","request_id":"secret"},
      "claims":[{"id":"c1","normalized_requirement":"does it","oracle_ids":["o1"]}],
      "oracles":[{"id":"o1","claim_id":"c1","strategy":"command"}]
    }"""
    lane = {
        "attempts": [
            {"response": {"status": "completed", "response": {"response": raw}}}
        ]
    }
    return {
        "pair_id": pair_id,
        "source_case_id": case_id,
        "goal": "do it",
        "intent": "create",
        "profile": "cli",
        "required_claims": [{"id": "required", "min_strength": "runtime"}],
        "lanes": {"contract_conformance": lane, "held_out_synthesis": lane},
    }


def record_with_raw(raw, pair_id="case-a--pair-01", case_id="case-a"):
    lane = {
        "attempts": [
            {"response": {"status": "completed", "response": {"response": raw}}}
        ]
    }
    value = record(pair_id, case_id)
    value["lanes"] = {"contract_conformance": lane, "held_out_synthesis": lane}
    return value


class BlindV4Test(unittest.TestCase):
    def test_items_keep_raw_semantics_and_hide_source_and_provider(self):
        items, mapping = prepare_semantic_items(
            records=[record()], contract_sha256="a" * 64
        )
        self.assertEqual(len(items), 2)
        self.assertEqual(items[0]["raw_claim"]["normalized_requirement"], "does it")
        self.assertNotIn("source_lane", items[0])
        self.assertNotIn("pair_id", items[0])
        self.assertNotIn("generation", str(items))
        self.assertEqual(
            {row["source_lane"] for row in mapping.values()},
            {
                "contract_conformance",
                "held_out_synthesis",
            },
        )

    def test_item_order_and_ids_are_deterministic(self):
        first = prepare_semantic_items(records=[record()], contract_sha256="a" * 64)
        second = prepare_semantic_items(records=[record()], contract_sha256="a" * 64)
        self.assertEqual(first, second)

    def test_raw_oracle_without_host_id_is_not_duplicated_as_orphan(self):
        raw = """{
          "claims":[{"id":"c1","normalized_requirement":"does it"}],
          "oracles":[{"claim_id":"c1","strategy":"command"}]
        }"""
        items, mapping = prepare_semantic_items(
            records=[record_with_raw(raw)], contract_sha256="1" * 64
        )
        self.assertEqual(len(items), 2)
        self.assertTrue(all(item["group_kind"] == "claim_group" for item in items))
        self.assertTrue(all(len(item["raw_oracles"]) == 1 for item in items))
        self.assertTrue(
            all(row["source_oracle_indexes"] == [0] for row in mapping.values())
        )

    def test_only_unmatched_raw_oracle_is_an_orphan(self):
        raw = """{
          "claims":[{"id":"c1","normalized_requirement":"does it"}],
          "oracles":[{"claim_id":"missing","strategy":"command"}]
        }"""
        items, mapping = prepare_semantic_items(
            records=[record_with_raw(raw)], contract_sha256="2" * 64
        )
        self.assertEqual(len(items), 4)
        self.assertEqual(sum(item["group_kind"] == "claim_group" for item in items), 2)
        self.assertEqual(
            sum(item["group_kind"] == "orphan_oracle" for item in items), 2
        )
        orphan_mapping = [
            mapping[item["item_id"]]
            for item in items
            if item["group_kind"] == "orphan_oracle"
        ]
        self.assertTrue(
            all(row["source_oracle_indexes"] == [0] for row in orphan_mapping)
        )

    def test_human_sample_covers_cases_and_has_ten_items(self):
        records = []
        for index in range(1, 9):
            records.append(record(f"case-{index}--pair-01", f"case-{index}"))
        items, mapping = prepare_semantic_items(
            records=records, contract_sha256="b" * 64
        )
        sample = human_sample(items=items, mapping=mapping)
        self.assertEqual(len(sample), 10)
        covered = {mapping[item_id]["source_case_id"] for item_id in sample}
        self.assertEqual(len(covered), 8)

    def test_report_accepts_nested_and_flat_review_payloads(self):
        items, mapping = prepare_semantic_items(
            records=[
                record(f"case-{index}--pair-01", f"case-{index}") for index in range(8)
            ],
            contract_sha256="c" * 64,
        )
        sample = human_sample(items=items, mapping=mapping)
        sampled_items = [
            next(item for item in items if item["item_id"] == item_id)
            for item_id in sample
        ]
        rows = [review_row(item["item_id"]) for item in items]
        first = model_review(items, rows, "family-a", nested=True)
        first["reviewer"]["provider"] = "openai-codex-agent"
        first["invocation_script_sha256"] = "not_applicable_agent_review"
        second = model_review(items, rows, "family-b", nested=False)
        human = {
            "reviewer_id": "fable",
            "reviewer_type": "human",
            "contract_authoring_involvement": False,
            "independence_confirmed": True,
            "items_sha256": canonical_sha256(items),
            "human_items_sha256": canonical_sha256(sampled_items),
            "item_ids": sample,
            "reviews": [review_row(item_id) for item_id in sample],
        }
        report = build_blind_report(
            items=items,
            model_documents=[first, second],
            human_document=human,
            human_items=sampled_items,
        )
        self.assertTrue(report["semantic_review_complete"])
        self.assertEqual(report["agreement"]["cohen_kappa"], 1.0)

    def test_model_review_accepts_prepare_template_reviews_key(self):
        items, _ = prepare_semantic_items(records=[record()], contract_sha256="3" * 64)
        document = {
            "items_sha256": canonical_sha256(items),
            "reviewer": {
                "provider": "test",
                "model_id_or_version": "family-a-model",
                "model_family": "family-a",
                "invoked_at": "2026-08-28T00:00:00+09:00",
                "independent": True,
            },
            "reviews": [review_row(item["item_id"]) for item in items],
            "invocation_script_sha256": "a" * 64,
        }
        validated = validate_model_review(
            document=document,
            expected_item_ids=[item["item_id"] for item in items],
            items_sha256=canonical_sha256(items),
        )
        self.assertTrue(validated["valid"])
        self.assertEqual(validated["review_count"], len(items))

    def test_report_keeps_blank_human_review_incomplete(self):
        items, mapping = prepare_semantic_items(
            records=[
                record(f"case-{index}--pair-01", f"case-{index}") for index in range(8)
            ],
            contract_sha256="d" * 64,
        )
        sample = human_sample(items=items, mapping=mapping)
        sampled_items = [
            next(item for item in items if item["item_id"] == item_id)
            for item_id in sample
        ]
        rows = [review_row(item["item_id"]) for item in items]
        human_rows = [review_row(item_id) for item_id in sample]
        for row in human_rows:
            row["verdict"] = ""
            row["rationale"] = ""
        report = build_blind_report(
            items=items,
            model_documents=[
                model_review(items, rows, "family-a", nested=True),
                model_review(items, rows, "family-b", nested=False),
            ],
            human_document={
                "reviewer_id": "fable",
                "reviewer_type": "human",
                "contract_authoring_involvement": False,
                "independence_confirmed": True,
                "items_sha256": canonical_sha256(items),
                "human_items_sha256": canonical_sha256(sampled_items),
                "item_ids": sample,
                "reviews": human_rows,
            },
            human_items=sampled_items,
        )
        self.assertFalse(report["semantic_review_complete"])
        self.assertIn("invalid_verdict", report["human_review"]["errors"][0])

    def test_non_human_or_contract_author_is_not_an_independent_human(self):
        items, mapping = prepare_semantic_items(
            records=[
                record(f"case-{index}--pair-01", f"case-{index}") for index in range(8)
            ],
            contract_sha256="e" * 64,
        )
        sample = human_sample(items=items, mapping=mapping)
        sampled_items = [
            next(item for item in items if item["item_id"] == item_id)
            for item_id in sample
        ]
        human = independent_human_template(
            items_sha256=canonical_sha256(items), human_items=sampled_items
        )
        human.update(
            {
                "reviewer_id": "fable",
                "reviewer_type": "model",
                "contract_authoring_involvement": True,
                "independence_confirmed": True,
                "reviews": [review_row(item_id) for item_id in sample],
            }
        )
        rows = [review_row(item["item_id"]) for item in items]
        report = build_blind_report(
            items=items,
            model_documents=[
                model_review(items, rows, "family-a", nested=True),
                model_review(items, rows, "family-b", nested=False),
            ],
            human_document=human,
            human_items=sampled_items,
        )
        self.assertFalse(report["semantic_review_complete"])
        self.assertIn("reviewer_type_must_be_human", report["human_review"]["errors"])
        self.assertIn(
            "contract_authoring_involvement_must_be_false",
            report["human_review"]["errors"],
        )

    def test_user_authorized_ai_can_complete_calibration_review(self):
        items, mapping = prepare_semantic_items(
            records=[
                record(f"case-{index}--pair-01", f"case-{index}") for index in range(8)
            ],
            contract_sha256="f" * 64,
        )
        sample = human_sample(items=items, mapping=mapping)
        sampled_items = [
            next(item for item in items if item["item_id"] == item_id)
            for item_id in sample
        ]
        policy = authorized_ai_policy()
        calibration = authorized_ai_reviewer_template(
            items_sha256=canonical_sha256(items),
            human_items=sampled_items,
            reviewer_policy=policy,
        )
        calibration.update(
            {
                "source_blind_confirmed": True,
                "forbidden_materials_not_accessed": True,
                "reviewer_output_independence_confirmed": True,
                "invoked_at": "2026-08-28T12:00:00+09:00",
                "reviews": [review_row(item_id) for item_id in sample],
            }
        )
        rows = [review_row(item["item_id"]) for item in items]
        report = build_blind_report(
            items=items,
            model_documents=[
                model_review(items, rows, "family-a", nested=True),
                model_review(items, rows, "family-b", nested=False),
            ],
            human_document=calibration,
            human_items=sampled_items,
            reviewer_policy=policy,
        )
        self.assertTrue(report["semantic_review_complete"])
        self.assertTrue(report["checks"]["calibration_review_complete"])
        self.assertFalse(report["checks"]["human_review_complete"])
        self.assertEqual(report["calibration_review"]["reviewer_type"], "ai")
        self.assertTrue(report["calibration_review"]["contract_authoring_involvement"])

    def test_authorized_ai_must_match_frozen_identity_and_boundary(self):
        items, mapping = prepare_semantic_items(
            records=[
                record(f"case-{index}--pair-01", f"case-{index}") for index in range(8)
            ],
            contract_sha256="9" * 64,
        )
        sample = human_sample(items=items, mapping=mapping)
        sampled_items = [
            next(item for item in items if item["item_id"] == item_id)
            for item_id in sample
        ]
        policy = authorized_ai_policy()
        calibration = authorized_ai_reviewer_template(
            items_sha256=canonical_sha256(items),
            human_items=sampled_items,
            reviewer_policy=policy,
        )
        calibration.update(
            {
                "model_id_or_version": "different-model",
                "source_blind_confirmed": False,
                "forbidden_materials_not_accessed": True,
                "reviewer_output_independence_confirmed": True,
                "invoked_at": "2026-08-28T12:00:00+09:00",
                "reviews": [review_row(item_id) for item_id in sample],
            }
        )
        rows = [review_row(item["item_id"]) for item in items]
        report = build_blind_report(
            items=items,
            model_documents=[
                model_review(items, rows, "family-a", nested=True),
                model_review(items, rows, "family-b", nested=False),
            ],
            human_document=calibration,
            human_items=sampled_items,
            reviewer_policy=policy,
        )
        self.assertFalse(report["semantic_review_complete"])
        self.assertIn(
            "authorized_ai_model_id_or_version_mismatch",
            report["calibration_review"]["errors"],
        )
        self.assertIn(
            "authorized_ai_source_blind_not_confirmed",
            report["calibration_review"]["errors"],
        )


def review_row(item_id):
    return {
        "item_id": item_id,
        "verdict": "acceptable",
        "axes": {axis: True for axis in AXES},
        "reason_codes": [],
        "rationale": "The visible claim and oracle form a complete semantic check.",
    }


def model_review(items, rows, family, *, nested):
    parsed = {"reviews": rows} if nested else rows
    return {
        "items_sha256": canonical_sha256(items),
        "reviewer": {
            "provider": "test",
            "model_id_or_version": family + "-model",
            "model_family": family,
            "invoked_at": "2026-08-27T00:00:00+09:00",
            "independent": True,
        },
        "parsed_reviews": parsed,
        "invocation_script_sha256": "a" * 64,
    }


def authorized_ai_policy():
    return {
        "allowed_reviewer_types": ["human", "ai"],
        "authorized_ai_reviewer": {
            "authorization_id": "issue-399-a12-fable-review",
            "authorization_scope": "A12 source-blind calibration sample only",
            "authorized_at": "2026-08-28",
            "authorized_by": "repository owner",
            "reviewer_id": "fable",
            "provider": "anthropic",
            "model_family": "claude",
            "model_id_or_version": "claude-fable-5",
            "contract_authoring_involvement": True,
        },
    }


if __name__ == "__main__":
    unittest.main()
