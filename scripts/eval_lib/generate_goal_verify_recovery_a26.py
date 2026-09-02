from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path
from typing import Any

from eval_lib.generate_goal_verify_main_v4 import _write_json
from eval_lib.generate_goal_verify_recovery_v4_a14_a2 import ROOT
from eval_lib.generate_goal_verify_recovery_v4_a15_a1 import (
    _validate_exact_sha_evidence,
)
from eval_lib.goal_verify_recovery_deterministic_pair import (
    A26_SCHEMA_VERSION,
    A26_SOURCE_PATHS,
    ARM_ORDER,
    NEXTJS_PORT,
    SCENARIO_ORDER,
    SCENARIOS,
    fixture_manifest_sha256,
    provisioning_manifest_sha256,
    sha256_file,
)

CONTRACT_PATH = (
    ROOT / "eval/goal_verify/v0/phase6-recovery-deterministic-a26-pilot-contract.json"
)
CONTRACT_ID = "phase6-recovery-deterministic-20260902-a26-pilot-01"


def _generator_sha256() -> str:
    return hashlib.sha256(Path(__file__).read_bytes()).hexdigest()


def _binary_version(commandagent_bin: Path) -> str:
    completed = subprocess.run(
        [str(commandagent_bin), "--version"],
        stdin=subprocess.DEVNULL,
        text=True,
        capture_output=True,
        timeout=30,
        check=False,
    )
    if completed.returncode != 0 or not completed.stdout.strip():
        raise ValueError("unable to read the A26 commandagent binary version")
    return completed.stdout.strip()


def build_contract(
    *,
    code_sha: str,
    exact_sha_ci_evidence: str,
    commandagent_bin: Path,
    node_modules_source: Path,
    authorized: bool,
) -> dict[str, Any]:
    if len(code_sha) != 40 or any(ch not in "0123456789abcdef" for ch in code_sha):
        raise ValueError("code SHA must be a full lowercase hexadecimal Git SHA")
    evidence_path = (ROOT / exact_sha_ci_evidence).resolve()
    try:
        evidence_path.relative_to(ROOT.resolve())
    except ValueError as error:
        raise ValueError("exact-SHA evidence must be inside the repository") from error
    _validate_exact_sha_evidence(code_sha=code_sha, evidence_path=evidence_path)

    commandagent_bin = commandagent_bin.resolve()
    if not commandagent_bin.is_file():
        raise ValueError(f"commandagent binary is missing:{commandagent_bin}")
    binary_sha256 = sha256_file(commandagent_bin)
    binary_version = _binary_version(commandagent_bin)
    if code_sha[:8] not in binary_version:
        raise ValueError("commandagent version does not contain the pinned code SHA")

    node_modules_source = node_modules_source.resolve()
    node_modules_sha256 = provisioning_manifest_sha256(node_modules_source)
    scenarios = [
        {
            "scenario_id": scenario_id,
            "profile": SCENARIOS[scenario_id].profile,
            "source_fixture": {
                "generic-fix": (
                    "tests/fixtures/goal_verify_v4/main/fix-generic-fixtures/before"
                ),
                "data-fix": (
                    "tests/fixtures/goal_verify_v4/a15/fix-data-reconciliation/before"
                ),
                "nextjs-fix": (
                    "tests/fixtures/goal_verify_v4/a15/fix-nextjs-route-label/before"
                ),
            }[scenario_id],
            "fixture_manifest_sha256": fixture_manifest_sha256(scenario_id),
            "target_path": SCENARIOS[scenario_id].target_path,
            "protected_paths": list(SCENARIOS[scenario_id].protected_paths),
            "verify_commands": list(SCENARIOS[scenario_id].verify_commands),
            "host_route_endpoint": (
                {
                    "path": "/",
                    "port": NEXTJS_PORT,
                    "selector": "#result-02",
                    "control_expected_text": "stale-02",
                    "treatment_expected_text": "ready-02",
                }
                if scenario_id == "nextjs-fix"
                else None
            ),
        }
        for scenario_id in SCENARIO_ORDER
    ]
    return {
        "schema_version": A26_SCHEMA_VERSION,
        "contract_id": CONTRACT_ID,
        "run_id": CONTRACT_ID,
        "status": "frozen",
        "frozen_at": "2026-09-02",
        "code_sha": code_sha,
        "binary_sha256": binary_sha256,
        "binary_version": binary_version,
        "exact_sha_ci_evidence": exact_sha_ci_evidence,
        "generator_source_sha256": _generator_sha256(),
        "authoritative_source_sha256": {
            relative: sha256_file(ROOT / relative) for relative in A26_SOURCE_PATHS
        },
        "nextjs_node_modules_manifest_sha256": node_modules_sha256,
        "nextjs_provisioning": {
            "mode": "external_hash_bound_directory_copy",
            "source_hint": (
                "tests/fixtures/goal_verify_v3/"
                "create-ui-copy-style-port-path/reference/node_modules"
            ),
            "manifest_sha256": node_modules_sha256,
            "copy_policy": "fresh private copy before each A26 arm",
            "network": "denied",
        },
        "scenarios": scenarios,
        "design": {
            "inference_role": "paired_instrument_validation_only",
            "scenario_order": list(SCENARIO_ORDER),
            "arm_order": list(ARM_ORDER),
            "required_profiles": ["generic", "data", "nextjs"],
            "recovery_auto_runs": {"control": 0, "treatment": 1},
            "fresh_workspace_per_arm": True,
            "same_input_snapshot_required_within_pair": True,
            "same_initial_failure_boundary_required_within_pair": True,
            "same_binary_required_for_all_arms": True,
            "process_returncode_is_endpoint": False,
            "endpoint": (
                "all scenario-registered verify commands pass; Next.js additionally "
                "requires the frozen / route selector to render ready-02"
            ),
            "control_endpoint": (
                "the registered reproducer remains at exit 1, all registered "
                "regressions remain at exit 0, and the repair target is unchanged"
            ),
            "treatment_endpoint": (
                "one Recovery run mutates only the intended target/probe outputs, "
                "passes all registered commands, passes the Next.js route endpoint, "
                "and is promoted"
            ),
            "allocation": (
                "deterministic control then treatment order on independent fresh "
                "copies; no arm replacement or rerun"
            ),
        },
        "estimand": {
            "name": "conditional scripted fault-boundary Recovery availability",
            "population": (
                "the three frozen A26 fixtures at the scripted, reproducible failure "
                "boundary only"
            ),
            "contrast": "Recovery auto-runs 1 versus 0 on the registered endpoint",
            "natural_operation_effect": False,
            "cross_profile_generalization": False,
            "confirmatory_effect_estimate_in_a26": False,
        },
        "analysis": {
            "pair_count": 3,
            "arm_count": 6,
            "descriptive_contrast_policy": (
                "record each paired binary endpoint contrast solely to validate the "
                "instrument; do not aggregate or report it as an effect estimate"
            ),
            "go_rule": (
                "GO requires exactly one valid pair in generic, data, and Next.js; "
                "identical input and initial failure boundary within every pair; "
                "failed control endpoint; passed and promoted treatment endpoint; "
                "unchanged protected paths; pinned binary and provisioning"
            ),
            "invalid_policy": (
                "freeze all A26 evidence, diagnose forward-only, and never replace, "
                "rerun, rescore, resize, or pool an A26 arm"
            ),
            "go_next_action": (
                "request repository-owner review before preregistering or collecting "
                "an A27 confirmatory conditional-effect experiment"
            ),
            "no_go_next_action": (
                "perform forward-only instrument diagnosis without effect estimation"
            ),
        },
        "historical_evidence_policy": {
            "A23": "immutable INVALID natural-exposure pilot; never rescore or pool",
            "A24": "immutable INVALID natural-exposure pilot; never rescore or pool",
            "A25": (
                "immutable GO instrument / NOT_MET natural-exposure pilot; never "
                "rescore or pool"
            ),
        },
        "effect_claim_allowed": False,
        "generalization_claim_allowed": False,
        "default_rollout_allowed": False,
        "full_collection_allowed": False,
        "authoritative_command": (
            "scripts/eval-goal-verify-recovery-deterministic-pair.py --contract "
            "eval/goal_verify/v0/phase6-recovery-deterministic-a26-pilot-contract.json "
            "--commandagent-bin <PINNED_BINARY> --nextjs-node-modules "
            "tests/fixtures/goal_verify_v3/create-ui-copy-style-port-path/reference/"
            "node_modules --run-dir dev-reports/issue-399/runs/"
            f"{CONTRACT_ID}"
        ),
        "report_exit_semantics": (
            "zero iff all preregistered paired instrument checks pass; nonzero freezes "
            "A26 as INVALID and requires forward-only diagnosis"
        ),
        "authorization": {
            "pilot_collection_authorized": authorized,
            "confirmatory_collection_authorized": False,
            "approved_by": "repository owner" if authorized else None,
            "approved_at": "2026-09-02" if authorized else None,
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Freeze the A26 deterministic paired Recovery pilot contract"
    )
    parser.add_argument("--code-sha", required=True)
    parser.add_argument("--exact-sha-ci-evidence", required=True)
    parser.add_argument("--commandagent-bin", type=Path, required=True)
    parser.add_argument("--nextjs-node-modules", type=Path, required=True)
    parser.add_argument("--pilot-collection-authorized", action="store_true")
    args = parser.parse_args()
    contract = build_contract(
        code_sha=args.code_sha,
        exact_sha_ci_evidence=args.exact_sha_ci_evidence,
        commandagent_bin=args.commandagent_bin,
        node_modules_source=args.nextjs_node_modules,
        authorized=args.pilot_collection_authorized,
    )
    _write_json(CONTRACT_PATH, contract)
    print(json.dumps(contract, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
