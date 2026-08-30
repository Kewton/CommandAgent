from __future__ import annotations

import argparse
from typing import Any

from eval_lib.generate_goal_verify_main_v4 import _write_json
from eval_lib.generate_goal_verify_recovery_v4_a14_a2 import ROOT
from eval_lib.generate_goal_verify_recovery_v4_a14_a3 import (
    _build_contract as _build_a14_a3_contract,
)

EVAL = ROOT / "eval/goal_verify/v0"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a14-a4-contract.json"

CONTRACT_ID = "phase6-recovery-v4-20260830-a14-a4-live-01"
SMOKE_ID = "phase6-recovery-v4-20260830-a14-a4-smoke-01"


def _build_contract(
    *,
    status: str,
    code_sha: str,
    exact_sha_ci_evidence: str,
    live_collection_authorized: bool,
) -> dict[str, Any]:
    contract = _build_a14_a3_contract(
        status=status,
        code_sha=code_sha,
        exact_sha_ci_evidence=exact_sha_ci_evidence,
        live_collection_authorized=live_collection_authorized,
    )
    contract.update(
        {
            "schema_version": (
                "commandagent.goal_verify.recovery_experiment.v4_a14_a4"
            ),
            "contract_id": CONTRACT_ID,
            "smoke_run_id": SMOKE_ID,
            "supersedes_contract": (
                "phase6-recovery-v4-20260829-a14-a3-live-01"
            ),
            "supersedes_smoke_run": (
                "phase6-recovery-v4-20260829-a14-a3-smoke-01"
            ),
        }
    )
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A14-A4",
            "reason": (
                "A14-A3 proved that a host-rewritten negative assertion and an "
                "internal false failure could trigger Recovery that damaged an "
                "already passing deliverable"
            ),
            "historical_run_policy": (
                "A14-A3 smoke-01 remains immutable diagnostic evidence and is "
                "never rescored as evidence for the corrected safety boundary"
            ),
            "inference_role": (
                "safety and oracle-executability diagnostic only; no Recovery "
                "success-rate or retry-count effect claim"
            ),
            "product_findings": [
                "shell_control_split discarded an || fallback and reversed polarity",
                "fallback_true_stripped reversed a negative assertion",
                "step plans lack a typed file-content absent predicate",
                "recover intent did not inherit fix-runtime temporal roles",
                "expected-fail steps could edit source artifacts",
                "verify repair could impose write_required on expected-fail steps",
                "expected-fail Python exceptions could never pass because traceback was fatal",
                "last-wins Recovery candidate selection lost failed-step checks",
                "F2 after-passes was not observed at the Recovery boundary",
                "generic completion added unregistered README and test targets",
                "the Recovery harness did not provision frozen playwright-core",
                "Next.js route-bound fallback language leaked into generic tasks",
            ],
        }
    )
    contract["analysis"].update(
        {
            "current_success_is_not_improvement": True,
            "current_success_mutation_forbidden": True,
            "external_oracle_passed_to_product_or_recovery": False,
            "recovery_treatment_promotion_requires_non_regression": True,
            "blocked_browser_oracle_excluded_from_effect_evidence": True,
        }
    )
    contract["recovery_safety_policy"] = {
        "semantic_rewrite": (
            "reject commands whose sanitizer rewrite would change fallback or polarity"
        ),
        "historical_failure": (
            "immutable evidence; never converted into a current must-fail obligation"
        ),
        "pre_recovery_observation": (
            "read-only product-visible completion contract only"
        ),
        "current_success": (
            "suppress automatic Recovery, preserve artifacts, and retain honest terminal"
        ),
        "candidate_selection": (
            "step-specific handoff outranks later phase-only handoff"
        ),
        "maximum_automatic_recovery_runs": 1,
        "mutation_isolation": (
            "run Recovery in a boundary-derived treatment workspace; promote "
            "source/config to the product workspace only after registered "
            "post-Recovery observations pass"
        ),
    }
    smoke = contract["smoke"]
    smoke.update(
        {
            "inference_role": "safety and oracle-executability diagnostic only",
            "minimum_executed_recovery_pairs": 0,
            "minimum_current_success_suppressions": 1,
            "require_executed_recovery_for_attribution": False,
            "require_browser_oracle_executability": True,
            "require_transaction_control_retention": True,
            "require_recovery_handoff_fidelity": True,
            "require_isolated_treatment_workspace": True,
            "effect_claim_allowed": False,
        }
    )
    smoke["required_readiness_checks"] = list(
        dict.fromkeys(
            [
                *smoke["required_readiness_checks"],
                "current_success_suppression_observed",
                "browser_oracle_executability_preflight",
                "transaction_control_retention",
                "recovery_handoff_fidelity",
                "isolated_recovery_treatment",
            ]
        )
    )
    contract["runner_sources"].append(
        "scripts/eval_lib/generate_goal_verify_recovery_v4_a14_a4.py"
    )
    return contract


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate A14-A4 Recovery safety-boundary contract"
    )
    parser.add_argument("--code-sha")
    parser.add_argument("--exact-sha-ci-evidence")
    parser.add_argument("--smoke-collection-authorized", action="store_true")
    args = parser.parse_args()
    if bool(args.code_sha) != bool(args.exact_sha_ci_evidence):
        parser.error("--code-sha and --exact-sha-ci-evidence must be paired")
    if args.smoke_collection_authorized and not args.code_sha:
        parser.error("smoke authorization requires exact-SHA inputs")
    contract = _build_contract(
        status="frozen" if args.code_sha else "draft",
        code_sha=args.code_sha or "",
        exact_sha_ci_evidence=args.exact_sha_ci_evidence or "",
        live_collection_authorized=args.smoke_collection_authorized,
    )
    _write_json(CONTRACT_PATH, contract)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
