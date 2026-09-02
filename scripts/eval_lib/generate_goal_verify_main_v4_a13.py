from __future__ import annotations

import argparse
import copy
from pathlib import Path
from typing import Any

from eval_lib.generate_goal_verify_main_v4 import _load, _write_json
from eval_lib.goal_verify_semantic_policy_v4 import semantic_policy_sha256

ROOT = Path(__file__).resolve().parents[2]
EVAL = ROOT / "eval/goal_verify/v0"

BASE_CONTRACT = EVAL / "phase6-main-v4-contract.json"
BASE_TASKS = EVAL / "phase6-task-contracts-v4-main.json"
BASE_ADAPTERS = EVAL / "phase6-command-adapters-v4-main.json"

CONTRACT_PATH = EVAL / "phase6-main-v4-a13-contract.json"
TASKS_PATH = EVAL / "phase6-task-contracts-v4-a13-main.json"
ADAPTERS_PATH = EVAL / "phase6-command-adapters-v4-a13-main.json"

CONTRACT_ID = "phase6-main-v4-20260829-live-02"
SMOKE_ID = "phase6-main-v4-20260829-a13-smoke-01"


def _build_tasks(*, status: str) -> dict[str, Any]:
    registry = copy.deepcopy(_load(BASE_TASKS))
    registry.update(
        {
            "status": status,
            "supersedes": "eval/goal_verify/v0/phase6-task-contracts-v4-main.json",
        }
    )
    for row in registry["cases"]:
        row["completion_contract"]["goal"] = row["goal"]
        row["decision"] = (
            "A13 binds the nested completion goal to the shared semantic goal; "
            "operational constraints remain non-scored"
        )
    return registry


def _build_adapters(*, status: str) -> dict[str, Any]:
    registry = copy.deepcopy(_load(BASE_ADAPTERS))
    registry.update(
        {
            "status": status,
            "contract_id": CONTRACT_ID,
            "supersedes": "eval/goal_verify/v0/phase6-command-adapters-v4-main.json",
        }
    )
    registry["rules"].update(
        {
            "semantic_policy_sha256": semantic_policy_sha256(),
            "semantic_admissibility": (
                "adapter availability and claim-specific observer capability are "
                "checked before execution and scoring"
            ),
            "observed_strength": (
                "derived only after semantic admissibility from both observation "
                "method and semantic sufficiency"
            ),
        }
    )
    for adapter in registry["adapters"]:
        executor = adapter.get("executor", {})
        if executor.get("kind") != "existing_evidence_probe":
            continue
        adapter["executor"] = {
            "kind": "unavailable",
            "executor_status": "unavailable",
            "reason": (
                "generic investigation artifact binding cannot establish the "
                "registered claim-specific oracle kind"
            ),
            "original_workspace": executor.get("workspace"),
            "original_stage": executor.get("stage"),
        }
    return registry


def _a13_amendment() -> dict[str, Any]:
    return {
        "id": "v4-A13",
        "recorded_at": "2026-08-29",
        "before_first_provider_response": True,
        "result_dependent": False,
        "supersedes_run_id": "phase6-main-v4-20260828-live-01",
        "replacement_run_id": CONTRACT_ID,
        "change": (
            "introduce a host-owned capability-bound semantic policy; fail closed "
            "when a claim-specific observer is unavailable; derive observed strength "
            "only after semantic admissibility; expose a scoring-free task-contract "
            "projection; compact deterministic prompt JSON; record phase timing; "
            "align existing plain-JavaScript Next workspaces without injecting "
            "TypeScript/Tailwind dependencies; accept investigation completion "
            "obligations while requiring full evidence for strong acceptance"
        ),
        "new_estimand": (
            "the same additive union score and 12x10x3 cluster design on the A13 "
            "product and instrument snapshot; no live-01 record is rescored or reused"
        ),
        "semantic_policy_sha256": semantic_policy_sha256(),
        "denominator_changes": "none",
        "exclusion_rule_changes": "none",
        "threshold_changes": "none",
        "schema_content_changes": "none",
        "scoring_changes": (
            "an adapter match can affect pass/fail/strength only after semantic "
            "admissibility; unavailable capability remains unverified"
        ),
        "prompt_changes": (
            "semantically equivalent compact A13 prompt plus explicit capability, "
            "proxy rejection, and honest-unknown rules"
        ),
        "product_changes": (
            "Next scaffold mode follows existing language/styling intent; "
            "investigation is a supported completion obligation with full-evidence "
            "acceptance"
        ),
        "resource_changes": (
            "candidate-visible task-contract projection, compact shape/input JSON, "
            "num_predict 1280, and per-phase wall-time instrumentation"
        ),
        "historical_evidence_policy": (
            "retain live-01 as immutable NO-GO evidence; never resume, rescore, "
            "rewrite, or copy its provider responses, raw records, or reviews"
        ),
    }


def _build_contract(
    *,
    status: str,
    code_sha: str,
    exact_sha_ci_evidence: str,
    live_collection_authorized: bool,
) -> dict[str, Any]:
    contract = copy.deepcopy(_load(BASE_CONTRACT))
    contract.update(
        {
            "status": status,
            "contract_id": CONTRACT_ID,
            "supersedes": "phase6-main-v4-20260828-live-01",
            "superseded_contract": ("eval/goal_verify/v0/phase6-main-v4-contract.json"),
            "code_sha": code_sha,
            "exact_sha_ci_evidence": exact_sha_ci_evidence,
            "task_contract_registry": (
                "eval/goal_verify/v0/phase6-task-contracts-v4-a13-main.json"
            ),
        }
    )
    contract["baseline"]["task_contract_registry"] = contract["task_contract_registry"]
    contract["baseline"]["product_findings_recorded"] = [
        *(
            f"live-01 finding: {finding}"
            for finding in contract["baseline"]["product_findings_recorded"]
        ),
        *[
            "live-01 investigation cells 09-12 rejected the investigation completion obligation before task execution (118/120)",
            "A13 changes the product baseline: existing plain-JavaScript Next workspaces preserve their language and plain-CSS dependency boundary",
            "A13 accepts investigation completion obligations but only full claim-bound evidence can satisfy strong acceptance",
        ],
    ]
    contract["comparison"]["estimand_version"] = "A13 product/instrument snapshot"
    contract["generation"].update(
        {
            "prompt": (
                "eval/goal_verify/v0/verification-spec-preflight-v4-a13.prompt.txt"
            ),
            "num_predict": 1280,
            "prompt_compaction": (
                "compact shape/input JSON and scoring-free task-contract projection; "
                "semantic requirements are unchanged or stricter"
            ),
        }
    )
    contract["scoring"]["answer_key"] = (
        "eval/goal_verify/v0/phase6-command-adapters-v4-a13-main.json"
    )
    contract["semantic_oracle_policy"] = {
        "schema_version": "commandagent.goal_verify.semantic_policy.v4_a13",
        "sha256": semantic_policy_sha256(),
        "source": "scripts/eval_lib/goal_verify_semantic_policy_v4.py",
        "enforcement_points": [
            "candidate-visible expected observations and capabilities",
            "pre-execution adapter availability",
            "post-execution scoring and observed-strength derivation",
        ],
        "unsupported_result": "unverified",
    }
    contract["main_analysis"]["resource_measurement"].update(
        {
            "candidate_phase_timing": (
                "prompt assembly, provider request, raw-schema validation, "
                "canonicalization, proposal validation, oracle execution, scoring, "
                "and instrumentation residual"
            ),
            "phase_timing_missing": "hard diagnostic failure; never imputed",
        }
    )
    contract["pre_live_amendments"] = [
        *contract["pre_live_amendments"],
        _a13_amendment(),
    ]
    contract["smoke"].update(
        {
            "run_id": SMOKE_ID,
            "request_namespace": SMOKE_ID,
            "full_run_id": CONTRACT_ID,
            "not_used_for": (
                "main quality inference; A13 smoke records and responses are never "
                "copied into live-02"
            ),
        }
    )
    contract["authorization"] = {
        "implementation_authorized": True,
        "live_collection_authorized": live_collection_authorized,
        "approved_at": "2026-08-29" if live_collection_authorized else None,
        "approved_by": "repository owner" if live_collection_authorized else None,
        "scope": (
            "A13 local preregistered smoke and 360-pair replacement collection"
            if live_collection_authorized
            else "A13 implementation and offline verification only"
        ),
        "note": (
            "live collection requires explicit authorization after the draft, "
            "exact-SHA CI evidence, and freeze inputs are reviewed"
        ),
    }
    reviewer_policy = contract["semantic_review"]["calibration_reviewer_policy"]
    reviewer_policy.pop("authorized_ai_reviewer", None)
    reviewer_policy["authorization_status"] = (
        "unassigned; bind a user-authorized source-blind reviewer before freeze"
    )
    contract["semantic_review"]["main_sample"]["authoritative_reviewer"] = (
        "unassigned before A13 freeze"
    )
    contract["freeze_checklist"] = [
        "A13 focused Python, Rust, corpus, Ruff, fmt, clippy, and full tests green",
        "release binaries built and exact implementation SHA CI/acceptance success recorded",
        "A13 contract, semantic policy, task contracts, adapters, prompt, product baseline changes, smoke set, budgets, thresholds, exclusions, and seed frozen",
        "main design remains 12 cells x 10 source tasks x 3 runs = 360 pairs",
        "A13 live collection and source-blind reviewer explicitly authorized",
        "preregistered 12-pair A13 smoke passes instrument and phase-timing gates",
        "smoke evidence remains isolated and semantic soundness is not inferred from smoke",
    ]
    contract["runner_sources"] = sorted(
        set(contract["runner_sources"])
        | {
            "scripts/eval_lib/generate_goal_verify_main_v4_a13.py",
            "scripts/eval_lib/goal_verify_resource_diagnostics_v4.py",
            "scripts/eval_lib/goal_verify_semantic_policy_v4.py",
            "src/minimal_loop/investigation_acceptance.rs",
            "src/planner/profiles/nextjs/scaffold_mode.rs",
        }
    )
    return contract


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate Phase 6 main-v4 A13 replacement inputs"
    )
    parser.add_argument("--code-sha")
    parser.add_argument("--exact-sha-ci-evidence")
    parser.add_argument("--live-collection-authorized", action="store_true")
    args = parser.parse_args()
    if bool(args.code_sha) != bool(args.exact_sha_ci_evidence):
        parser.error("--code-sha and --exact-sha-ci-evidence must be paired")
    if args.live_collection_authorized and not args.code_sha:
        parser.error("live authorization can only be materialized with exact SHA")
    status = "frozen" if args.code_sha else "draft"
    tasks = _build_tasks(status=status)
    adapters = _build_adapters(status=status)
    contract = _build_contract(
        status=status,
        code_sha=args.code_sha or "",
        exact_sha_ci_evidence=args.exact_sha_ci_evidence or "",
        live_collection_authorized=args.live_collection_authorized,
    )
    for path, value in (
        (TASKS_PATH, tasks),
        (ADAPTERS_PATH, adapters),
        (CONTRACT_PATH, contract),
    ):
        _write_json(path, value)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
