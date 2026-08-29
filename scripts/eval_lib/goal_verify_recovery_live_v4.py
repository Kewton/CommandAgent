from __future__ import annotations

import json
import subprocess
import urllib.request
from pathlib import Path
from typing import Any

from eval_lib.goal_verify_additive_v4 import (
    candidate_visible_manifest,
    workspace_manifest,
)
from eval_lib.goal_verify_baseline_product_v3 import run_current_product_baseline
from eval_lib.goal_verify_live import (
    _acquire_run_lock,
    _append_record_ledger,
    _atomic_json,
    _load_record_ledger,
    load_json,
    sha256_file,
)
from eval_lib.goal_verify_main_design_v4 import product_stage, workspace_case_id
from eval_lib.goal_verify_preflight_v3 import exact_sha_ci_evidence_errors
from eval_lib.goal_verify_recovery_experiment_v4 import (
    artifact_delta,
    classify_case_recovery_eligibility,
    classify_initial_recovery_eligibility,
    compare_recovery_arms,
    execute_frozen_external_oracles,
    recovery_contract_errors,
)
from eval_lib.goal_verify_task_contracts_v4 import (
    bind_task_contract,
    load_task_contract_registry,
)
from eval_lib.goal_verify_workspaces_v3 import (
    prepare_workspace_stage,
    workspace_by_case,
)
from eval_lib.goal_verify_workspaces_v4 import load_v4_workspace_registry


def run_recovery_pair(
    *,
    root: Path,
    contract: dict[str, Any],
    case: dict[str, Any],
    task_contract: dict[str, Any],
    workspace_contract: dict[str, Any],
    pair_id: str,
    execution_root: Path,
    namespace: str,
    commandagent_bin: Path,
    adapters: list[dict[str, Any]],
    baseline_runner=run_current_product_baseline,
    oracle_executor=None,
) -> dict[str, Any]:
    execution_action = contract["paired_run_contract"].get(
        "execution_action", "plan_run"
    )
    preregistered = classify_case_recovery_eligibility(
        task_contract=task_contract, adapters=adapters
    )
    frozen = contract["recovery_eligibility"]["preregistered_smoke_cases"].get(
        case["case_id"]
    )
    if frozen != preregistered:
        raise ValueError(f"recovery eligibility drift:{case['case_id']}")
    initial_workspace, initial_input = _prepare_arm_workspace(
        root=root,
        case=case,
        workspace_contract=workspace_contract,
        destination=execution_root / namespace / f"{pair_id}--initial-only",
        provisioned_root=execution_root / "provisioned",
    )
    initial_result = baseline_runner(
        commandagent_bin=commandagent_bin,
        workspace=initial_workspace,
        case=case,
        model=contract["model"],
        timeout_sec=int(contract["product_timeout_sec"]),
        completion_contract=case.get("task_contract", {}).get("completion_contract"),
        recovery_plan_auto_runs=0,
        execution_action=execution_action,
    )
    initial_output_full = workspace_manifest(initial_workspace)
    initial_output_artifact = candidate_visible_manifest(initial_output_full)
    initial_oracles = execute_frozen_external_oracles(
        case_id=case["case_id"],
        adapters=adapters,
        workspace=initial_workspace,
        **({"executor": oracle_executor} if oracle_executor is not None else {}),
    )
    runtime_eligibility = classify_initial_recovery_eligibility(
        preregistered=preregistered, baseline=initial_result
    )
    record: dict[str, Any] = {
        "schema_version": "commandagent.goal_verify.recovery_pair.v4_a14",
        "pair_id": pair_id,
        "case_id": case["case_id"],
        "profile": case["profile"],
        "intent": case["intent"],
        "eligibility": {
            "preregistered": preregistered,
            "runtime": runtime_eligibility,
        },
        "initial_only": {
            "input_manifest": initial_input,
            "result": initial_result,
            "output_full_manifest": initial_output_full,
            "output_artifact_manifest": initial_output_artifact,
            "input_to_output_artifact_delta": artifact_delta(
                candidate_visible_manifest(initial_input), initial_output_artifact
            ),
            "external_oracles": initial_oracles,
        },
        "recovery_one": {
            "status": "not_run",
            "reason": runtime_eligibility["reason"],
        },
        "comparison": None,
    }
    if runtime_eligibility["run_recovery_one_arm"] is not True:
        return record

    recovery_workspace, recovery_input = _prepare_arm_workspace(
        root=root,
        case=case,
        workspace_contract=workspace_contract,
        destination=execution_root / namespace / f"{pair_id}--recovery-one",
        provisioned_root=execution_root / "provisioned",
    )
    if recovery_input["snapshot_sha256"] != initial_input["snapshot_sha256"]:
        raise ValueError(f"paired input snapshot mismatch:{pair_id}")
    recovery_result = baseline_runner(
        commandagent_bin=commandagent_bin,
        workspace=recovery_workspace,
        case=case,
        model=contract["model"],
        timeout_sec=int(contract["product_timeout_sec"]),
        completion_contract=case.get("task_contract", {}).get("completion_contract"),
        recovery_plan_auto_runs=1,
        execution_action=execution_action,
    )
    recovery_output_full = workspace_manifest(recovery_workspace)
    recovery_output_artifact = candidate_visible_manifest(recovery_output_full)
    recovery_oracles = execute_frozen_external_oracles(
        case_id=case["case_id"],
        adapters=adapters,
        workspace=recovery_workspace,
        **({"executor": oracle_executor} if oracle_executor is not None else {}),
    )
    record["recovery_one"] = {
        "status": "completed",
        "input_manifest": recovery_input,
        "result": recovery_result,
        "output_full_manifest": recovery_output_full,
        "output_artifact_manifest": recovery_output_artifact,
        "input_to_output_artifact_delta": artifact_delta(
            candidate_visible_manifest(recovery_input), recovery_output_artifact
        ),
        "external_oracles": recovery_oracles,
    }
    record["comparison"] = compare_recovery_arms(
        initial_only=initial_result,
        recovery_one=recovery_result,
        initial_oracles=initial_oracles,
        recovery_oracles=recovery_oracles,
        initial_artifact_manifest=initial_output_artifact,
        recovery_artifact_manifest=recovery_output_artifact,
    )
    return record


def run_recovery_smoke(
    *,
    root: Path,
    contract_path: Path,
    run_dir: Path,
    execution_root: Path,
    commandagent_bin: Path,
    limit: int | None = None,
) -> dict[str, Any]:
    contract = load_json(contract_path)
    _verify_frozen_inputs(
        root=root,
        contract=contract,
        execution_root=execution_root,
        commandagent_bin=commandagent_bin,
    )
    namespace = contract["smoke_run_id"]
    if run_dir.name != namespace:
        raise ValueError("smoke run directory differs from frozen smoke_run_id")
    corpus = load_json(root / contract["corpus"])
    corpus_by_id = {row["case_id"]: row for row in corpus["cases"]}
    task_registry = load_task_contract_registry(
        root / contract["task_contract_registry"]
    )
    task_by_id = {row["case_id"]: row for row in task_registry["cases"]}
    adapters = load_json(root / contract["frozen_external_oracles"])["adapters"]
    workspaces = workspace_by_case(
        load_v4_workspace_registry(root=root, contract=contract)
    )
    selected_pair_ids = contract["smoke"]["selected_pair_ids"]
    if limit is not None:
        selected_pair_ids = selected_pair_ids[:limit]
    manifest = {
        "schema_version": "commandagent.goal_verify.recovery_campaign.v4_a14",
        "contract_id": contract["contract_id"],
        "contract_sha256": sha256_file(contract_path),
        "code_sha": contract["code_sha"],
        "run_id": namespace,
        "execution_root": str(execution_root.resolve()),
        "selected_pair_ids": selected_pair_ids,
        "target_pairs": len(selected_pair_ids),
        "commandagent_bin": str(commandagent_bin.resolve()),
    }
    run_dir.mkdir(parents=True, exist_ok=True)
    manifest_path = run_dir / "campaign-manifest.json"
    if manifest_path.exists() and load_json(manifest_path) != manifest:
        raise ValueError("recovery campaign manifest differs from frozen inputs")
    if not manifest_path.exists():
        _atomic_json(manifest_path, manifest)
    lock = _acquire_run_lock(run_dir, contract["integrity"]["exclusive_run_lock"])
    try:
        ledger_path = run_dir / contract["integrity"]["record_ledger"]
        entries, ledger_head = _load_record_ledger(
            root=root, run_dir=run_dir, ledger_path=ledger_path
        )
        for completed, pair_id in enumerate(selected_pair_ids, 1):
            case_id = pair_id.removesuffix("--pair-01")
            if case_id not in corpus_by_id or pair_id != f"{case_id}--pair-01":
                raise ValueError(f"invalid frozen recovery pair ID:{pair_id}")
            relative = Path("raw") / case_id / "pair-01.json"
            record_path = run_dir / relative
            reference = str(record_path.relative_to(root))
            if not record_path.exists():
                case = bind_task_contract(corpus_by_id[case_id], task_registry)
                workspace_id = workspace_case_id(case)
                record = run_recovery_pair(
                    root=root,
                    contract=contract,
                    case=case,
                    task_contract=task_by_id[case_id],
                    workspace_contract=workspaces[workspace_id],
                    pair_id=pair_id,
                    execution_root=execution_root,
                    namespace=namespace,
                    commandagent_bin=commandagent_bin,
                    adapters=adapters,
                )
                _atomic_json(
                    record_path,
                    {
                        **record,
                        "source_case_id": case_id,
                        "record_path": reference,
                    },
                )
                ledger_head = _append_record_ledger(
                    ledger_path=ledger_path,
                    entries=entries,
                    previous=ledger_head,
                    pair_id=pair_id,
                    source_case_id=case_id,
                    record_reference=reference,
                    record_path=record_path,
                )
            _atomic_json(
                run_dir / "campaign-summary.json",
                {
                    "schema_version": (
                        "commandagent.goal_verify.recovery_summary.v4_a14"
                    ),
                    "contract_id": contract["contract_id"],
                    "run_id": namespace,
                    "completed_pairs": completed,
                    "target_pairs": len(selected_pair_ids),
                    "record_ledger_head_sha256": ledger_head,
                },
            )
        return load_json(run_dir / "campaign-summary.json")
    finally:
        lock.close()


def _prepare_arm_workspace(
    *,
    root: Path,
    case: dict[str, Any],
    workspace_contract: dict[str, Any],
    destination: Path,
    provisioned_root: Path,
) -> tuple[Path, dict[str, Any]]:
    stage = product_stage(case)
    workspace = prepare_workspace_stage(
        root=root,
        workspace=workspace_contract,
        stage=stage,
        destination=destination / stage,
        provisioned_root=provisioned_root,
    )
    return workspace, workspace_manifest(workspace)


def _verify_frozen_inputs(
    *,
    root: Path,
    contract: dict[str, Any],
    execution_root: Path,
    commandagent_bin: Path,
) -> None:
    contract_errors = recovery_contract_errors(contract)
    if contract_errors:
        raise ValueError("A14 recovery contract invalid:" + ",".join(contract_errors))
    if contract.get("status") != "frozen":
        raise ValueError("A14 recovery contract is not frozen")
    if contract.get("authorization", {}).get("smoke_collection_authorized") is not True:
        raise ValueError("A14 recovery smoke is not authorized")
    required_root = Path(contract["execution_root_policy"]["required_root"]).resolve()
    if execution_root.resolve() != required_root:
        raise ValueError("A14 recovery execution root must be the frozen SSD root")
    code_sha = contract.get("code_sha")
    if not isinstance(code_sha, str) or len(code_sha) != 40:
        raise ValueError("A14 frozen code SHA is invalid")
    unchanged = subprocess.run(
        ["git", "diff", "--quiet", code_sha, "--", *contract["runner_sources"]],
        cwd=root,
        check=False,
    )
    if unchanged.returncode != 0:
        raise ValueError("A14 frozen runner source differs from code SHA")
    ci_errors = exact_sha_ci_evidence_errors(root=root, contract=contract)
    if ci_errors:
        raise ValueError("A14 exact-SHA CI invalid:" + ",".join(ci_errors))
    version = subprocess.run(
        [str(commandagent_bin.resolve()), "--version"],
        text=True,
        capture_output=True,
        check=False,
    )
    if (
        version.returncode != 0
        or code_sha[:8] not in version.stdout.split()
        or "+dirty" in version.stdout
    ):
        raise ValueError("A14 commandagent binary does not match frozen code SHA")
    tags_endpoint = contract["endpoint"].removesuffix("/api/generate") + "/api/tags"
    with urllib.request.urlopen(tags_endpoint, timeout=30) as response:
        tags = json.loads(response.read().decode())
    matching = [
        model
        for model in tags.get("models", [])
        if model.get("name") == contract["model"]
    ]
    if len(matching) != 1 or matching[0].get("digest") != contract["model_digest"]:
        raise ValueError("A14 local model digest differs from frozen contract")
