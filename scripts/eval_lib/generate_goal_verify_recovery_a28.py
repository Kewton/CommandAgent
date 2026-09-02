from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any

from eval_lib import generate_goal_verify_recovery_a26 as a26_generator
from eval_lib.generate_goal_verify_main_v4 import _load, _write_json
from eval_lib.generate_goal_verify_recovery_v4_a14_a2 import ROOT
from eval_lib.goal_verify_recovery_deterministic_pair import (
    NEXTJS_READINESS_OVERLAY,
    manifest_sha256,
)

A27_CONTRACT_PATH = (
    ROOT / "eval/goal_verify/v0/phase6-recovery-deterministic-a27-pilot-contract.json"
)
CONTRACT_PATH = (
    ROOT / "eval/goal_verify/v0/phase6-recovery-deterministic-a28-pilot-contract.json"
)
CONTRACT_ID = "phase6-recovery-deterministic-20260902-a28-pilot-01"
A27_CONTRACT_ID = "phase6-recovery-deterministic-20260902-a27-pilot-01"
A27_REPORT_SHA256 = "42e0c578dc179be805e4528d5af87fea6ee3a86a5e0647346abcea5802667493"


def _generator_sha256() -> str:
    return hashlib.sha256(Path(__file__).read_bytes()).hexdigest()


def build_contract(
    *,
    code_sha: str,
    exact_sha_ci_evidence: str,
    commandagent_bin: Path,
    node_modules_source: Path,
    authorized: bool,
) -> dict[str, Any]:
    historical = _load(A27_CONTRACT_PATH)
    if (
        historical.get("contract_id") != A27_CONTRACT_ID
        or historical.get("status") != "frozen"
        or historical.get("effect_claim_allowed") is not False
    ):
        raise ValueError("unexpected A27 historical contract")
    contract = a26_generator.build_contract(
        code_sha=code_sha,
        exact_sha_ci_evidence=exact_sha_ci_evidence,
        commandagent_bin=commandagent_bin,
        node_modules_source=node_modules_source,
        authorized=authorized,
    )
    contract.update(
        {
            "contract_id": CONTRACT_ID,
            "run_id": CONTRACT_ID,
            "supersedes_contract": A27_CONTRACT_ID,
            "generator_source_sha256": _generator_sha256(),
        }
    )
    contract["historical_evidence_policy"].update(
        {
            "A26": (
                "immutable INVALID deterministic paired pilot; never rerun, "
                "rescore, replace, or pool any arm"
            ),
            "A27": (
                "immutable INVALID deterministic paired pilot; never rerun, "
                "rescore, replace, or pool any arm"
            ),
        }
    )
    contract["a27_diagnosis"] = {
        "frozen_report_sha256": A27_REPORT_SHA256,
        "valid_pairs": ["generic-fix", "data-fix"],
        "invalid_pair": "nextjs-fix",
        "root_causes": [
            (
                "the scripted Next.js fixture omitted the test artifact and README "
                "required by its own completion contract, so Recovery fix-safety "
                "correctly rejected the label-only treatment"
            ),
            (
                "the failure-boundary signature included recovery_prompt_saved "
                "events emitted inside the treatment, rather than only events before "
                "the first recovery_plan_auto_run_start"
            ),
            (
                "the fixture goal and package scripts did not bind the already "
                "registered host route port 4185, allowing deterministic profile "
                "repair to alter package.json inside the treatment workspace"
            ),
        ],
        "product_recovery_defect_established": False,
    }
    contract["forward_corrections"] = {
        "initial_failure_boundary_event_window": (
            "events strictly before the first recovery_plan_auto_run_start; all "
            "treatment-internal prompts are excluded"
        ),
        "nextjs_readiness_overlay": {
            "path": str(NEXTJS_READINESS_OVERLAY.relative_to(ROOT)),
            "manifest_sha256": manifest_sha256(NEXTJS_READINESS_OVERLAY),
            "contents": ["package.json", "tests/label.test.mjs", "README.md"],
        },
        "nextjs_port_binding": (
            "goal plus package scripts.dev and scripts.start bind port 4185 before "
            "either arm snapshot"
        ),
        "nextjs_verification_strengthening": (
            "node tests/label.test.mjs is a new registered assertion-bearing command"
        ),
        "endpoint_change": (
            "the route/reproducer target is unchanged; the Next.js endpoint is "
            "strictly stronger because it additionally runs the registered test"
        ),
    }
    for scenario in contract["scenarios"]:
        if scenario["scenario_id"] == "nextjs-fix":
            scenario["source_fixture"] = [
                "tests/fixtures/goal_verify_v4/a15/fix-nextjs-route-label/before",
                str(NEXTJS_READINESS_OVERLAY.relative_to(ROOT)),
            ]
    contract["nextjs_provisioning"]["copy_policy"] = (
        "fresh private symlink-preserving copy before each A28 arm"
    )
    contract["analysis"].update(
        {
            "pilot_generation": "A28 forward-only instrument correction",
            "a27_reuse_policy": (
                "no A27 arm, endpoint, report result, or descriptive contrast is "
                "reused in A28"
            ),
            "go_next_action": (
                "request repository-owner review before preregistering or collecting "
                "a separate confirmatory conditional-effect experiment"
            ),
            "invalid_policy": (
                "freeze all A28 evidence, diagnose forward-only, and never replace, "
                "rerun, rescore, resize, or pool an A28 arm"
            ),
        }
    )
    contract["estimand"].update(
        {
            "confirmatory_effect_estimate_in_a28": False,
            "population": (
                "the three frozen A28 fixtures at the scripted, reproducible failure "
                "boundary only"
            ),
        }
    )
    contract["authoritative_command"] = (
        "scripts/eval-goal-verify-recovery-deterministic-pair.py --contract "
        "eval/goal_verify/v0/phase6-recovery-deterministic-a28-pilot-contract.json "
        "--commandagent-bin <PINNED_BINARY> --nextjs-node-modules "
        "tests/fixtures/goal_verify_v3/create-ui-copy-style-port-path/reference/"
        "node_modules --run-dir dev-reports/issue-399/runs/"
        f"{CONTRACT_ID}"
    )
    contract["report_exit_semantics"] = (
        "zero iff all preregistered paired instrument checks pass; nonzero freezes "
        "A28 as INVALID and requires forward-only diagnosis"
    )
    return contract


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Freeze the A28 forward-only deterministic paired pilot"
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
