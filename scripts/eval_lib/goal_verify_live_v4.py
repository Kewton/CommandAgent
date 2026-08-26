from __future__ import annotations

import copy
import json
import shutil
from pathlib import Path
from typing import Any

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
from eval_lib.goal_verify_observation_match_v3 import score_claim_coverage
from eval_lib.goal_verify_preflight_v4 import readiness_report
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
from eval_lib.goal_verify_workspaces_v3 import (
    load_workspace_registry,
    workspace_by_case,
)


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
) -> dict[str, Any]:
    contract = load_json(contract_path)
    readiness = readiness_report(
        root=root, contract_path=contract_path, execution_root=execution_root
    )
    if not readiness["ready"]:
        raise ValueError("v4 preflight is not ready: " + ",".join(readiness["blockers"]))
    commandagent = (commandagent_bin or root / "target/release/commandagent").resolve()
    live_inputs = verify_live_inputs_v3(
        root=root,
        contract=contract,
        commandagent_bin=commandagent,
        validator=validator,
    )
    if run_dir.name != contract["contract_id"]:
        raise ValueError("contract_id differs from run directory basename")
    resolved_prompt, base_prompt = load_prompt_from_contract(
        root=root, contract=contract, cli_prompt=prompt_path
    )
    corpus = load_json(corpus_path)
    schema = load_json(schema_path)
    adapters = load_json(root / contract["scoring"]["answer_key"])["adapters"]
    capabilities = load_json(root / contract["capability_registry"])
    workspaces = workspace_by_case(
        load_workspace_registry(root / contract["workspace_registry"])
    )
    selected_ids = [row["case_id"] for row in contract["selected_cells"]]
    corpus_by_case = {row["case_id"]: row for row in corpus["cases"]}
    selected = [corpus_by_case[case_id] for case_id in selected_ids]
    shapes = {
        intent: (root / path).read_text(encoding="utf-8")
        for intent, path in contract["generation"]["shape_examples"].items()
    }
    manifest = {
        "schema_version": "commandagent.goal_verify.phase6_live_manifest.v4",
        "contract_id": contract["contract_id"],
        "contract_sha256": sha256_file(contract_path),
        "code_sha": contract["code_sha"],
        "corpus_sha256": sha256_file(corpus_path),
        "schema_sha256": sha256_file(schema_path),
        "prompt_file_sha256": sha256_file(resolved_prompt),
        "answer_key_sha256": sha256_file(root / contract["scoring"]["answer_key"]),
        "workspace_registry_sha256": sha256_file(root / contract["workspace_registry"]),
        "target_pairs": len(selected) * int(contract["samples_per_cell"]),
        "target_proposals": len(selected) * int(contract["samples_per_cell"]) * len(LANES),
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
                if limit is not None and completed >= limit:
                    return _summary(run_dir, completed, target, ledger_head)
                pair_id = f"{case['case_id']}--pair-{sample_index:02d}"
                relative = Path("raw") / case["case_id"] / f"pair-{sample_index:02d}.json"
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
                        destination=pair_root / "source-stages",
                    )
                    product_workspace = _product_workspace(case, stage_paths)
                    frozen_before = _freeze_before(case, product_workspace, pair_root)
                    baseline = _run_baseline(
                        root=root,
                        contract=contract,
                        case=case,
                        pair_root=pair_root,
                        stage_paths=stage_paths,
                        adapters=adapters,
                        commandagent_bin=commandagent,
                        baseline_runner=baseline_runner
                        or _default_baseline_runner(),
                    )
                    frozen_product = pair_root / "frozen-product"
                    _copy_workspace(product_workspace, frozen_product)
                    snapshot_manifests = {
                        "product": workspace_manifest(frozen_product),
                    }
                    if frozen_before is not None:
                        snapshot_manifests["before"] = workspace_manifest(frozen_before)
                    pair_index = source_index * int(contract["samples_per_cell"]) + sample_index
                    lanes = {
                        lane: _run_lane_v4(
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
                            frozen_product=frozen_product,
                            frozen_before=frozen_before,
                            snapshot_manifests=snapshot_manifests,
                            baseline=baseline,
                            pair_root=pair_root,
                        )
                        for lane in LANES
                    }
                    record = {
                        "schema_version": "commandagent.goal_verify.phase6_live_record.v4",
                        "pair_id": pair_id,
                        "source_case_id": case["case_id"],
                        "goal": case["goal"],
                        "intent": case["intent"],
                        "profile": case["profile"],
                        "required_claims": copy.deepcopy(case["required_claims"]),
                        "record_path": reference,
                        "snapshot_manifests": snapshot_manifests,
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
    *, root, contract, schema, validator, provider, case, pair_id, pair_index, lane,
    base_prompt, shape, adapters, capabilities, frozen_product, frozen_before,
    snapshot_manifests, baseline, pair_root,
):
    attempts = []
    validation = {"valid": False, "spec": None, "errors": ["not_attempted"]}
    visible = {
        stage: candidate_visible_manifest(manifest)
        for stage, manifest in snapshot_manifests.items()
    }
    for attempt in (1, 2):
        request_id = f"{contract['contract_id']}:{pair_id}:{lane}:attempt-{attempt}"
        prompt = _build_prompt_v4(
            lane=lane,
            base_prompt=base_prompt,
            case=case,
            request_id=request_id,
            shape=shape,
            adapters=adapters,
            capabilities=capabilities,
            manifests=visible,
        )
        response = provider(
            endpoint=contract["endpoint"],
            model=contract["model"],
            prompt=prompt,
            schema=schema,
            seed=regeneration_seed(contract["generation"]["seed_base"], pair_index, lane, attempt),
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
                    normalize_v2_proposal(raw, case=case, model=contract["model"], request_id=request_id)
                    if lane == "contract_conformance"
                    else canonicalize_held_out_proposal(raw, case=case, model=contract["model"], request_id=request_id)
                )
                validation = validate_proposal(
                    validator=validator,
                    goal=case["goal"],
                    intent=case["intent"],
                    normalized_raw=normalized,
                )
            except (TypeError, ValueError, json.JSONDecodeError) as error:
                validation = {"valid": False, "spec": None, "errors": [f"proposal_parse_failed:{error}"]}
        else:
            validation = {"valid": False, "spec": None, "errors": [response.get("error_kind", "provider_error")]}
        attempts.append(
            {"attempt": attempt, "response": response, "normalized_proposal": normalized, "validation": copy.deepcopy(validation)}
        )
        if not should_regenerate(validation, attempt):
            break
    execution = {"evaluations": [], "same_snapshot": False, "reference_fallback_count": 0, "gold_used_for_execution_count": 0}
    coverage = None
    additive = None
    if validation.get("valid"):
        raw_execution = _execute_spec_isolated(
            spec=validation["spec"],
            lane_root=pair_root / "candidate-workspaces" / lane,
            frozen_product=frozen_product,
            frozen_before=frozen_before,
            snapshot_manifests=snapshot_manifests,
        )
        scored = score_candidate_outcomes(
            case_id=case["case_id"],
            lane=lane,
            oracles=validation["spec"]["oracles"],
            outcomes=raw_execution["evaluations"],
            adapters=adapters,
        )
        execution = {**raw_execution, "evaluations": scored}
        coverage = score_claim_coverage(case=case, adapters=adapters, evaluations=scored)
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
    }


def _build_prompt_v4(*, lane, base_prompt, case, request_id, shape, adapters, capabilities, manifests):
    prompt = (
        build_conformance_prompt(base_prompt, case, request_id, shape, adapters=adapters)
        if lane == "contract_conformance"
        else build_held_out_prompt(base_prompt, case, request_id, shape, capabilities=capabilities)
    )
    prefix, payload = prompt.rsplit("INPUT JSON:\n", 1)
    request = json.loads(payload)
    request["workspace_manifests"] = manifests
    for claim in request.get("required_claims", []):
        for observation in claim.get("expected_observations", []):
            observation.pop("adapter_id", None)
    return prefix + "INPUT JSON:\n" + json.dumps(request, ensure_ascii=False, sort_keys=True) + "\n"


def _execute_spec_isolated(
    *, spec, lane_root, frozen_product, frozen_before, snapshot_manifests
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
    desired = "initial" if case["intent"] == "create" else "before"
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
            ".commandagent-state",
            ".commandagent-eval-home",
            ".commandagent-eval-tmp",
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
