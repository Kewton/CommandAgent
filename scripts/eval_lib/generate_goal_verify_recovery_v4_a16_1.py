from __future__ import annotations

import argparse
import copy
from typing import Any

from eval_lib.generate_goal_verify_main_v4 import _load, _write_json
from eval_lib.generate_goal_verify_recovery_v4_a14_a2 import ROOT
from eval_lib.generate_goal_verify_recovery_v4_a15_a1 import (
    _validate_exact_sha_evidence,
)

EVAL = ROOT / "eval/goal_verify/v0"
BASE_CONTRACT_PATH = EVAL / "phase6-recovery-v4-a16-smoke-contract.json"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a16-1-smoke-contract.json"
CONTRACT_ID = "phase6-recovery-v4-20260902-a16-1-smoke-01"
GENERATOR_SOURCE = "scripts/eval_lib/generate_goal_verify_recovery_v4_a16.py"


def build_contract(
    *, code_sha: str, exact_sha_ci_evidence: str, authorized: bool
) -> dict[str, Any]:
    base = _load(BASE_CONTRACT_PATH)
    if base.get("contract_id") != "phase6-recovery-v4-20260902-a16-smoke-01":
        raise ValueError("unexpected A16 base contract")
    if base.get("status") != "frozen" or "full_experiment" in base:
        raise ValueError("A16.1 must inherit the frozen A16 smoke contract")

    evidence_path = (ROOT / exact_sha_ci_evidence).resolve()
    try:
        evidence_path.relative_to(ROOT.resolve())
    except ValueError as error:
        raise ValueError("exact-SHA evidence must be inside the repository") from error
    _validate_exact_sha_evidence(code_sha=code_sha, evidence_path=evidence_path)

    contract = copy.deepcopy(base)
    contract.update(
        {
            "contract_id": CONTRACT_ID,
            "smoke_run_id": CONTRACT_ID,
            "code_sha": code_sha,
            "exact_sha_ci_evidence": exact_sha_ci_evidence,
            "status": "frozen",
            "supersedes_contract": base["contract_id"],
            "supersedes_smoke_run": base["smoke_run_id"],
        }
    )
    contract["runner_sources"] = [
        source for source in base["runner_sources"] if source != GENERATOR_SOURCE
    ]
    if len(contract["runner_sources"]) != len(base["runner_sources"]) - 1:
        raise ValueError("A16 generator source metadata was not present exactly once")
    contract["analysis"]["a16_historical_run_policy"] = (
        "A16 stopped before manifest creation, model invocation, pair execution, or raw "
        "record persistence; its frozen contract remains unchanged and is never resumed"
    )
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A16.1",
            "reason": (
                "A16 incorrectly listed its contract generator as an exact-SHA runner "
                "source even though the generator does not execute during collection"
            ),
            "historical_run_policy": (
                "A16 produced no manifest, ledger, raw record, product outcome, or model "
                "request and is not resumed"
            ),
            "correction_scope": (
                "change only contract/run identifiers, supersession metadata, and remove "
                "the non-runtime contract generator from runner_sources"
            ),
            "frozen_design_policy": (
                "preserve all six selected pairs, inputs, model and digest, prompts, "
                "external oracles, Recovery arms, gates, sentinels, and resource budgets"
            ),
            "inference_role": (
                "pre-collection metadata correction; six-pair instrument smoke only"
            ),
        }
    )
    contract["authorization"].update(
        {
            "smoke_collection_authorized": authorized,
            "full_collection_authorized": False,
            "approved_by": "repository owner" if authorized else None,
            "approved_at": "2026-09-02" if authorized else None,
        }
    )
    return contract


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Freeze A16.1 after the pre-collection runner-source correction"
    )
    parser.add_argument("--code-sha", required=True)
    parser.add_argument("--exact-sha-ci-evidence", required=True)
    parser.add_argument("--smoke-collection-authorized", action="store_true")
    args = parser.parse_args()
    contract = build_contract(
        code_sha=args.code_sha,
        exact_sha_ci_evidence=args.exact_sha_ci_evidence,
        authorized=args.smoke_collection_authorized,
    )
    _write_json(CONTRACT_PATH, contract)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
