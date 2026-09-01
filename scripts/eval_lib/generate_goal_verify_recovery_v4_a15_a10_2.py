from __future__ import annotations

import argparse
import copy
from typing import Any

from eval_lib.generate_goal_verify_main_v4 import _load, _write_json
from eval_lib.generate_goal_verify_recovery_v4_a14_a2 import ROOT, _file_sha256
from eval_lib.generate_goal_verify_recovery_v4_a15_a1 import (
    _validate_exact_sha_evidence,
)

EVAL = ROOT / "eval/goal_verify/v0"
BASE_CONTRACT_PATH = EVAL / "phase6-recovery-v4-a15-a10-1-full-contract.json"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a15-a10-2-full-contract.json"
CONTRACT_ID = "phase6-recovery-v4-20260901-a15-a10-2-live-01"
PARTIAL_CONTRACT_ID = "phase6-recovery-v4-20260901-a15-a10-1-live-01"
TIMEOUT_CAPTURE_SOURCE = "scripts/eval_lib/subprocess_capture.py"
PARTIAL_CODE_SHA = "a346e33b57f4027cb20a4cbff3e5d83e01222f55"
PARTIAL_BINDINGS = {
    "campaign_manifest_sha256": (
        "6bcadab72622ae14c8df693784a6978e2e4778d88318db17a90f6e585c9dfe3d"
    ),
    "campaign_summary_sha256": (
        "207045ce1669de132083155f73446d204004c11ec574685eff994dcacd917d41"
    ),
    "commandagent_binary_sha256": (
        "91e511715854f953ce7384fb79ad5f445d92ad385fb11cdb75da4858dc65df22"
    ),
    "contract_sha256": (
        "fcee31cf3bb7bb9f703cc20d5ce7591aec90fb7c7960ad952c99f93d5b06d8b0"
    ),
    "last_record_sha256": (
        "b1c214aefd0ad7daa7734c56bc042f17002475ccd189faf9342b9ce1bfc7be29"
    ),
    "record_ledger_head_sha256": (
        "139d92502314527bf69df1f4bc60f073b578b2ce60e0561adc429c01787dc451"
    ),
    "record_ledger_sha256": (
        "7367b77e930eb0e06f13edced444ac736080d7b91eb9323de7440482d0eb631b"
    ),
}


def _validate_partial_failure(evidence: dict[str, Any]) -> None:
    expected = {
        "schema_version": "commandagent.goal_verify.recovery_partial_failure.v1",
        "contract_id": PARTIAL_CONTRACT_ID,
        "run_id": PARTIAL_CONTRACT_ID,
        "code_sha": PARTIAL_CODE_SHA,
        "status": "instrumentation_unusable",
        "completed_pairs": 73,
        "target_pairs": 140,
        "last_completed_pair_id": "phase6-main-c13-task-05--pair-01",
    }
    if any(evidence.get(key) != value for key, value in expected.items()):
        raise ValueError("unexpected A15-A10.1 partial-run identity or count")
    failure = evidence.get("failure")
    if not isinstance(failure, dict) or any(
        failure.get(key) != value
        for key, value in {
            "phase": "raw_record_serialization",
            "trigger": "product_timeout",
            "product_timeout_sec": 900,
            "exception_type": "TypeError",
            "exception_message": "Object of type bytes is not JSON serializable",
            "uncommitted_pair_id": "phase6-main-c13-task-05--pair-02",
            "raw_record_created": False,
            "ledger_entry_created": False,
        }.items()
    ):
        raise ValueError("unexpected A15-A10.1 partial-run failure")
    bindings = evidence.get("bindings")
    if bindings != PARTIAL_BINDINGS:
        raise ValueError("A15-A10.1 partial-run bindings are invalid")


def build_contract(
    *,
    code_sha: str,
    exact_sha_ci_evidence: str,
    partial_failure: dict[str, Any],
    partial_failure_path: str,
    partial_failure_sha256: str,
    authorized: bool,
) -> dict[str, Any]:
    base = _load(BASE_CONTRACT_PATH)
    if base.get("contract_id") != PARTIAL_CONTRACT_ID:
        raise ValueError("unexpected A15-A10.1 base contract")
    if base.get("status") != "frozen" or "full_experiment" not in base:
        raise ValueError("A15-A10.2 must inherit the frozen full contract")
    _validate_partial_failure(partial_failure)

    evidence_path = (ROOT / exact_sha_ci_evidence).resolve()
    partial_path = (ROOT / partial_failure_path).resolve()
    for path, label in (
        (evidence_path, "exact-SHA evidence"),
        (partial_path, "partial-run evidence"),
    ):
        try:
            path.relative_to(ROOT.resolve())
        except ValueError as error:
            raise ValueError(f"{label} must be inside the repository") from error
    _validate_exact_sha_evidence(code_sha=code_sha, evidence_path=evidence_path)
    if _file_sha256(partial_path) != partial_failure_sha256:
        raise ValueError("A15-A10.1 partial-run evidence sha256 mismatch")

    contract = copy.deepcopy(base)
    contract.update(
        {
            "contract_id": CONTRACT_ID,
            "smoke_run_id": CONTRACT_ID,
            "code_sha": code_sha,
            "exact_sha_ci_evidence": exact_sha_ci_evidence,
            "status": "frozen",
            "supersedes_contract": base["contract_id"],
            "partial_run_evidence": {
                "contract_id": partial_failure["contract_id"],
                "path": partial_failure_path,
                "sha256": partial_failure_sha256,
                "status": partial_failure["status"],
                "completed_pairs": partial_failure["completed_pairs"],
                "target_pairs": partial_failure["target_pairs"],
                "record_ledger_head_sha256": partial_failure["bindings"][
                    "record_ledger_head_sha256"
                ],
                "inference_role": partial_failure["inference_role"],
            },
        }
    )
    contract["runner_sources"] = list(
        dict.fromkeys([*base["runner_sources"], TIMEOUT_CAPTURE_SOURCE])
    )
    contract["analysis"].update(
        {
            "a15_a10_1_historical_run_policy": partial_failure[
                "historical_run_policy"
            ],
            "subprocess_timeout_capture_policy": (
                "normalize TimeoutExpired stdout and stderr to bounded UTF-8 strings "
                "at the subprocess boundary; preserve timeout as an honest "
                "baseline_unavailable result and reject any remaining non-JSON record "
                "value with its typed field path"
            ),
        }
    )
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A15-A10.2",
            "reason": (
                "A15-A10.1 stopped after 73 committed records when pair 74 reached "
                "the unchanged 900-second product timeout and Python exposed captured "
                "output as bytes that the raw-record JSON writer rejected"
            ),
            "historical_run_policy": partial_failure["historical_run_policy"],
            "correction_scope": (
                "normalize timeout stdout and stderr to bounded UTF-8 strings and add "
                "a field-path JSON-safety guard before raw-record persistence"
            ),
            "frozen_design_policy": (
                "restart all 140 pairs under one corrected exact-SHA runner; preserve "
                "every task, repetition, model, prompt, workspace, external oracle, "
                "Recovery 0-vs-1 arm, maximum one Recovery, exclusion, 900-second "
                "product timeout, four resource budgets, stopping rule, and "
                "2,000-sample bootstrap without inspecting A15-A10.2 outcomes"
            ),
            "inference_role": (
                "pre-collection instrument correction; A15-A10.1 partial records are "
                "excluded from the effect estimate"
            ),
        }
    )
    contract["authorization"].update(
        {
            "full_collection_authorized": authorized,
            "approved_by": "repository owner" if authorized else None,
            "approved_at": "2026-09-01" if authorized else None,
        }
    )
    return contract


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Freeze A15-A10.2 after the timeout-capture instrument fix"
    )
    parser.add_argument("--code-sha", required=True)
    parser.add_argument("--exact-sha-ci-evidence", required=True)
    parser.add_argument("--partial-run-evidence", required=True)
    parser.add_argument("--full-collection-authorized", action="store_true")
    args = parser.parse_args()
    partial_path = (ROOT / args.partial_run_evidence).resolve()
    partial = _load(partial_path)
    contract = build_contract(
        code_sha=args.code_sha,
        exact_sha_ci_evidence=args.exact_sha_ci_evidence,
        partial_failure=partial,
        partial_failure_path=args.partial_run_evidence,
        partial_failure_sha256=_file_sha256(partial_path),
        authorized=args.full_collection_authorized,
    )
    _write_json(CONTRACT_PATH, contract)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
