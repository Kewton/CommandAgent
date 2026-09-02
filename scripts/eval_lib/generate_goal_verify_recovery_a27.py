from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any

from eval_lib import generate_goal_verify_recovery_a26 as a26_generator
from eval_lib.generate_goal_verify_main_v4 import _load, _write_json
from eval_lib.generate_goal_verify_recovery_v4_a14_a2 import ROOT

A26_CONTRACT_PATH = (
    ROOT / "eval/goal_verify/v0/phase6-recovery-deterministic-a26-pilot-contract.json"
)
CONTRACT_PATH = (
    ROOT / "eval/goal_verify/v0/phase6-recovery-deterministic-a27-pilot-contract.json"
)
CONTRACT_ID = "phase6-recovery-deterministic-20260902-a27-pilot-01"
A26_CONTRACT_ID = "phase6-recovery-deterministic-20260902-a26-pilot-01"
A26_REPORT_SHA256 = "ac69639de1f160cef7c9a35236873909ba91b2feff43454accc9ae5973962a84"
RUNTIME_CACHE_EXCLUSIONS = (
    ".anvil",
    ".commandagent",
    ".commandagent-state",
    ".goal-verify-tools",
    ".next",
    ".pytest_cache",
    "__pycache__",
    "node_modules",
)


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
    historical = _load(A26_CONTRACT_PATH)
    if (
        historical.get("contract_id") != A26_CONTRACT_ID
        or historical.get("status") != "frozen"
        or historical.get("effect_claim_allowed") is not False
    ):
        raise ValueError("unexpected A26 historical contract")
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
            "supersedes_contract": A26_CONTRACT_ID,
            "generator_source_sha256": _generator_sha256(),
        }
    )
    contract["historical_evidence_policy"]["A26"] = (
        "immutable INVALID deterministic paired pilot; never rerun, rescore, replace, "
        "or pool any arm"
    )
    contract["a26_diagnosis"] = {
        "frozen_report_sha256": A26_REPORT_SHA256,
        "false_instrument_checks": [
            "data protected_paths_unchanged",
            "Next.js registered build/route endpoint and Recovery execution",
        ],
        "root_causes": [
            (
                "protected-path manifest counted pytest-generated __pycache__ while "
                "the input manifest excluded runtime caches"
            ),
            (
                "default shutil.copytree dereferenced node_modules/.bin/next and "
                "changed the relative module-resolution base"
            ),
        ],
        "product_recovery_defect_established": False,
    }
    contract["forward_corrections"] = {
        "protected_manifest_runtime_cache_exclusions": list(RUNTIME_CACHE_EXCLUSIONS),
        "nextjs_provisioning_symlink_policy": (
            "copy every hash-bound node_modules entry with symlinks=True and preserve "
            "the relative .bin/next link target"
        ),
        "scope_change": "none; retain the A26 scenarios, arms, endpoints, and order",
    }
    contract["nextjs_provisioning"]["copy_policy"] = (
        "fresh private symlink-preserving copy before each A27 arm"
    )
    contract["analysis"].update(
        {
            "pilot_generation": "A27 forward-only instrument correction",
            "a26_reuse_policy": (
                "no A26 arm, endpoint, report result, or descriptive contrast is "
                "reused in A27"
            ),
            "go_next_action": (
                "request repository-owner review before preregistering or collecting "
                "a separate confirmatory conditional-effect experiment"
            ),
            "invalid_policy": (
                "freeze all A27 evidence, diagnose forward-only, and never replace, "
                "rerun, rescore, resize, or pool an A27 arm"
            ),
        }
    )
    contract["estimand"]["confirmatory_effect_estimate_in_a27"] = contract["estimand"][
        "confirmatory_effect_estimate_in_a26"
    ]
    contract["estimand"]["population"] = (
        "the three frozen A27 fixtures, scope-identical to A26, at the scripted, "
        "reproducible failure boundary only"
    )
    contract["authoritative_command"] = (
        "scripts/eval-goal-verify-recovery-deterministic-pair.py --contract "
        "eval/goal_verify/v0/phase6-recovery-deterministic-a27-pilot-contract.json "
        "--commandagent-bin <PINNED_BINARY> --nextjs-node-modules "
        "tests/fixtures/goal_verify_v3/create-ui-copy-style-port-path/reference/"
        "node_modules --run-dir dev-reports/issue-399/runs/"
        f"{CONTRACT_ID}"
    )
    contract["report_exit_semantics"] = (
        "zero iff all preregistered paired instrument checks pass; nonzero freezes "
        "A27 as INVALID and requires forward-only diagnosis"
    )
    return contract


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Freeze the A27 forward-only deterministic paired pilot"
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
