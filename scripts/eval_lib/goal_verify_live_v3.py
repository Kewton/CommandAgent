from __future__ import annotations

import copy
import json
from collections.abc import Callable
from pathlib import Path
from typing import Any

from eval_lib.goal_verify_baseline_product_v3 import (
    extract_product_observations,
    run_current_product_baseline,
    score_baseline_observations,
)
from eval_lib.goal_verify_executors_v3 import run_command
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
from eval_lib.goal_verify_observation_match_v3 import (
    evaluate_candidate_spec,
    score_claim_coverage,
)
from eval_lib.goal_verify_preflight_v3 import readiness_report
from eval_lib.goal_verify_v2 import normalize_v2_proposal
from eval_lib.goal_verify_v3 import (
    LANES,
    build_conformance_prompt,
    build_held_out_prompt,
    canonicalize_held_out_proposal,
    effective_prompt_sha256,
    load_prompt_from_contract,
    regeneration_seed,
    should_regenerate,
)
from eval_lib.goal_verify_workspaces_v3 import (
    load_workspace_registry,
    prepare_workspace_stage,
    workspace_by_case,
)

Provider = Callable[..., dict[str, Any]]
BaselineRunner = Callable[..., dict[str, Any]]


def run_campaign_v3(
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
    provider: Provider = request_ollama,
    baseline_runner: BaselineRunner = run_current_product_baseline,
) -> dict[str, Any]:
    contract = load_json(contract_path)
    readiness = readiness_report(
        root=root, contract_path=contract_path, execution_root=execution_root
    )
    if not readiness["ready"]:
        raise ValueError(
            "v3 preflight is not ready: " + ",".join(readiness["blockers"])
        )
    if run_dir.name != contract["contract_id"]:
        raise ValueError("contract_id differs from run directory basename")
    prompt_path, base_prompt = load_prompt_from_contract(
        root=root, contract=contract, cli_prompt=prompt_path
    )
    corpus = load_json(corpus_path)
    schema = load_json(schema_path)
    adapters_registry = load_json(
        root / contract["oracle_execution"]["adapter_registry"]
    )
    adapters = adapters_registry["adapters"]
    capabilities = load_json(
        root / "eval/goal_verify/v0/phase6-execution-capabilities-v3.json"
    )
    workspace_registry = load_workspace_registry(
        root / contract["oracle_execution"]["workspace_registry"]
    )
    workspaces = workspace_by_case(workspace_registry)
    selected_ids = [row["case_id"] for row in contract["selected_cells"]]
    corpus_by_case = {row["case_id"]: row for row in corpus["cases"]}
    selected = [corpus_by_case[case_id] for case_id in selected_ids]
    shapes = {
        intent: (root / path).read_text(encoding="utf-8")
        for intent, path in contract["generation"]["shape_examples"].items()
    }
    effective_hashes = {}
    for case in selected:
        for lane in LANES:
            prompt = _build_prompt(
                lane,
                base_prompt,
                case,
                "manifest-hash",
                shapes[case["intent"]],
                adapters,
                capabilities,
            )
            effective_hashes[f"{case['intent']}:{lane}"] = effective_prompt_sha256(
                prompt
            )
    manifest = {
        "schema_version": "commandagent.goal_verify.phase6_live_manifest.v3",
        "contract": str(contract_path.relative_to(root)),
        "contract_sha256": sha256_file(contract_path),
        "contract_id": contract["contract_id"],
        "code_sha": contract["code_sha"],
        "corpus_sha256": sha256_file(corpus_path),
        "schema_sha256": sha256_file(schema_path),
        "prompt_file_sha256": sha256_file(prompt_path),
        "effective_prompt_sha256": effective_hashes,
        "adapter_registry_sha256": sha256_file(
            root / contract["oracle_execution"]["adapter_registry"]
        ),
        "workspace_registry_sha256": sha256_file(
            root / contract["oracle_execution"]["workspace_registry"]
        ),
        "model": contract["model"],
        "model_digest": contract["model_digest"],
        "samples_per_cell": contract["samples_per_cell"],
        "proposal_lanes": list(LANES),
        "target_pair_indexes": len(selected) * contract["samples_per_cell"],
        "target_proposals": len(selected) * contract["samples_per_cell"] * len(LANES),
        "execution_root": str(execution_root.resolve()),
        "commandagent_bin": str(
            (commandagent_bin or root / "target/release/commandagent").resolve()
        ),
    }
    manifest_path = run_dir / "campaign-manifest.json"
    if manifest_path.exists() and load_json(manifest_path) != manifest:
        raise ValueError("campaign manifest differs from frozen v3 inputs")
    if not manifest_path.exists():
        _atomic_json(manifest_path, manifest)
    lock = _acquire_run_lock(run_dir, contract["integrity"]["exclusive_run_lock"])
    try:
        ledger_path = run_dir / contract["integrity"]["record_ledger"]
        ledger_entries, ledger_head = _load_record_ledger(
            root=root, run_dir=run_dir, ledger_path=ledger_path
        )
        completed = 0
        target = len(selected) * int(contract["samples_per_cell"])
        for source_index, case in enumerate(selected):
            for sample_index in range(1, int(contract["samples_per_cell"]) + 1):
                if limit is not None and completed >= limit:
                    return _summary(run_dir, completed, target, ledger_head)
                pair_id = f"{case['case_id']}--pair-{sample_index:02d}"
                relative = (
                    Path("raw") / case["case_id"] / f"pair-{sample_index:02d}.json"
                )
                record_path = run_dir / relative
                reference = str(record_path.relative_to(root))
                if record_path.exists():
                    record = load_json(record_path)
                else:
                    pair_root = execution_root / contract["contract_id"] / pair_id
                    stage_paths = _prepare_pair_stages(
                        root=root,
                        case=case,
                        workspace=workspaces.get(case["case_id"]),
                        destination=pair_root,
                    )
                    baseline = _run_baseline(
                        root=root,
                        contract=contract,
                        case=case,
                        pair_root=pair_root,
                        stage_paths=stage_paths,
                        adapters=adapters,
                        commandagent_bin=commandagent_bin,
                        baseline_runner=baseline_runner,
                    )
                    lanes = {}
                    pair_index = (
                        source_index * int(contract["samples_per_cell"]) + sample_index
                    )
                    for lane in LANES:
                        lanes[lane] = _run_lane(
                            root=root,
                            contract=contract,
                            schema=schema,
                            validator=validator,
                            provider=provider,
                            case=case,
                            pair_id=pair_id,
                            pair_index=pair_index,
                            lane=lane,
                            base_prompt=base_prompt,
                            shape=shapes[case["intent"]],
                            adapters=adapters,
                            capabilities=capabilities,
                            stage_paths=stage_paths,
                        )
                    record = {
                        "schema_version": "commandagent.goal_verify.phase6_live_record.v3",
                        "pair_id": pair_id,
                        "source_case_id": case["case_id"],
                        "cell_lane": next(
                            row["lane"]
                            for row in contract["selected_cells"]
                            if row["case_id"] == case["case_id"]
                        ),
                        "goal": case["goal"],
                        "intent": case["intent"],
                        "profile": case["profile"],
                        "required_claims": copy.deepcopy(case["required_claims"]),
                        "record_path": reference,
                        "baseline": baseline,
                        "lanes": lanes,
                    }
                    _atomic_json(record_path, record)
                    ledger_head = _append_record_ledger(
                        ledger_path=ledger_path,
                        entries=ledger_entries,
                        previous=ledger_head,
                        pair_id=pair_id,
                        source_case_id=case["case_id"],
                        record_reference=reference,
                        record_path=record_path,
                    )
                completed += 1
                _summary(run_dir, completed, target, ledger_head)
        return _summary(run_dir, completed, target, ledger_head)
    finally:
        lock.close()


def _prepare_pair_stages(
    *,
    root: Path,
    case: dict[str, Any],
    workspace: dict[str, Any] | None,
    destination: Path,
) -> dict[tuple[str, str], Path]:
    if workspace is None:
        return {}
    return {
        (case["case_id"], stage): prepare_workspace_stage(
            root=root,
            workspace=workspace,
            stage=stage,
            destination=destination / stage,
            provisioned_root=destination.parents[1] / "provisioned",
        )
        for stage in workspace["stages"]
    }


def _run_baseline(
    *,
    root: Path,
    contract: dict[str, Any],
    case: dict[str, Any],
    pair_root: Path,
    stage_paths: dict[tuple[str, str], Path],
    adapters: list[dict[str, Any]],
    commandagent_bin: Path | None,
    baseline_runner: BaselineRunner,
) -> dict[str, Any]:
    workspace = next(
        (
            path
            for (case_id, stage), path in stage_paths.items()
            if case_id == case["case_id"] and stage in {"initial", "before"}
        ),
        None,
    )
    if workspace is None:
        return {"status": "baseline_unavailable", "reason": "workspace_unavailable"}
    result = baseline_runner(
        commandagent_bin=commandagent_bin or root / "target/release/commandagent",
        workspace=workspace,
        case=case,
        model=contract["model"],
        timeout_sec=int(contract["generation"]["request_timeout_sec"]),
    )
    run_path = result.get("product_run_dir")
    observations = (
        extract_product_observations(
            Path(run_path), replay=run_command, replay_cwd=workspace
        )
        if run_path
        else []
    )
    scored = score_baseline_observations(
        observations, adapters, case_id=case["case_id"]
    )
    coverage = score_claim_coverage(case=case, adapters=adapters, evaluations=scored)
    return {
        **result,
        "observations": observations,
        "evaluations": scored,
        "coverage": coverage,
    }


def _build_prompt(lane, base, case, request_id, shape, adapters, capabilities):
    if lane == "contract_conformance":
        return build_conformance_prompt(
            base, case, request_id, shape, adapters=adapters
        )
    return build_held_out_prompt(
        base, case, request_id, shape, capabilities=capabilities
    )


def _run_lane(
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
    adapters,
    capabilities,
    stage_paths,
):
    attempts = []
    validation = {"valid": False, "spec": None, "errors": ["not_attempted"]}
    for attempt in (1, 2):
        request_id = f"{contract['contract_id']}:{pair_id}:{lane}:attempt-{attempt}"
        prompt = _build_prompt(
            lane, base_prompt, case, request_id, shape, adapters, capabilities
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
        if response.get("status") == "completed":
            raw = response["response"].get("response", "")
            try:
                normalized = (
                    normalize_v2_proposal(
                        raw, case=case, model=contract["model"], request_id=request_id
                    )
                    if lane == "contract_conformance"
                    else canonicalize_held_out_proposal(
                        raw, case=case, model=contract["model"], request_id=request_id
                    )
                )
                validation = validate_proposal(
                    validator=validator,
                    goal=case["goal"],
                    intent=case["intent"],
                    normalized_raw=normalized,
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
                "normalized_proposal": normalized,
                "validation": copy.deepcopy(validation),
            }
        )
        if not should_regenerate(validation, attempt):
            break
    evaluation = {"evaluations": [], "scoring_coverage": False}
    coverage = None
    if validation.get("valid"):
        evaluation = evaluate_candidate_spec(
            case_id=case["case_id"],
            spec=validation["spec"],
            adapters=adapters,
            workspaces=stage_paths,
            lane=lane,
        )
        coverage = score_claim_coverage(
            case=case, adapters=adapters, evaluations=evaluation["evaluations"]
        )
    return {
        "attempts": attempts,
        "regenerated": len(attempts) == 2,
        "validation": validation,
        "execution": evaluation,
        "coverage": coverage,
    }


def _summary(
    run_dir: Path, completed: int, target: int, ledger_head: str
) -> dict[str, Any]:
    value = {
        "completed_pairs": completed,
        "target_pairs": target,
        "completed_proposals": completed * 2,
        "target_proposals": target * 2,
        "complete": completed == target,
        "record_ledger_entries": completed,
        "record_ledger_head_sha256": ledger_head,
    }
    _atomic_json(run_dir / "campaign-summary.json", value)
    return value
