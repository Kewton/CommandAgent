from __future__ import annotations

import copy
import json
import shutil
import time
from pathlib import Path
from typing import Any

import jsonschema

from eval_lib.goal_verify_additive_v4 import (
    candidate_visible_manifest,
    combine_evaluations,
    evaluate_candidate_spec_v4,
    score_candidate_outcomes,
    workspace_manifest,
)
from eval_lib.goal_verify_live import (
    _acquire_run_lock,
    _append_record_ledger,
    _atomic_json,
    _load_record_ledger,
    load_json,
    request_ollama,
    sha256_file,
    validate_proposal,
)
from eval_lib.goal_verify_live_v3 import (
    _prepare_pair_stages,
    _run_baseline,
    verify_live_inputs_v3,
)
from eval_lib.goal_verify_main_design_v4 import (
    cluster_metadata,
    product_stage,
    workspace_case_id,
)
from eval_lib.goal_verify_next_v4 import (
    candidate_visible_executor_capabilities,
    detect_executor_capabilities,
)
from eval_lib.goal_verify_observation_match_v3 import score_claim_coverage
from eval_lib.goal_verify_preflight_v4 import readiness_report
from eval_lib.goal_verify_repairs_v4 import apply_meaning_preserving_repairs
from eval_lib.goal_verify_task_contracts_v4 import (
    bind_existing_evidence_registry,
    bind_task_contract,
    load_task_contract_registry,
)
from eval_lib.goal_verify_v2 import normalize_v2_proposal
from eval_lib.goal_verify_v3 import (
    LANES,
    build_conformance_prompt,
    build_held_out_prompt,
    canonicalize_held_out_proposal,
    load_prompt_from_contract,
    regeneration_seed,
    should_regenerate,
)
from eval_lib.goal_verify_workspaces_v3 import workspace_by_case
from eval_lib.goal_verify_workspaces_v4 import load_v4_workspace_registry


def _cluster_manifest(
    *,
    selected: list[dict[str, Any]],
    samples_per_task: int,
    selected_pair_ids: list[str],
) -> dict[str, Any]:
    if not any("source_task_id" in case or "cell_id" in case for case in selected):
        return {}
    population_tasks: set[str] = set()
    population_cells: set[str] = set()
    selected_tasks: set[str] = set()
    selected_cells: set[str] = set()
    selected_pair_set = set(selected_pair_ids)
    for case in selected:
        metadata = cluster_metadata(case)
        population_tasks.add(metadata["source_task_id"])
        population_cells.add(metadata["cell_id"])
        for sample_index in range(1, samples_per_task + 1):
            pair_id = f"{case['case_id']}--pair-{sample_index:02d}"
            if pair_id in selected_pair_set:
                selected_tasks.add(metadata["source_task_id"])
                selected_cells.add(metadata["cell_id"])
    return {
        "cluster_design": {
            "cluster_unit": "source_task_id",
            "population_cell_count": len(population_cells),
            "population_source_task_count": len(population_tasks),
            "runs_per_source_task": samples_per_task,
            "population_pair_count": len(population_tasks) * samples_per_task,
            "selected_cell_count": len(selected_cells),
            "selected_source_task_count": len(selected_tasks),
            "selected_pair_count": len(selected_pair_ids),
        }
    }


def run_campaign_v4(
    *,
    root: Path,
    corpus_path: Path,
    contract_path: Path,
    schema_path: Path,
    prompt_path: Path | None,
    validator: Path,
    run_dir: Path,
    execution_root: Path,
    commandagent_bin: Path | None = None,
    limit: int | None = None,
    provider=request_ollama,
    baseline_runner=None,
    pair_ids: list[str] | None = None,
    request_namespace: str | None = None,
) -> dict[str, Any]:
    contract = load_json(contract_path)
    namespace = _campaign_namespace(
        contract=contract,
        pair_ids=pair_ids,
        request_namespace=request_namespace,
        limit=limit,
    )
    configured_corpus = contract.get("corpus")
    if (
        configured_corpus is not None
        and corpus_path.resolve() != (root / configured_corpus).resolve()
    ):
        raise ValueError("v4 corpus path differs from contract.corpus")
    expected_schema = (
        root / contract["generation"]["structured_output_schema"]
    ).resolve()
    if schema_path.resolve() != expected_schema:
        raise ValueError(
            "v4 schema path differs from contract.generation.structured_output_schema"
        )
    readiness = readiness_report(
        root=root, contract_path=contract_path, execution_root=execution_root
    )
    if not readiness["ready"]:
        raise ValueError(
            "v4 preflight is not ready: " + ",".join(readiness["blockers"])
        )
    commandagent = (commandagent_bin or root / "target/release/commandagent").resolve()
    live_inputs = verify_live_inputs_v3(
        root=root,
        contract=contract,
        commandagent_bin=commandagent,
        validator=validator,
    )
    if run_dir.name != namespace:
        raise ValueError("request namespace differs from run directory basename")
    resolved_prompt, base_prompt = load_prompt_from_contract(
        root=root, contract=contract, cli_prompt=prompt_path
    )
    corpus = load_json(corpus_path)
    schema = load_json(schema_path)
    canonical_schema_path = contract["generation"].get("canonical_schema")
    canonical_schema = (
        load_json(root / canonical_schema_path) if canonical_schema_path else schema
    )
    adapters = load_json(root / contract["scoring"]["answer_key"])["adapters"]
    capabilities = load_json(root / contract["capability_registry"])
    workspaces = workspace_by_case(
        load_v4_workspace_registry(root=root, contract=contract)
    )
    selected_ids = [row["case_id"] for row in contract["selected_cells"]]
    corpus_by_case = {row["case_id"]: row for row in corpus["cases"]}
    task_contract_path = contract.get("task_contract_registry")
    task_contracts = (
        load_task_contract_registry(root / task_contract_path)
        if task_contract_path
        else None
    )
    selected = [
        (
            bind_task_contract(corpus_by_case[case_id], task_contracts)
            if task_contracts is not None
            else corpus_by_case[case_id]
        )
        for case_id in selected_ids
    ]
    selected_pair_ids = _select_campaign_pair_ids(
        selected=selected,
        samples_per_cell=int(contract["samples_per_cell"]),
        pair_ids=pair_ids,
    )
    selected_pair_set = set(selected_pair_ids)
    shapes = {
        intent: (root / path).read_text(encoding="utf-8")
        for intent, path in contract["generation"]["shape_examples"].items()
    }
    workspace_additions = contract.get("workspace_registry_additions")
    manifest = {
        "schema_version": "commandagent.goal_verify.phase6_live_manifest.v4",
        "contract_id": contract["contract_id"],
        "contract_sha256": sha256_file(contract_path),
        "code_sha": contract["code_sha"],
        "corpus_sha256": sha256_file(corpus_path),
        **(
            {"task_contract_registry_sha256": sha256_file(root / task_contract_path)}
            if task_contract_path
            else {}
        ),
        "schema_sha256": sha256_file(schema_path),
        **(
            {"canonical_schema_sha256": sha256_file(root / canonical_schema_path)}
            if canonical_schema_path
            else {}
        ),
        "prompt_file_sha256": sha256_file(resolved_prompt),
        "answer_key_sha256": sha256_file(root / contract["scoring"]["answer_key"]),
        "workspace_registry_sha256": sha256_file(root / contract["workspace_registry"]),
        **(
            {
                "workspace_registry_additions_sha256": sha256_file(
                    root / workspace_additions
                )
            }
            if workspace_additions
            else {}
        ),
        "request_namespace": namespace,
        "campaign_role": "full" if pair_ids is None else "preregistered_smoke",
        "selected_pair_ids": selected_pair_ids,
        "target_pairs": len(selected_pair_ids),
        "target_proposals": len(selected_pair_ids) * len(LANES),
        **_cluster_manifest(
            selected=selected,
            samples_per_task=int(contract["samples_per_cell"]),
            selected_pair_ids=selected_pair_ids,
        ),
        "execution_root": str(execution_root.resolve()),
        "commandagent_bin": str(commandagent),
        **live_inputs,
    }
    manifest_path = run_dir / "campaign-manifest.json"
    if manifest_path.exists() and load_json(manifest_path) != manifest:
        raise ValueError("campaign manifest differs from frozen v4 inputs")
    if not manifest_path.exists():
        _atomic_json(manifest_path, manifest)
    lock = _acquire_run_lock(run_dir, contract["integrity"]["exclusive_run_lock"])
    try:
        ledger_path = run_dir / contract["integrity"]["record_ledger"]
        entries, ledger_head = _load_record_ledger(
            root=root, run_dir=run_dir, ledger_path=ledger_path
        )
        completed = 0
        target = manifest["target_pairs"]
        for source_index, case in enumerate(selected):
            for sample_index in range(1, int(contract["samples_per_cell"]) + 1):
                pair_id = f"{case['case_id']}--pair-{sample_index:02d}"
                if pair_id not in selected_pair_set:
                    continue
                if limit is not None and completed >= limit:
                    return _summary(run_dir, completed, target, ledger_head)
                relative = (
                    Path("raw") / case["case_id"] / f"pair-{sample_index:02d}.json"
                )
                record_path = run_dir / relative
                reference = str(record_path.relative_to(root))
                if record_path.exists():
                    record = load_json(record_path)
                else:
                    pair_root = execution_root / namespace / pair_id
                    stage_paths = _prepare_pair_stages(
                        root=root,
                        case=case,
                        workspace=workspaces.get(workspace_case_id(case)),
                        destination=pair_root,
                    )
                    product_workspace = _product_workspace(case, stage_paths)
                    browser_toolchain = _freeze_browser_toolchain(
                        product_workspace, pair_root
                    )
                    frozen_before = _freeze_before(case, product_workspace, pair_root)
                    baseline = _run_baseline(
                        root=root,
                        contract=contract,
                        case=case,
                        pair_root=pair_root,
                        stage_paths=stage_paths,
                        adapters=adapters,
                        commandagent_bin=commandagent,
                        baseline_runner=baseline_runner or _default_baseline_runner(),
                    )
                    candidate_case = bind_existing_evidence_registry(
                        case, product_workspace
                    )
                    frozen_product = pair_root / "frozen-product"
                    _copy_workspace(product_workspace, frozen_product)
                    executor_capabilities = detect_executor_capabilities(
                        frozen_product, profile=case["profile"]
                    )
                    snapshot_manifests = {
                        "product": workspace_manifest(frozen_product),
                    }
                    if frozen_before is not None:
                        snapshot_manifests["before"] = workspace_manifest(frozen_before)
                    pair_index = (
                        source_index * int(contract["samples_per_cell"]) + sample_index
                    )
                    lanes = {
                        lane: _run_lane_v4(
                            root=root,
                            contract=contract,
                            schema=schema,
                            validator=validator,
                            provider=provider,
                            case=candidate_case,
                            pair_id=pair_id,
                            pair_index=pair_index,
                            lane=lane,
                            base_prompt=base_prompt,
                            shape=shapes[case["intent"]],
                            raw_schema=schema if canonical_schema_path else None,
                            canonical_schema=canonical_schema,
                            adapters=adapters,
                            capabilities=capabilities,
                            frozen_product=frozen_product,
                            frozen_before=frozen_before,
                            snapshot_manifests=snapshot_manifests,
                            browser_toolchain=browser_toolchain,
                            baseline=baseline,
                            pair_root=pair_root,
                            executor_capabilities=executor_capabilities,
                            request_namespace=namespace,
                        )
                        for lane in LANES
                    }
                    record = {
                        "schema_version": "commandagent.goal_verify.phase6_live_record.v4",
                        "pair_id": pair_id,
                        "source_case_id": case["case_id"],
                        **(
                            cluster_metadata(case)
                            if "cell_id" in case or "source_task_id" in case
                            else {}
                        ),
                        "goal": case["goal"],
                        "source_goal": case.get("source_goal", case["goal"]),
                        "intent": case["intent"],
                        "profile": case["profile"],
                        "required_claims": copy.deepcopy(case["required_claims"]),
                        "existing_evidence_registry": copy.deepcopy(
                            candidate_case.get("existing_evidence_registry", [])
                        ),
                        "record_path": reference,
                        "snapshot_manifests": snapshot_manifests,
                        "browser_toolchain_sha256": (
                            workspace_manifest(browser_toolchain)["snapshot_sha256"]
                            if browser_toolchain is not None
                            else None
                        ),
                        "executor_capabilities": executor_capabilities,
                        "request_namespace": namespace,
                        "baseline": baseline,
                        "lanes": lanes,
                    }
                    _atomic_json(record_path, record)
                    ledger_head = _append_record_ledger(
                        ledger_path=ledger_path,
                        entries=entries,
                        previous=ledger_head,
                        pair_id=pair_id,
                        source_case_id=case["case_id"],
                        record_reference=reference,
                        record_path=record_path,
                    )
                del record
                completed += 1
                _summary(run_dir, completed, target, ledger_head)
        return _summary(run_dir, completed, target, ledger_head)
    finally:
        lock.close()


def _run_lane_v4(
    *,
    root,
    contract,
    schema,
    validator,
    provider,
    case,
    pair_id,
    pair_index,
    lane,
    base_prompt,
    shape,
    raw_schema,
    canonical_schema,
    adapters,
    capabilities,
    frozen_product,
    frozen_before,
    snapshot_manifests,
    browser_toolchain,
    baseline,
    pair_root,
    executor_capabilities,
    request_namespace,
):
    started_ns = time.monotonic_ns()
    attempts = []
    validation = {"valid": False, "spec": None, "errors": ["not_attempted"]}
    visible = {
        stage: candidate_visible_manifest(manifest)
        for stage, manifest in snapshot_manifests.items()
    }
    for attempt in (1, 2):
        request_id = f"{request_namespace}:{pair_id}:{lane}:attempt-{attempt}"
        prompt = _build_prompt_v4(
            lane=lane,
            base_prompt=base_prompt,
            case=case,
            request_id=request_id,
            shape=shape,
            adapters=adapters,
            capabilities=capabilities,
            manifests=visible,
            executor_capabilities=executor_capabilities,
        )
        response = provider(
            endpoint=contract["endpoint"],
            model=contract["model"],
            prompt=prompt,
            schema=schema,
            seed=regeneration_seed(
                contract["generation"]["seed_base"], pair_index, lane, attempt
            ),
            temperature=float(contract["generation"]["temperature"]),
            num_predict=int(contract["generation"]["num_predict"]),
            timeout_sec=int(contract["generation"]["request_timeout_sec"]),
            keep_alive=str(contract["generation"]["keep_alive"]),
            think=bool(contract["generation"]["think"]),
        )
        normalized = None
        raw_schema_validation = {
            "applied": raw_schema is not None,
            "valid": None,
            "errors": [],
        }
        allow_unverifiable = (
            contract.get("claim_policy", {}).get("allow_unverifiable_reason") is True
        )
        if response.get("status") == "completed":
            raw = response["response"].get("response", "")
            if raw_schema is not None:
                raw_schema_validation = _validate_raw_proposal(
                    raw=raw, schema=raw_schema
                )
            if raw_schema_validation.get("valid") is False:
                validation = {
                    "valid": False,
                    "spec": None,
                    "errors": raw_schema_validation["errors"],
                }
            else:
                try:
                    normalized = (
                        normalize_v2_proposal(
                            raw,
                            case=case,
                            model=contract["model"],
                            request_id=request_id,
                            allow_unverifiable_claims=allow_unverifiable,
                        )
                        if lane == "contract_conformance"
                        else canonicalize_held_out_proposal(
                            raw,
                            case=case,
                            model=contract["model"],
                            request_id=request_id,
                            allow_unverifiable_claims=allow_unverifiable,
                        )
                    )
                    validation = _validate_proposal_v4(
                        validator=validator,
                        goal=case["goal"],
                        intent=case["intent"],
                        normalized_raw=normalized,
                        proposal_schema=(
                            canonical_schema if allow_unverifiable else None
                        ),
                    )
                except (TypeError, ValueError, json.JSONDecodeError) as error:
                    validation = {
                        "valid": False,
                        "spec": None,
                        "errors": [f"proposal_parse_failed:{error}"],
                    }
        else:
            validation = {
                "valid": False,
                "spec": None,
                "errors": [response.get("error_kind", "provider_error")],
            }
        attempts.append(
            {
                "attempt": attempt,
                "response": response,
                "raw_schema_validation": raw_schema_validation,
                "normalized_proposal": normalized,
                "validation": copy.deepcopy(validation),
            }
        )
        if not should_regenerate(validation, attempt):
            break
    execution = {
        "evaluations": [],
        "same_snapshot": False,
        "reference_fallback_count": 0,
        "gold_used_for_execution_count": 0,
    }
    coverage = None
    additive = None
    if validation.get("valid"):
        raw_execution = _execute_spec_isolated(
            spec=validation["spec"],
            lane_root=pair_root / "candidate-workspaces" / lane,
            frozen_product=frozen_product,
            frozen_before=frozen_before,
            snapshot_manifests=snapshot_manifests,
            browser_toolchain=browser_toolchain,
            executor_capabilities=executor_capabilities,
        )
        scored = score_candidate_outcomes(
            case_id=case["case_id"],
            lane=lane,
            oracles=validation["spec"]["oracles"],
            outcomes=raw_execution["evaluations"],
            adapters=adapters,
        )
        execution = {**raw_execution, "evaluations": scored}
        coverage = score_claim_coverage(
            case=case, adapters=adapters, evaluations=scored
        )
        additive = combine_evaluations(
            case=case,
            adapters=adapters,
            baseline_evaluations=baseline.get("evaluations", []),
            candidate_evaluations=scored,
            baseline_status=baseline.get("status", "baseline_unavailable"),
        )
    return {
        "attempts": attempts,
        "regenerated": len(attempts) == 2,
        "validation": validation,
        "execution": execution,
        "candidate_coverage": coverage,
        "additive_comparison": additive,
        "resource_usage": _candidate_resource_usage(
            attempts=attempts,
            wall_time_ns=time.monotonic_ns() - started_ns,
        ),
    }


def _candidate_resource_usage(
    *, attempts: list[dict[str, Any]], wall_time_ns: int
) -> dict[str, Any]:
    input_tokens = 0
    output_tokens = 0
    token_measurement_complete = True
    for attempt in attempts:
        response = attempt.get("response", {}).get("response")
        if not isinstance(response, dict):
            token_measurement_complete = False
            continue
        prompt_eval_count = response.get("prompt_eval_count")
        eval_count = response.get("eval_count")
        if not (
            isinstance(prompt_eval_count, int)
            and not isinstance(prompt_eval_count, bool)
            and prompt_eval_count >= 0
            and isinstance(eval_count, int)
            and not isinstance(eval_count, bool)
            and eval_count >= 0
        ):
            token_measurement_complete = False
            continue
        input_tokens += prompt_eval_count
        output_tokens += eval_count
    return {
        "scope": "candidate generation, canonicalization, validation, oracle execution, and scoring",
        "wall_time_ms": wall_time_ns / 1_000_000.0,
        "input_tokens": input_tokens if token_measurement_complete else None,
        "output_tokens": output_tokens if token_measurement_complete else None,
        "total_tokens": (
            input_tokens + output_tokens if token_measurement_complete else None
        ),
        "attempt_count": len(attempts),
        "token_measurement_complete": token_measurement_complete,
    }


def _validate_raw_proposal(*, raw: str, schema: dict[str, Any]) -> dict[str, Any]:
    try:
        proposal = json.loads(raw)
    except json.JSONDecodeError as error:
        return {
            "applied": True,
            "valid": False,
            "errors": [f"raw_schema_invalid:$:invalid JSON:{error.msg}"],
        }
    schema_errors = sorted(
        jsonschema.Draft202012Validator(schema).iter_errors(proposal),
        key=lambda error: (
            tuple(str(part) for part in error.absolute_path),
            error.message,
        ),
    )
    return {
        "applied": True,
        "valid": not schema_errors,
        "errors": [
            "raw_schema_invalid:"
            + ("/".join(str(part) for part in error.absolute_path) or "$")
            + f":{error.message}"
            for error in schema_errors
        ],
    }


def _build_prompt_v4(
    *,
    lane,
    base_prompt,
    case,
    request_id,
    shape,
    adapters,
    capabilities,
    manifests,
    executor_capabilities=None,
):
    prompt = (
        build_conformance_prompt(
            base_prompt, case, request_id, shape, adapters=adapters
        )
        if lane == "contract_conformance"
        else build_held_out_prompt(
            base_prompt, case, request_id, shape, capabilities=capabilities
        )
    )
    prefix, payload = prompt.rsplit("INPUT JSON:\n", 1)
    request = json.loads(payload)
    request["workspace_manifests"] = manifests
    if executor_capabilities is not None:
        strategies = request.get("executor_capabilities", [])
        request["executor_capabilities"] = {
            "strategies": strategies if isinstance(strategies, list) else [],
            **candidate_visible_executor_capabilities(executor_capabilities),
        }
    if case.get("task_contract"):
        request["task_contract"] = copy.deepcopy(case["task_contract"])
    for claim in request.get("required_claims", []):
        for observation in claim.get("expected_observations", []):
            observation.pop("adapter_id", None)
    return (
        prefix
        + "INPUT JSON:\n"
        + json.dumps(request, ensure_ascii=False, sort_keys=True)
        + "\n"
    )


def _validate_proposal_v4(
    *, validator, goal, intent, normalized_raw, proposal_schema=None
):
    """Validate the v0 core in Rust and preserve v4-only browser bindings."""
    proposal = json.loads(normalized_raw)
    before = _validate_proposal_core_v4(
        validator=validator,
        goal=goal,
        intent=intent,
        proposal=proposal,
        proposal_schema=proposal_schema,
    )
    repaired, host_repairs = apply_meaning_preserving_repairs(proposal)
    validation = (
        _validate_proposal_core_v4(
            validator=validator,
            goal=goal,
            intent=intent,
            proposal=repaired,
            proposal_schema=proposal_schema,
        )
        if host_repairs
        else before
    )
    return {
        **validation,
        "valid_before_host_repairs": before.get("valid", False),
        "errors_before_host_repairs": before.get("errors", []),
        "host_repairs": host_repairs,
    }


def _validate_proposal_core_v4(
    *, validator, goal, intent, proposal, proposal_schema=None
):
    if proposal_schema is not None:
        schema_errors = sorted(
            jsonschema.Draft202012Validator(proposal_schema).iter_errors(proposal),
            key=lambda error: (
                tuple(str(part) for part in error.absolute_path),
                error.message,
            ),
        )
        if schema_errors:
            return {
                "valid": False,
                "spec": None,
                "errors": [
                    "schema_invalid:"
                    + "/".join(str(part) for part in error.absolute_path)
                    + f":{error.message}"
                    for error in schema_errors
                ],
            }
    extensions = {}
    extension_errors = []
    stripped = copy.deepcopy(proposal)
    unverifiable_claims = [
        claim for claim in stripped.get("claims", []) if not claim.get("oracle_ids")
    ]
    if unverifiable_claims:
        unverifiable_ids = {claim.get("id") for claim in unverifiable_claims}
        stripped["claims"] = [
            claim
            for claim in stripped.get("claims", [])
            if claim.get("id") not in unverifiable_ids
        ]
    for oracle in stripped.get("oracles", []):
        input_value = oracle.get("input")
        if not isinstance(input_value, dict) or input_value.get("kind") != "dom":
            if oracle.get("strategy") in {"dom", "interaction"}:
                extension_errors.append(
                    f"v4_dom_input_required:{oracle.get('id', 'unknown')}"
                )
            continue
        extension = {
            key: input_value.pop(key)
            for key in ("port", "actions", "property")
            if key in input_value
        }
        extensions[oracle.get("id")] = extension
        extension_errors.extend(_v4_browser_extension_errors(oracle, extension))
    if stripped.get("claims"):
        validation = validate_proposal(
            validator=validator,
            goal=goal,
            intent=intent,
            normalized_raw=json.dumps(stripped, ensure_ascii=False, sort_keys=True),
        )
    elif proposal_schema is not None and unverifiable_claims:
        validation = {"valid": True, "spec": copy.deepcopy(proposal), "errors": []}
    else:
        validation = {
            "valid": False,
            "spec": None,
            "errors": ["every_claim_requires_an_oracle"],
        }
    if extension_errors:
        return {
            "valid": False,
            "spec": None,
            "errors": sorted({*validation.get("errors", []), *extension_errors}),
        }
    if not validation.get("valid"):
        return validation
    if stripped.get("claims") and unverifiable_claims:
        validation["spec"]["claims"] = copy.deepcopy(proposal["claims"])
    validation["unverifiable_claims"] = [
        {
            "claim_id": claim["id"],
            "reason": claim["unverifiable_reason"],
        }
        for claim in unverifiable_claims
    ]
    for oracle in validation["spec"].get("oracles", []):
        oracle["input"].update(extensions.get(oracle.get("id"), {}))
    return validation


def _v4_browser_extension_errors(oracle, extension):
    oracle_id = oracle.get("id", "unknown")
    errors = []
    port = extension.get("port")
    if not isinstance(port, int) or isinstance(port, bool) or not 1 <= port <= 65535:
        errors.append(f"v4_dom_port_invalid:{oracle_id}")
    setup = oracle.get("setup")
    argv = setup.get("argv") if isinstance(setup, dict) else None
    if isinstance(port, int) and (not isinstance(argv, list) or str(port) not in argv):
        errors.append(f"v4_dom_port_unbound:{oracle_id}")
    actions = extension.get("actions", [])
    actions_valid = (
        isinstance(actions, list)
        and len(actions) <= 32
        and all(
            isinstance(action, dict)
            and set(action) <= {"kind", "selector", "repeat"}
            and action.get("kind") == "click"
            and isinstance(action.get("selector"), str)
            and bool(action["selector"])
            and isinstance(action.get("repeat", 1), int)
            and not isinstance(action.get("repeat", 1), bool)
            and 1 <= action.get("repeat", 1) <= 16
            for action in actions
        )
    )
    if not actions_valid:
        errors.append(f"v4_dom_actions_invalid:{oracle_id}")
    strategy = oracle.get("strategy")
    if strategy == "interaction" and not actions:
        errors.append(f"v4_interaction_actions_missing:{oracle_id}")
    if strategy == "dom" and actions:
        errors.append(f"v4_dom_actions_forbidden:{oracle_id}")
    property_value = extension.get("property")
    if property_value is not None and (
        not isinstance(property_value, str)
        or not property_value
        or len(property_value) > 128
    ):
        errors.append(f"v4_dom_property_invalid:{oracle_id}")
    return errors


def _execute_spec_isolated(
    *,
    spec,
    lane_root,
    frozen_product,
    frozen_before,
    snapshot_manifests,
    browser_toolchain,
    executor_capabilities,
):
    evaluations = []
    same_snapshot = True
    fallback_count = 0
    gold_count = 0
    for index, oracle in enumerate(spec["oracles"], 1):
        oracle_root = lane_root / f"oracle-{index:03d}"
        product = oracle_root / "product"
        _copy_workspace(frozen_product, product)
        workspaces = {"product": product}
        frozen = {"product": snapshot_manifests["product"]["snapshot_sha256"]}
        if frozen_before is not None:
            before = oracle_root / "before"
            _copy_workspace(frozen_before, before)
            workspaces["before"] = before
            frozen["before"] = snapshot_manifests["before"]["snapshot_sha256"]
        result = evaluate_candidate_spec_v4(
            spec={"claims": spec["claims"], "oracles": [oracle]},
            workspaces=workspaces,
            frozen_snapshot_sha256=frozen,
            browser_toolchain=browser_toolchain,
            executor_capabilities=executor_capabilities,
        )
        evaluations.extend(result["evaluations"])
        same_snapshot = same_snapshot and result["same_snapshot"]
        fallback_count += result["reference_fallback_count"]
        gold_count += result["gold_used_for_execution_count"]
    return {
        "evaluations": evaluations,
        "same_snapshot": same_snapshot,
        "reference_fallback_count": fallback_count,
        "gold_used_for_execution_count": gold_count,
    }


def _product_workspace(case, stage_paths):
    desired = product_stage(case)
    path = stage_paths.get((case["case_id"], desired))
    if path is None:
        raise ValueError(f"product workspace missing:{case['case_id']}:{desired}")
    return path


def _freeze_before(case, product_workspace, pair_root):
    if case["intent"] != "fix":
        return None
    destination = pair_root / "frozen-before"
    _copy_workspace(product_workspace, destination)
    return destination


def _freeze_browser_toolchain(product_workspace, pair_root):
    source = product_workspace / "node_modules" / "playwright-core"
    if not source.is_dir():
        return None
    destination = pair_root / "candidate-toolchain"
    if destination.exists():
        shutil.rmtree(destination)
    destination.mkdir(parents=True)
    shutil.copytree(source, destination / "playwright-core", symlinks=True)
    return destination


def _copy_workspace(source, destination):
    if destination.exists():
        shutil.rmtree(destination)
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copytree(
        source,
        destination,
        symlinks=True,
        ignore=shutil.ignore_patterns(
            ".anvil",
            ".commandagent",
            ".commandagent-state",
            ".commandagent-eval-home",
            ".commandagent-eval-tmp",
            ".goal-verify-baseline",
            ".env",
            ".env.*",
            ".npmrc",
            ".pypirc",
            "credentials",
            "credentials.json",
        ),
    )


def _default_baseline_runner():
    from eval_lib.goal_verify_baseline_product_v3 import run_current_product_baseline

    return run_current_product_baseline


def _select_campaign_pair_ids(*, selected, samples_per_cell, pair_ids):
    all_pair_ids = [
        f"{case['case_id']}--pair-{sample_index:02d}"
        for case in selected
        for sample_index in range(1, samples_per_cell + 1)
    ]
    if pair_ids is None:
        return all_pair_ids
    if not pair_ids or len(pair_ids) != len(set(pair_ids)):
        raise ValueError("selected pair IDs must be present and unique")
    if set(pair_ids) - set(all_pair_ids):
        raise ValueError("selected pair IDs are not in the contract")
    return list(pair_ids)


def _campaign_namespace(*, contract, pair_ids, request_namespace, limit):
    if pair_ids is None:
        if request_namespace is not None:
            raise ValueError("full campaign cannot override the request namespace")
        return contract["contract_id"]
    if limit is not None:
        raise ValueError("preregistered smoke cannot use a plain pair limit")
    smoke = contract.get("smoke")
    if not isinstance(smoke, dict):
        raise TypeError("preregistered smoke contract is missing")
    expected_pairs = smoke.get("pair_ids")
    expected_namespace = smoke.get("request_namespace")
    if pair_ids != expected_pairs:
        raise ValueError("smoke pair IDs differ from the preregistered exact set")
    if request_namespace != expected_namespace:
        raise ValueError("smoke request namespace differs from the preregistered value")
    if expected_namespace == contract["contract_id"]:
        raise ValueError("smoke request namespace must differ from the full run")
    return expected_namespace


def _summary(run_dir, completed, target, ledger_head):
    value = {
        "completed_pairs": completed,
        "target_pairs": target,
        "completed_proposals": completed * len(LANES),
        "target_proposals": target * len(LANES),
        "complete": completed == target,
        "record_ledger_entries": completed,
        "record_ledger_head_sha256": ledger_head,
    }
    _atomic_json(run_dir / "campaign-summary.json", value)
    return value
