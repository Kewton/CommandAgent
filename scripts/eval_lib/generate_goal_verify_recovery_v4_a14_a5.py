from __future__ import annotations

import argparse
import copy
import hashlib
import json
from typing import Any

from eval_lib.generate_goal_verify_main_v4 import _load, _write_json
from eval_lib.generate_goal_verify_recovery_v4_a14_a2 import ROOT
from eval_lib.generate_goal_verify_recovery_v4_a14_a4 import (
    _build_contract as _build_a14_a4_contract,
)

EVAL = ROOT / "eval/goal_verify/v0"
CONTRACT_PATH = EVAL / "phase6-recovery-v4-a14-a5-contract.json"
SOURCE_ADAPTERS = EVAL / "phase6-command-adapters-v4-a14-a2.json"
ADAPTERS_PATH = EVAL / "phase6-command-adapters-v4-a14-a5.json"

CONTRACT_ID = "phase6-recovery-v4-20260830-a14-a5-live-01"
SMOKE_ID = "phase6-recovery-v4-20260830-a14-a5-smoke-01"
REFERENCE_ROOT = (
    "tests/fixtures/goal_verify_v4_a14_a5/"
    "create-ui-copy-style-port-path"
)
REFERENCE_FILES = [
    "reference/app/layout.js",
    "reference/app/page.js",
    "reference/app/play-01/page.js",
    "reference/next.config.js",
    "reference/package.json",
]


def _build_adapters() -> dict[str, Any]:
    value = copy.deepcopy(_load(SOURCE_ADAPTERS))
    for adapter in value["adapters"]:
        executor = adapter.get("executor", {})
        if executor.get("kind") != "playwright_script":
            continue
        for check in executor.get("observation", {}).get("checks", []):
            if check.get("computed") != "background-color":
                continue
            expected = check.get("expected_any", [])
            if "blue" in expected and "rgb(0, 0, 255)" not in expected:
                expected.append("rgb(0, 0, 255)")
    value["schema_version"] = "commandagent.goal_verify.adapters.v4_a14_a5"
    return value


def _build_contract(
    *,
    status: str,
    code_sha: str,
    exact_sha_ci_evidence: str,
    live_collection_authorized: bool,
) -> dict[str, Any]:
    contract = _build_a14_a4_contract(
        status=status,
        code_sha=code_sha,
        exact_sha_ci_evidence=exact_sha_ci_evidence,
        live_collection_authorized=live_collection_authorized,
    )
    contract.update(
        {
            "schema_version": (
                "commandagent.goal_verify.recovery_experiment.v4_a14_a5"
            ),
            "contract_id": CONTRACT_ID,
            "smoke_run_id": SMOKE_ID,
            "supersedes_contract": (
                "phase6-recovery-v4-20260830-a14-a4-live-01"
            ),
            "supersedes_smoke_run": (
                "phase6-recovery-v4-20260830-a14-a4-smoke-01"
            ),
        }
    )
    contract["pre_live_amendments"].append(
        {
            "amendment_id": "v4-A14-A5",
            "reason": (
                "A14-A4 kept Recovery harm at zero but conflated candidate server "
                "failure with browser-oracle executability and rejected a "
                "preregistered configured-one/runtime-excluded executed-zero pair"
            ),
            "historical_run_policy": (
                "A14-A4 smoke-01 remains immutable NO-GO evidence and is never "
                "rescored into A14-A5 evidence"
            ),
            "inference_role": (
                "measurement-instrument correction only; effect claim remains forbidden"
            ),
            "instrument_findings": [
                "browser executability was checked on the candidate artifact instead of a frozen reference workspace",
                "the inherited reference route /play did not match the A14 registered route /play-01",
                "the A14 adapter dropped the browser-normalized rgb(0, 0, 255) expected value",
                "the registered browser command omitted executed=true after an actual process execution",
                "runtime dependency exclusion can only be known after a preregistered configured-one shared run starts",
                "maximum-one execution was incorrectly aliased to configuration validity",
            ],
        }
    )
    contract["analysis"].update(
        {
            "browser_executability_preflight_source": (
                "frozen reference workspace, never candidate artifact"
            ),
            "runtime_exclusion_allows_configured_one_executed_zero": True,
            "candidate_server_failure_is_product_outcome_not_instrument_failure": True,
        }
    )
    contract["authorization"]["approved_at"] = (
        "2026-08-30" if live_collection_authorized else None
    )
    contract["smoke"].update(
        {
            "require_separate_browser_oracle_preflight": True,
            "browser_oracle_gate_source": "oracle-executability-preflight.json",
            "effect_claim_allowed": False,
        }
    )
    adapter_registry = _build_adapters()
    contract["frozen_external_oracles"] = str(ADAPTERS_PATH.relative_to(ROOT))
    contract["frozen_input_sha256"][str(ADAPTERS_PATH.relative_to(ROOT))] = (
        hashlib.sha256(
            (
                json.dumps(
                    adapter_registry, ensure_ascii=False, indent=2, sort_keys=True
                )
                + "\n"
            ).encode()
        ).hexdigest()
    )
    reference_hashes = {
        relative: hashlib.sha256(
            (ROOT / REFERENCE_ROOT / relative).read_bytes()
        ).hexdigest()
        for relative in REFERENCE_FILES
    }
    contract["oracle_executability_preflight"] = {
        "candidate_visible": False,
        "passed_to_product_or_recovery": False,
        "reference_overrides": {
            "create-ui-copy-style-port-path": {
                "root": f"{REFERENCE_ROOT}/",
                "stage": "reference",
                "tracked_files": REFERENCE_FILES,
                "frozen_file_sha256": reference_hashes,
                "reuse_provisioning_from": "create-ui-copy-style-port-path",
            }
        },
    }
    for relative, digest in reference_hashes.items():
        contract["frozen_input_sha256"][f"{REFERENCE_ROOT}/{relative}"] = digest
    contract["runner_sources"].append(
        "scripts/eval_lib/generate_goal_verify_recovery_v4_a14_a5.py"
    )
    return contract


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate A14-A5 Recovery instrument-correction contract"
    )
    parser.add_argument("--code-sha")
    parser.add_argument("--exact-sha-ci-evidence")
    parser.add_argument("--smoke-collection-authorized", action="store_true")
    args = parser.parse_args()
    if bool(args.code_sha) != bool(args.exact_sha_ci_evidence):
        parser.error("--code-sha and --exact-sha-ci-evidence must be paired")
    if args.smoke_collection_authorized and not args.code_sha:
        parser.error("smoke authorization requires exact-SHA inputs")
    adapter_registry = _build_adapters()
    contract = _build_contract(
        status="frozen" if args.code_sha else "draft",
        code_sha=args.code_sha or "",
        exact_sha_ci_evidence=args.exact_sha_ci_evidence or "",
        live_collection_authorized=args.smoke_collection_authorized,
    )
    _write_json(ADAPTERS_PATH, adapter_registry)
    _write_json(CONTRACT_PATH, contract)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
