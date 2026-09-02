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

A28_CONTRACT_PATH = (
    ROOT / "eval/goal_verify/v0/phase6-recovery-deterministic-a28-pilot-contract.json"
)
CONTRACT_PATH = (
    ROOT / "eval/goal_verify/v0/phase6-recovery-deterministic-a29-pilot-contract.json"
)
CONTRACT_ID = "phase6-recovery-deterministic-20260902-a29-pilot-01"
A28_CONTRACT_ID = "phase6-recovery-deterministic-20260902-a28-pilot-01"
A28_REPORT_SHA256 = "efd948481c3e9df3a897012ade507664e29243924d6df1d467f7a6c56ba36477"


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
    historical = _load(A28_CONTRACT_PATH)
    if (
        historical.get("contract_id") != A28_CONTRACT_ID
        or historical.get("status") != "frozen"
        or historical.get("effect_claim_allowed") is not False
    ):
        raise ValueError("unexpected A28 historical contract")
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
            "supersedes_contract": A28_CONTRACT_ID,
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
            "A28": (
                "immutable INVALID deterministic paired pilot; never rerun, "
                "rescore, replace, or pool any arm"
            ),
        }
    )
    contract["a28_diagnosis"] = {
        "frozen_report_sha256": A28_REPORT_SHA256,
        "valid_pairs": ["generic-fix", "data-fix"],
        "nextjs_registered_endpoint_contrast": 1,
        "nextjs_product_recovery_succeeded": True,
        "false_nextjs_arm_checks": [
            "protected_paths_unchanged",
            "scripted_read_write_sequence",
        ],
        "root_causes": [
            (
                "the readiness package.json was semantically complete but not in "
                "serde_json canonical key order, so deterministic post-step repair "
                "created a formatting-only byte delta"
            ),
            (
                "the scripted provider matched an obsolete initial-inspection prompt "
                "substring and therefore emitted Write without the preregistered "
                "preceding Read trace"
            ),
        ],
        "product_recovery_defect_established": False,
    }
    contract["forward_corrections"] = {
        "nextjs_package_serialization": (
            "preformat package.json in the same recursively key-sorted pretty JSON "
            "form produced by the deterministic profile repair"
        ),
        "scripted_provider_sequence": (
            "the initial nextjs_initial execution deterministically emits Read for "
            "lib/label.mjs before the Recovery repair emits Write"
        ),
        "sequence_check": (
            "scripted_read_write_sequence requires an observed Read index strictly "
            "before an observed Write index"
        ),
        "nextjs_readiness_overlay": {
            "path": str(NEXTJS_READINESS_OVERLAY.relative_to(ROOT)),
            "manifest_sha256": manifest_sha256(NEXTJS_READINESS_OVERLAY),
        },
        "scope_change": (
            "none; retain all A28 scenarios, arms, registered commands, endpoints, "
            "port binding, and order"
        ),
    }
    for scenario in contract["scenarios"]:
        if scenario["scenario_id"] == "nextjs-fix":
            scenario["source_fixture"] = [
                "tests/fixtures/goal_verify_v4/a15/fix-nextjs-route-label/before",
                str(NEXTJS_READINESS_OVERLAY.relative_to(ROOT)),
            ]
    contract["nextjs_provisioning"]["copy_policy"] = (
        "fresh private symlink-preserving copy before each A29 arm"
    )
    contract["analysis"].update(
        {
            "pilot_generation": "A29 forward-only instrument correction",
            "a28_reuse_policy": (
                "no A28 arm, endpoint, report result, or descriptive contrast is "
                "reused in A29"
            ),
            "go_next_action": (
                "request repository-owner review before preregistering or collecting "
                "a separate confirmatory conditional-effect experiment"
            ),
            "invalid_policy": (
                "freeze all A29 evidence, diagnose forward-only, and never replace, "
                "rerun, rescore, resize, or pool an A29 arm"
            ),
        }
    )
    contract["estimand"].update(
        {
            "confirmatory_effect_estimate_in_a29": False,
            "population": (
                "the three frozen A29 fixtures at the scripted, reproducible failure "
                "boundary only"
            ),
        }
    )
    contract["authoritative_command"] = (
        "scripts/eval-goal-verify-recovery-deterministic-pair.py --contract "
        "eval/goal_verify/v0/phase6-recovery-deterministic-a29-pilot-contract.json "
        "--commandagent-bin <PINNED_BINARY> --nextjs-node-modules "
        "tests/fixtures/goal_verify_v3/create-ui-copy-style-port-path/reference/"
        "node_modules --run-dir dev-reports/issue-399/runs/"
        f"{CONTRACT_ID}"
    )
    contract["report_exit_semantics"] = (
        "zero iff all preregistered paired instrument checks pass; nonzero freezes "
        "A29 as INVALID and requires forward-only diagnosis"
    )
    return contract


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Freeze the A29 forward-only deterministic paired pilot"
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
