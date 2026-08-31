from __future__ import annotations

import argparse
import copy
from typing import Any

from eval_lib.generate_goal_verify_main_v4 import _load, _write_json
from eval_lib.generate_goal_verify_recovery_v4_a14_a2 import ROOT, _json_sha256
from eval_lib.generate_goal_verify_recovery_v4_a15_a1 import (
    _validate_exact_sha_evidence,
)
from eval_lib.goal_verify_recovery_experiment_v4 import (
    SMOKE_PROFILE_PATH_COVERAGE_POLICY_V2,
)

EVAL = ROOT / "eval/goal_verify/v0"
BASE_CONTRACT_PATH = EVAL / "phase6-recovery-v4-a15-a4-smoke-contract.json"
BASE_TASKS_PATH = EVAL / "phase6-task-contracts-v4-a15-a4.json"
TASKS_PATH = EVAL / "phase6-task-contracts-v4-a15-a5.json"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a15-a5-smoke-contract.json"
CONTRACT_ID = "phase6-recovery-v4-20260831-a15-a5-smoke-01"


def build_tasks() -> dict[str, Any]:
    tasks = copy.deepcopy(_load(BASE_TASKS_PATH))
    tasks["status"] = "frozen"
    tasks["supersedes"] = str(BASE_TASKS_PATH.relative_to(ROOT))
    amended = 0
    for task in tasks.get("cases", []):
        if task.get("case_id", "").startswith("phase6-main-c13-task-"):
            task["completion_contract"]["protected_paths"] = [
                "data",
                "scripts/repro.py",
                "scripts/contract_check.py",
                "tests",
            ]
            task["decision"] = (
                "A15-A5 keeps the candidate-visible data inputs, registered reproducer, "
                "contract check, and frozen regressions read/execute-only; Recovery may "
                "repair pipeline/main.py and regenerate output artifacts."
            )
            amended += 1
    if amended != 10:
        raise ValueError(f"expected 10 data protected-path amendments, got {amended}")
    return tasks


def build_contract(
    *, code_sha: str, exact_sha_ci_evidence: str, authorized: bool, tasks: dict[str, Any]
) -> dict[str, Any]:
    base = _load(BASE_CONTRACT_PATH)
    if base.get("contract_id") != "phase6-recovery-v4-20260831-a15-a4-smoke-01":
        raise ValueError("unexpected A15-A4 base contract")
    if base.get("status") != "frozen" or "full_experiment" in base:
        raise ValueError("A15-A5 must inherit the frozen A15-A4 smoke contract")

    evidence_path = (ROOT / exact_sha_ci_evidence).resolve()
    try:
        evidence_path.relative_to(ROOT.resolve())
    except ValueError as error:
        raise ValueError("exact-SHA evidence must be inside the repository") from error
    _validate_exact_sha_evidence(code_sha=code_sha, evidence_path=evidence_path)

    contract = copy.deepcopy(base)
    previous_tasks = contract["task_contract_registry"]
    current_tasks = str(TASKS_PATH.relative_to(ROOT))
    contract.update(
        {
            "contract_id": CONTRACT_ID,
            "smoke_run_id": CONTRACT_ID,
            "code_sha": code_sha,
            "exact_sha_ci_evidence": exact_sha_ci_evidence,
            "task_contract_registry": current_tasks,
            "status": "frozen",
            "supersedes_contract": base["contract_id"],
            "supersedes_smoke_run": base["smoke_run_id"],
        }
    )
    contract["frozen_input_sha256"].pop(previous_tasks)
    contract["frozen_input_sha256"][current_tasks] = _json_sha256(tasks)
    contract["smoke"]["real_profile_path_coverage_policy"] = copy.deepcopy(
        SMOKE_PROFILE_PATH_COVERAGE_POLICY_V2
    )
    contract["analysis"].update(
        {
            "recovery_final_success_execution_policy": (
                "the host executes the exact registered CompletionContract commands; "
                "a failed final-success observation terminates Recovery without a model "
                "rewrite of the command"
            ),
            "nextjs_javascript_route_evidence_policy": (
                "app/page.js and src/app/page.js are route evidence on the same terms as "
                "the registered JSX and TSX App Router entrypoints"
            ),
            "version_aware_next_build_evidence_policy": (
                "a policy-approved direct npx next build invocation, including the frozen "
                "--webpack variant, is strong build and bound verification evidence"
            ),
            "real_profile_smoke_path_semantics": (
                "executed Recovery establishes repair-path coverage; all-pass natural "
                "completion or explicit current-success suppression establishes only "
                "no-mutation safety-path coverage"
            ),
            "data_failure_interpretation": (
                "unchanged data failures remain honest capability observations and are "
                "not converted into Recovery improvements"
            ),
            "protected_verification_input_policy": (
                "candidate-visible protected_paths may be read or executed but never "
                "created, edited, moved, or deleted; only unprotected implementation "
                "and generated output paths are repair targets"
            ),
        }
    )
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A15-A5",
            "reason": (
                "A15-A4 exposed an internal false NG for a passing plain-JavaScript "
                "Next.js artifact and a smoke-report false negative for natural completion"
            ),
            "historical_run_policy": (
                "A15-A4 smoke-01 remains immutable NO-GO evidence and is never rescored"
            ),
            "frozen_design_policy": (
                "selected pairs, tasks, model, prompts, source workspaces, external "
                "oracles, Recovery 0-vs-1 arms, exclusions, resource budgets, and "
                "effect-claim prohibition remain unchanged"
            ),
            "inference_role": (
                "repeat the same frozen 14-pair instrument smoke; no effect claim"
            ),
            "product_findings": [
                "plain-JavaScript App Router entrypoints were omitted from route evidence",
                "the approved direct version-aware Next build command was not classified as strong evidence",
                "a failed host-owned Recovery final verification could be reissued through model-authored shell syntax",
                "natural successful completion was incorrectly excluded from safety-path coverage",
                "data attempts could rewrite their registered reproducer and frozen regressions instead of repairing the pipeline",
            ],
        }
    )
    contract["authorization"].update(
        {
            "smoke_collection_authorized": authorized,
            "full_collection_authorized": False,
            "approved_at": "2026-08-31" if authorized else None,
        }
    )
    return contract


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Freeze the A15-A5 host verification and safety-path smoke"
    )
    parser.add_argument("--code-sha", required=True)
    parser.add_argument("--exact-sha-ci-evidence", required=True)
    parser.add_argument("--smoke-collection-authorized", action="store_true")
    args = parser.parse_args()
    tasks = build_tasks()
    contract = build_contract(
        code_sha=args.code_sha,
        exact_sha_ci_evidence=args.exact_sha_ci_evidence,
        authorized=args.smoke_collection_authorized,
        tasks=tasks,
    )
    _write_json(TASKS_PATH, tasks)
    _write_json(CONTRACT_PATH, contract)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
