"""Bind the added promotion record to the unchanged R0 contract and exact overlays."""

import copy
import json
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from issue448_nextjs_contract import parse_results
from issue448_nextjs_r0 import FIXTURE, assert_provenance, files, materialize, sha256

PROMOTION = FIXTURE.parent / "issue448-nextjs-promotion"


def report():
    return json.loads((PROMOTION / "measured-contract-results.json").read_text())


def test_original_contract_and_corpus_retained_in_every_decision():
    assert_provenance()
    measured = report()
    assert measured["original_corpus_sha256"] == files(FIXTURE)
    contract_path = FIXTURE / "evidence/original-completion-contract.json"
    contract = json.loads(contract_path.read_text())
    for row in measured["results"]:
        assert row["contract_sha256"] == sha256(contract_path)
        for key in ["required_capabilities", "required_evidence", "required_obligations", "verify_commands"]:
            assert row[key] == contract[key]
        assert row["observer"]["observer_id"] == "nextjs_browser_interaction_v1"
        assert row["observer"]["profile"] == "nextjs"
        assert row["observer"]["port"] == 60302
        assert "no browser/model run" in row["kind"]
    parse_results("\n".join("ISSUE448_ORIGINAL_CONTRACT " + json.dumps(r) for r in measured["results"]))


def test_combined_treatment_uses_only_existing_source_repairs(tmp_path):
    expected = materialize({"overlays": ["ui-only", "export", "promise"]}, tmp_path / "combined")
    original = files(FIXTURE / "original")
    actual = files(expected)
    assert actual.keys() == original.keys()
    assert {p for p in actual if actual[p] != original[p]} == {
        "src/app/page.tsx", "src/lib/types.ts", "src/app/api/projects/route.ts", "src/app/api/tasks/route.ts"}
    for row in report()["results"]:
        if row["scenario"] not in ["missing_implementation", "build_failure"]:
            assert row["source_sha256"] == actual
    for relative in actual:
        before = (FIXTURE / "original" / relative).read_text()
        after = (expected / relative).read_text()
        for forbidden in ["any", "@ts-ignore", "@ts-nocheck", "@ts-expect-error", "ignoreBuildErrors", "ignoreDuringBuilds"]:
            assert after.count(forbidden) <= before.count(forbidden)


def test_build_success_does_not_replace_original_observations():
    rows = {r["scenario"]: r for r in report()["results"]}
    positive = rows["all_original_observations_pass"]
    assert positive["commands"].count("npm run build [real] exit=0") == 2
    assert positive["readiness"]["ok"] is True
    assert positive["readiness"]["dev_server"]["child_reaped"] is True
    assert positive["interaction"]["ok"] is True
    assert positive["static_acceptance"]["passed"] is True
    assert set(positive["static_acceptance"]["evidence_tiers"].values()) == {"strong"}
    assert set(positive["static_acceptance"]["evidence_tiers"]) == set(positive["required_evidence"])
    for key in ["missing_capabilities", "missing_evidence", "missing_obligations"]:
        assert positive["static_acceptance"][key] == []
    for scenario in ["failed_interaction", "missing_interaction", "http_failure", "missing_implementation"]:
        row = rows[scenario]
        assert row["build_exit_code"] == 0
        assert row["decision"]["decision"] == "rejected"
        assert row["control_before_sha256"] == row["control_after_sha256"]
    assert rows["missing_interaction"]["interaction"] is None
    assert rows["failed_interaction"]["interaction"]["ok"] is False
    assert rows["missing_implementation"]["interaction"]["ok"] is True
    assert "non_implementation_obligation_only:scaffold" in rows["missing_implementation"]["decision"]["reason"]


@pytest.mark.parametrize("damage", ["missing_case", "failed_positive", "promoted_negative", "changed_control", "unrun_build"])
def test_evidence_parser_rejects_incomplete_or_false_pass(damage):
    rows = copy.deepcopy(report()["results"])
    if damage == "missing_case":
        rows.pop()
    elif damage == "failed_positive":
        rows[0]["decision"]["decision"] = "rejected"
    elif damage == "promoted_negative":
        rows[1]["decision"]["decision"] = "promoted"
    elif damage == "changed_control":
        rows[1]["control_after_sha256"] = "changed"
    else:
        rows[0]["build_mode"] = "measured-replay"
    with pytest.raises(AssertionError):
        parse_results("\n".join("ISSUE448_ORIGINAL_CONTRACT " + json.dumps(r) for r in rows))


def test_added_observation_manifest_and_provenance():
    manifest = json.loads((PROMOTION / "fixture-sha256.json").read_text())
    assert {p: h for p, h in files(PROMOTION).items() if p != "fixture-sha256.json"} == manifest
    provenance = json.loads((PROMOTION / "provenance.json").read_text())
    assert provenance["original_manifest_sha256"] == sha256(FIXTURE / "fixture-sha256.json")
    assert provenance["original_contract_sha256"] == sha256(FIXTURE / "evidence/original-completion-contract.json")
    assert provenance["original_lock_sha256"] == sha256(FIXTURE / "original/package-lock.json")
