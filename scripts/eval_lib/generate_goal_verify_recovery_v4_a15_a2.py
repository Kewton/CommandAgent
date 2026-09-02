from __future__ import annotations

import argparse
import copy
from typing import Any

from eval_lib.generate_goal_verify_main_v4 import _load, _write_json
from eval_lib.generate_goal_verify_recovery_v4_a14_a2 import ROOT, _json_sha256
from eval_lib.generate_goal_verify_recovery_v4_a15_a1 import (
    _validate_exact_sha_evidence,
)

EVAL = ROOT / "eval/goal_verify/v0"
BASE_CONTRACT_PATH = EVAL / "phase6-recovery-v4-a15-a1-1-smoke-contract.json"
BASE_TASKS_PATH = EVAL / "phase6-task-contracts-v4-a15.json"
TASKS_PATH = EVAL / "phase6-task-contracts-v4-a15-a2.json"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a15-a2-smoke-contract.json"
CONTRACT_ID = "phase6-recovery-v4-20260831-a15-a2-smoke-01"


def build_tasks() -> dict[str, Any]:
    tasks = copy.deepcopy(_load(BASE_TASKS_PATH))
    tasks["status"] = "frozen"
    amended = 0
    for task in tasks.get("cases", []):
        if task.get("case_id", "").startswith("phase6-main-c07-task-"):
            completion = task["completion_contract"]
            completion["required_obligations"] = ["implementation"]
            task["decision"] = (
                "A15-A2 keeps implementation and the exact bound reproducer as product "
                "requirements; verification and acceptance remain executed observations, "
                "not synthetic README or test-file artifact obligations"
            )
            amended += 1
    if amended != 10:
        raise ValueError(f"expected 10 generic fix task amendments, got {amended}")
    return tasks


def build_contract(
    *,
    code_sha: str,
    exact_sha_ci_evidence: str,
    authorized: bool,
    tasks: dict[str, Any],
) -> dict[str, Any]:
    base = _load(BASE_CONTRACT_PATH)
    if base.get("contract_id") != "phase6-recovery-v4-20260831-a15-a1-1-smoke-01":
        raise ValueError("unexpected A15-A1.1 base contract")
    if base.get("status") != "frozen" or "full_experiment" in base:
        raise ValueError("A15-A2 must inherit the frozen A15-A1.1 smoke contract")
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
    contract["analysis"].update(
        {
            "recovery_fix_origin_evidence_policy": (
                "immutable failed-fix evidence is inherited across Recovery boundary "
                "observations; later adjudication writes cannot change its hash"
            ),
            "recovery_fix_runtime_resume_timing": "before the first Recovery phase",
            "recovery_generated_step_binding_timing": "before StepPlan lint",
            "recovery_inspection_phase_policy": (
                "one host-owned read-only step using workspace-relative product paths only"
            ),
            "hidden_path_workspace_policy": (
                "engine-private components are evaluated after stripping the treatment "
                "workspace root; a private parent directory does not taint product paths"
            ),
            "nextjs_registered_runtime_families": [
                "Next 14 / React 18",
                "Next 16 / React 19",
            ],
            "generic_fix_obligation_policy": (
                "implementation artifact plus exact registered bound reproducer; no "
                "unregistered README or synthetic test-file obligation"
            ),
            "existing_frozen_input_absence_assertion_policy": (
                "reject unless the task explicitly requires deleting that path"
            ),
        }
    )
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A15-A2",
            "reason": (
                "correct fix-origin continuity, pre-lint Recovery binding, treatment-root "
                "path semantics, registered Next.js version families, and generic fix "
                "obligations found by the A15-A1.1 smoke"
            ),
            "historical_run_policy": (
                "A15-A1.1 smoke-01 remains immutable NO-GO evidence and is never rescored"
            ),
            "inference_role": "repeat the same frozen 14-pair instrument smoke; no effect claim",
            "product_findings": [
                "successful resumed fix evidence was mutable and rejected at promotion preflight",
                "registered Recovery verification was bound after StepPlan lint",
                "a .commandagent parent of the isolated workspace tainted valid product paths",
                "Next 16/React 19 was coerced to Next 14/React 18",
                "generic fix tasks declared unrequested verification and acceptance artifact obligations",
                "generic repair fallback proposed Next.js entrypoints",
                "a negative assertion could require an existing frozen input to disappear",
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
    parser = argparse.ArgumentParser(description="Freeze the A15-A2 Recovery smoke")
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
