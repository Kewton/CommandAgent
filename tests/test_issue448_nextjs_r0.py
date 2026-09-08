"""Integrity and no-suppression checks; real builds run via the explicit matrix CLI."""

import json
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from issue448_nextjs_r0 import FIXTURE, assert_provenance, files, materialize


def test_frozen_r0_provenance():
    assert_provenance()


@pytest.mark.parametrize("case", json.loads((FIXTURE / "cases.json").read_text()), ids=lambda c: c["name"])
def test_variants_preserve_api_and_verification(case, tmp_path):
    root = materialize(case, tmp_path / "app")
    original = files(FIXTURE / "original")
    actual = files(root)
    assert actual.keys() == original.keys(), "no source, route, config or lock deletion"
    allowed = {str(p.relative_to(FIXTURE / "overlays" / overlay))
               for overlay in case["overlays"] for p in (FIXTURE / "overlays" / overlay).rglob("*") if p.is_file()}
    assert {p for p in original if original[p] != actual[p]} == allowed
    for path in allowed:
        before = (FIXTURE / "original" / path).read_text()
        after = (root / path).read_text()
        for forbidden in ["any", "@ts-ignore", "@ts-nocheck", "@ts-expect-error", "ignoreBuildErrors", "ignoreDuringBuilds"]:
            assert after.count(forbidden) <= before.count(forbidden)
    assert json.loads((root / "tsconfig.json").read_text())["compilerOptions"]["strict"] is True
    assert json.loads((root / "package.json").read_text())["scripts"]["build"] == "next build"
    if "export" in case["overlays"]:
        assert (root / "src/lib/types.ts").read_text().startswith((FIXTURE / "original/src/lib/types.ts").read_text())


def test_materialize_refuses_existing_workspace(tmp_path):
    with pytest.raises(FileExistsError):
        materialize({"overlays": ["promise"]}, tmp_path)


def test_measured_build_results_cover_each_case(tmp_path):
    report = json.loads((FIXTURE / "evidence/build-results.json").read_text())
    cases = json.loads((FIXTURE / "cases.json").read_text())
    assert report["install_exit_code"] == 0
    assert report["dependencies"]["next"] == "14.2.35"
    assert report["fixture_inputs_sha256"] == {p: h for p, h in files(FIXTURE).items()
                                               if p.startswith(("original/", "overlays/")) or p == "cases.json"}
    assert len(report["results"]) == len(cases)
    for case, result in zip(cases, report["results"], strict=True):
        assert result["case"] == case["name"]
        assert result["exit_code"] == case["exit_code"]
        assert result["type_check_observed"] is True
        assert result["source_sha256"] == files(materialize(case, tmp_path / case["name"]))
        for expected in case["diagnostics"]:
            assert expected in result["diagnostics"]
