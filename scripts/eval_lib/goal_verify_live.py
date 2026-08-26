from __future__ import annotations

import copy
import fcntl
import hashlib
import json
import os
import subprocess
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any

from eval_lib.goal_verify_artifacts_v2 import (
    build_registered_baseline_spec,
    evaluate_spec_against_snapshot,
    prepare_snapshot_workspace,
    snapshot_case_v2,
    validate_adapter_registry,
    validate_snapshot_registry,
)
from eval_lib.goal_verify_sandbox import run_macos_sandbox
from eval_lib.goal_verify_v2 import (
    build_v2_prompt,
    candidate_case_v2,
    classify_oracle_execution,
    normalize_v2_proposal,
)


def load_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise TypeError(f"expected JSON object: {path}")
    return value


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _atomic_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    temporary.replace(path)


def _canonical_json(value: Any) -> bytes:
    return json.dumps(
        value, ensure_ascii=False, separators=(",", ":"), sort_keys=True
    ).encode()


def _ledger_entry_hash(entry_without_hash: dict[str, Any]) -> str:
    digest = hashlib.sha256()
    digest.update(entry_without_hash["previous_entry_sha256"].encode())
    digest.update(_canonical_json(entry_without_hash))
    return digest.hexdigest()


def _load_record_ledger(
    *, root: Path, run_dir: Path, ledger_path: Path
) -> tuple[dict[str, dict[str, Any]], str]:
    entries: dict[str, dict[str, Any]] = {}
    previous = "0" * 64
    if ledger_path.exists():
        for index, line in enumerate(ledger_path.read_text(encoding="utf-8").splitlines(), 1):
            entry = json.loads(line)
            if not isinstance(entry, dict):
                raise TypeError(f"ledger entry {index} must be an object")
            unsigned = {key: value for key, value in entry.items() if key != "entry_sha256"}
            if entry.get("sequence") != index:
                raise ValueError(f"ledger sequence mismatch at entry {index}")
            if entry.get("previous_entry_sha256") != previous:
                raise ValueError(f"ledger chain mismatch at entry {index}")
            expected_hash = _ledger_entry_hash(unsigned)
            if entry.get("entry_sha256") != expected_hash:
                raise ValueError(f"ledger entry hash mismatch at entry {index}")
            record_reference = entry.get("record_path")
            if not isinstance(record_reference, str) or record_reference in entries:
                raise ValueError(f"ledger record path invalid at entry {index}")
            record_path = root / record_reference
            if not record_path.is_file() or sha256_file(record_path) != entry.get("record_sha256"):
                raise ValueError(f"ledger record hash mismatch at entry {index}")
            entries[record_reference] = entry
            previous = expected_hash
    raw_paths = {
        str(path.relative_to(root)) for path in (run_dir / "raw").glob("**/pair-*.json")
    }
    if raw_paths != set(entries):
        raise ValueError("raw record set differs from append-only ledger")
    return entries, previous


def _append_record_ledger(
    *,
    ledger_path: Path,
    entries: dict[str, dict[str, Any]],
    previous: str,
    pair_id: str,
    source_case_id: str,
    record_reference: str,
    record_path: Path,
) -> str:
    unsigned = {
        "sequence": len(entries) + 1,
        "pair_id": pair_id,
        "source_case_id": source_case_id,
        "record_path": record_reference,
        "record_sha256": sha256_file(record_path),
        "previous_entry_sha256": previous,
    }
    entry = {**unsigned, "entry_sha256": _ledger_entry_hash(unsigned)}
    ledger_path.parent.mkdir(parents=True, exist_ok=True)
    with ledger_path.open("a", encoding="utf-8") as target:
        target.write(json.dumps(entry, ensure_ascii=False, sort_keys=True) + "\n")
        target.flush()
        os.fsync(target.fileno())
    entries[record_reference] = entry
    return entry["entry_sha256"]


def _validate_record_identity(
    record: dict[str, Any], *, pair_id: str, source_case_id: str, record_reference: str
) -> None:
    expected = {
        "pair_id": pair_id,
        "source_case_id": source_case_id,
        "record_path": record_reference,
    }
    if {key: record.get(key) for key in expected} != expected:
        raise ValueError(f"record identity mismatch for {pair_id}")


def _acquire_run_lock(run_dir: Path, lock_name: str) -> Any:
    run_dir.mkdir(parents=True, exist_ok=True)
    lock = (run_dir / lock_name).open("a+", encoding="utf-8")
    try:
        fcntl.flock(lock.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        lock.close()
        raise RuntimeError(f"campaign run directory is already active: {run_dir}") from None
    return lock


def _evidence_registry(case: dict[str, Any]) -> list[dict[str, Any]]:
    if case["intent"] == "create":
        return []
    registry = []
    for index, claim in enumerate(case["required_claims"]):
        if case["intent"] == "fix":
            requirement_id = ("before_fails", "after_passes", "no_regression")[
                min(index, 2)
            ]
            registry.append(
                {
                    "claim_id": claim["id"],
                    "artifact_path": "evidence/fix-evidence.json",
                    "requirement_id": requirement_id,
                    "stage": "before" if requirement_id == "before_fails" else "after",
                    "expected_polarity": "failure"
                    if requirement_id == "before_fails"
                    else "success",
                    "lineage": case["case_id"],
                    "epoch": 1,
                }
            )
        else:
            requirement_id = "reproducer_fails" if index == 0 else "diagnosis_bound"
            registry.append(
                {
                    "claim_id": claim["id"],
                    "artifact_path": "evidence/investigation-evidence.json",
                    "requirement_id": requirement_id,
                    "binding_id": f"{case['case_id']}:{claim['id']}",
                    "stage": "reproduce"
                    if requirement_id == "reproducer_fails"
                    else "diagnosis",
                    "lineage": case["case_id"],
                    "epoch": 1,
                }
            )
    return registry


def build_prompt(
    base_prompt: str, case: dict[str, Any], request_id: str, shape_example: str
) -> str:
    request = {
        "goal": case["goal"],
        "intent": case["intent"],
        "profile": case["profile"],
        "generation": {
            "provider": "ollama-local",
            "model": "set-by-caller",
            "request_id": request_id,
            "raw_response_sha256": "",
        },
    }
    registry = _evidence_registry(case)
    if registry:
        request["existing_evidence_registry"] = registry
    return (
        f"{base_prompt.rstrip()}\n\n"
        "The following object is a shape example only. Use exactly its field names and nested "
        "structure; do not copy its values. In particular the top-level key is schema_version, "
        "claims use origin/normalized_requirement/required/kind/oracle_ids, and oracles use the "
        "exact schema fields.\n"
        f"SHAPE EXAMPLE:\n{shape_example.rstrip()}\n\n"
        "Now emit the proposal for INPUT JSON. Copy goal, intent, profile, and generation values "
        "from INPUT JSON exactly. Return JSON only.\n"
        f"INPUT JSON:\n{json.dumps(request, ensure_ascii=False)}\n"
    )


def request_ollama(
    *,
    endpoint: str,
    model: str,
    prompt: str,
    schema: dict[str, Any],
    seed: int,
    temperature: float,
    num_predict: int,
    timeout_sec: int,
    keep_alive: str,
    think: bool,
) -> dict[str, Any]:
    body = json.dumps(
        {
            "model": model,
            "prompt": prompt,
            "stream": False,
            "format": schema,
            "think": think,
            "keep_alive": keep_alive,
            "options": {
                "seed": seed,
                "temperature": temperature,
                "num_predict": num_predict,
            },
        }
    ).encode()
    request = urllib.request.Request(
        endpoint,
        data=body,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    started = time.monotonic_ns()
    try:
        with urllib.request.urlopen(request, timeout=timeout_sec) as response:
            payload = json.loads(response.read().decode())
        if not isinstance(payload, dict):
            raise TypeError("Ollama response must be an object")
        payload["client_wall_time_ns"] = time.monotonic_ns() - started
        return {"status": "completed", "response": payload}
    except (TimeoutError, urllib.error.URLError, ValueError, json.JSONDecodeError) as error:
        return {
            "status": "provider_error",
            "error_kind": type(error).__name__,
            "error": str(error),
            "client_wall_time_ns": time.monotonic_ns() - started,
        }


def normalize_proposal(raw: str, *, model: str, request_id: str) -> str:
    value = json.loads(raw)
    if not isinstance(value, dict):
        raise TypeError("provider proposal must be an object")
    value["generation"] = {
        "provider": "ollama-local",
        "model": model,
        "request_id": request_id,
        "raw_response_sha256": hashlib.sha256(raw.encode()).hexdigest(),
    }
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"))


def validate_proposal(
    *, validator: Path, goal: str, intent: str, normalized_raw: str
) -> dict[str, Any]:
    completed = subprocess.run(
        [str(validator)],
        input=json.dumps({"goal": goal, "intent": intent, "raw": normalized_raw}),
        text=True,
        capture_output=True,
        check=False,
    )
    if completed.returncode != 0:
        return {
            "valid": False,
            "spec": None,
            "errors": [f"validator_failed:{completed.stderr.strip()}"],
        }
    value = json.loads(completed.stdout)
    if not isinstance(value, dict):
        raise TypeError("validator response must be an object")
    return value


def _verify_frozen_git_inputs(root: Path, contract: dict[str, Any]) -> str:
    code_sha = contract["code_sha"]
    frozen_inputs = contract.get("frozen_inputs", contract.get("runner_sources"))
    if not isinstance(frozen_inputs, list) or not frozen_inputs or not all(
        isinstance(path, str) and path for path in frozen_inputs
    ):
        raise ValueError("frozen input paths are absent or invalid")
    ancestor = subprocess.run(
        ["git", "merge-base", "--is-ancestor", code_sha, "HEAD"],
        cwd=root,
        text=True,
        capture_output=True,
        check=False,
    )
    if ancestor.returncode != 0:
        raise ValueError(f"frozen code SHA is not an ancestor of HEAD: {code_sha}")
    unchanged = subprocess.run(
        ["git", "diff", "--quiet", code_sha, "--", *frozen_inputs],
        cwd=root,
        text=True,
        capture_output=True,
        check=False,
    )
    if unchanged.returncode == 1:
        raise ValueError("frozen runner or experiment input differs from code SHA")
    if unchanged.returncode != 0:
        raise RuntimeError(
            "unable to compare frozen inputs with code SHA: "
            + unchanged.stderr.strip()
        )
    return code_sha


def _verify_exact_sha_ci(root: Path, contract: dict[str, Any], code_sha: str) -> None:
    ci = load_json(root / contract["exact_sha_ci_evidence"])
    workflows = ci.get("workflows")
    if (
        not isinstance(workflows, list)
        or not workflows
        or not all(isinstance(run, dict) for run in workflows)
    ):
        raise ValueError("exact-SHA CI evidence is absent or non-green")
    required = set(contract.get("required_ci_workflows", ["CI"]))
    successful = {
        run.get("name")
        for run in workflows
        if run.get("status") == "completed" and run.get("conclusion") == "success"
    }
    if (
        ci.get("head_sha") != code_sha
        or any(
            run.get("status") != "completed" or run.get("conclusion") != "success"
            for run in workflows
        )
        or not required.issubset(successful)
    ):
        raise ValueError("exact-SHA CI evidence is absent or non-green")


def preflight(root: Path, contract: dict[str, Any]) -> None:
    code_sha = _verify_frozen_git_inputs(root, contract)
    _verify_exact_sha_ci(root, contract, code_sha)
    budget_config = load_json(root / contract["resource_budget_config"])
    registered = budget_config["resource_budget_registration"].get("values")
    if registered != contract["resource_budgets"]:
        raise ValueError("registered resource budgets differ from paired contract")
    tags_endpoint = contract["endpoint"].removesuffix("/api/generate") + "/api/tags"
    with urllib.request.urlopen(tags_endpoint, timeout=30) as response:
        tags = json.loads(response.read().decode())
    matching = [model for model in tags.get("models", []) if model.get("name") == contract["model"]]
    if len(matching) != 1 or matching[0].get("digest") != contract["model_digest"]:
        raise ValueError("local provider model digest differs from paired contract")


def _candidate_case(
    source: dict[str, Any], pair_id: str, record: dict[str, Any]
) -> dict[str, Any]:
    case = copy.deepcopy(source)
    case["case_id"] = pair_id
    response = record.get("response", {})
    validation = record.get("validation", {})
    spec = validation.get("spec") if validation.get("valid") else None
    claims = spec.get("claims", []) if isinstance(spec, dict) else []
    claimed = list(
        dict.fromkeys(
            claim["id"]
            for claim in claims
            if isinstance(claim, dict) and isinstance(claim.get("id"), str)
        )
    )
    required_ids = {claim["id"] for claim in source["required_claims"]}
    verified = [
        copy.deepcopy(binding)
        for binding in source["observation"]["verified_claims"]
        if binding["claim_id"] in required_ids and binding["claim_id"] in claimed
    ]
    ollama = response.get("response", {}) if response.get("status") == "completed" else {}
    wall_ns = ollama.get("total_duration", response.get("client_wall_time_ns", 0))
    observation = copy.deepcopy(source["observation"])
    observation.update(
        {
            "claimed_claim_ids": claimed,
            "verified_claims": verified,
            "schema_valid": bool(validation.get("valid")),
            "wall_time_ms": max(0, int(wall_ns) // 1_000_000),
            "verify_runtime_ms": 0,
            "input_tokens": max(0, int(ollama.get("prompt_eval_count", 0))),
            "output_tokens": max(0, int(ollama.get("eval_count", 0))),
            "planner_calls": 1,
            "retries": 0,
            "repairs": 0,
            "source_reference": record["record_path"],
        }
    )
    case["observation"] = observation
    return case


def run_campaign(
    *,
    root: Path,
    corpus_path: Path,
    contract_path: Path,
    schema_path: Path,
    prompt_path: Path,
    validator: Path,
    run_dir: Path,
    execution_root: Path | None = None,
    limit: int | None = None,
) -> dict[str, Any]:
    corpus = load_json(corpus_path)
    contract = load_json(contract_path)
    schema = load_json(schema_path)
    prompt = prompt_path.read_text(encoding="utf-8")
    proposal_mode = contract.get("proposal_contract", {}).get("mode", "legacy_v1")
    if proposal_mode not in {"legacy_v1", "phase6_preflight_v2"}:
        raise ValueError(f"unsupported proposal contract mode: {proposal_mode}")
    selected_intents = set(
        contract.get("proposal_contract", {}).get(
            "selected_intents", ["create", "fix", "investigate"]
        )
    )
    selected_cases = [
        case for case in corpus["cases"] if case["intent"] in selected_intents
    ]
    if not selected_cases:
        raise ValueError("proposal contract selects no corpus cases")
    oracle_execution = contract.get("oracle_execution", {"enabled": False})
    adapters: list[dict[str, Any]] = []
    snapshot_cases: dict[str, dict[str, Any]] = {}
    if oracle_execution.get("enabled"):
        if proposal_mode != "phase6_preflight_v2" or execution_root is None:
            raise ValueError("v2 oracle execution requires an explicit execution root")
        adapter_registry = load_json(root / oracle_execution["adapter_registry"])
        snapshot_registry = load_json(root / oracle_execution["snapshot_registry"])
        registry_errors = validate_adapter_registry(
            adapters=adapter_registry, corpus=corpus
        ) + validate_snapshot_registry(
            root=root, registry=snapshot_registry, corpus=corpus
        )
        if registry_errors:
            raise ValueError(
                "invalid v2 artifact registry:\n- " + "\n- ".join(registry_errors)
            )
        adapters = adapter_registry["adapters"]
        snapshot_cases = {case["case_id"]: case for case in snapshot_registry["cases"]}
    preflight(root, contract)
    if contract["execution"] != {
        "concurrency": 1,
        "sharding": False,
        "resume_requires_valid_hash_chain": True,
    }:
        raise ValueError("live runner requires the frozen single-process execution contract")
    run_lock = _acquire_run_lock(run_dir, contract["integrity"]["exclusive_run_lock"])
    artifact_workspaces = {}
    if oracle_execution.get("enabled"):
        workspace_root = execution_root / contract["contract_id"]
        artifact_workspaces = {
            case["case_id"]: prepare_snapshot_workspace(
                root=root,
                snapshot_case=snapshot_cases[case["case_id"]],
                destination=workspace_root / case["case_id"],
            )
            for case in selected_cases
        }
    shape_examples = {
        intent: (root / path).read_text(encoding="utf-8")
        for intent, path in contract["generation"]["shape_examples"].items()
    }
    manifest_path = run_dir / "campaign-manifest.json"
    frozen = {
        "schema_version": "commandagent.goal_verify.phase6_live_manifest.v0",
        "contract": str(contract_path.relative_to(root)),
        "contract_sha256": sha256_file(contract_path),
        "corpus": str(corpus_path.relative_to(root)),
        "corpus_sha256": sha256_file(corpus_path),
        "schema_sha256": sha256_file(schema_path),
        "prompt_sha256": sha256_file(prompt_path),
        "resource_budget_config_sha256": sha256_file(
            root / contract["resource_budget_config"]
        ),
        "exact_sha_ci_evidence": contract["exact_sha_ci_evidence"],
        "exact_sha_ci_evidence_sha256": sha256_file(
            root / contract["exact_sha_ci_evidence"]
        ),
        "runner_source_sha256": {
            path: sha256_file(root / path) for path in contract["runner_sources"]
        },
        "validator_binary_sha256": sha256_file(validator),
        "commandagent_binary_sha256": sha256_file(root / "target/release/commandagent"),
        "shape_example_sha256": {
            intent: sha256_file(root / path)
            for intent, path in contract["generation"]["shape_examples"].items()
        },
        "code_sha": contract["code_sha"],
        "model": contract["model"],
        "model_digest": contract["model_digest"],
        "samples_per_cell": contract["samples_per_cell"],
        "exclusion_rules": contract["pairing"]["exclusion_rules"],
        "record_ledger": contract["integrity"]["record_ledger"],
        "exclusive_run_lock": contract["integrity"]["exclusive_run_lock"],
    }
    if proposal_mode == "phase6_preflight_v2":
        frozen["proposal_contract_mode"] = proposal_mode
        frozen["selected_intents"] = sorted(selected_intents)
        frozen["oracle_execution"] = oracle_execution
        if oracle_execution.get("enabled"):
            frozen["adapter_registry_sha256"] = sha256_file(
                root / oracle_execution["adapter_registry"]
            )
            frozen["snapshot_registry_sha256"] = sha256_file(
                root / oracle_execution["snapshot_registry"]
            )
            frozen["execution_root"] = str(execution_root.resolve())
    if manifest_path.exists():
        if load_json(manifest_path) != frozen:
            raise ValueError("campaign manifest differs from the frozen contract")
    else:
        _atomic_json(manifest_path, frozen)

    ledger_path = run_dir / contract["integrity"]["record_ledger"]
    ledger_entries, ledger_head = _load_record_ledger(
        root=root, run_dir=run_dir, ledger_path=ledger_path
    )

    baseline_cases: list[dict[str, Any]] = []
    candidate_cases: list[dict[str, Any]] = []
    completed_count = 0
    target = len(selected_cases) * int(contract["samples_per_cell"])
    for source_index, source in enumerate(selected_cases):
        for sample_index in range(1, int(contract["samples_per_cell"]) + 1):
            pair_id = f"{source['case_id']}--pair-{sample_index:02d}"
            relative_record = Path("raw") / source["case_id"] / f"pair-{sample_index:02d}.json"
            record_path = run_dir / relative_record
            record_reference = str(record_path.relative_to(root))
            if record_path.exists():
                if record_reference not in ledger_entries:
                    raise ValueError(f"existing record lacks ledger entry: {record_reference}")
                record = load_json(record_path)
            else:
                if record_reference in ledger_entries:
                    raise ValueError(f"ledger references missing record: {record_reference}")
                if limit is not None and completed_count >= limit:
                    result = _write_corpora(
                        corpus,
                        baseline_cases,
                        candidate_cases,
                        run_dir,
                        completed_count,
                        target,
                        ledger_head,
                    )
                    run_lock.close()
                    return result
                request_id = f"{contract['contract_id']}:{pair_id}"
                prompt_builder = (
                    build_v2_prompt
                    if proposal_mode == "phase6_preflight_v2"
                    else build_prompt
                )
                generated_prompt = prompt_builder(
                    prompt, source, request_id, shape_examples[source["intent"]]
                )
                response = request_ollama(
                    endpoint=contract["endpoint"],
                    model=contract["model"],
                    prompt=generated_prompt,
                    schema=schema,
                    seed=int(contract["generation"]["seed_base"])
                    + source_index * int(contract["samples_per_cell"])
                    + sample_index,
                    temperature=float(contract["generation"]["temperature"]),
                    num_predict=int(contract["generation"]["num_predict"]),
                    timeout_sec=int(contract["generation"]["request_timeout_sec"]),
                    keep_alive=str(contract["generation"]["keep_alive"]),
                    think=bool(contract["generation"]["think"]),
                )
                validation: dict[str, Any] = {
                    "valid": False,
                    "spec": None,
                    "errors": [response.get("error_kind", "provider_error")],
                }
                normalized_raw = None
                if response["status"] == "completed":
                    raw = response["response"].get("response", "")
                    try:
                        if proposal_mode == "phase6_preflight_v2":
                            normalized_raw = normalize_v2_proposal(
                                raw,
                                case=source,
                                model=contract["model"],
                                request_id=request_id,
                            )
                        else:
                            normalized_raw = normalize_proposal(
                                raw, model=contract["model"], request_id=request_id
                            )
                        validation = validate_proposal(
                            validator=validator,
                            goal=source["goal"],
                            intent=source["intent"],
                            normalized_raw=normalized_raw,
                        )
                    except (TypeError, ValueError, json.JSONDecodeError) as error:
                        validation = {
                            "valid": False,
                            "spec": None,
                            "errors": [f"proposal_parse_failed:{error}"],
                        }
                execution_classification = []
                oracle_evaluations = []
                candidate_execution = {"unmatched_candidate_oracle_ids": []}
                if proposal_mode == "phase6_preflight_v2" and validation.get("valid"):
                    execution_classification = [
                        classify_oracle_execution(oracle)
                        for oracle in validation["spec"]["oracles"]
                    ]
                    if oracle_execution.get("enabled"):
                        candidate_execution = evaluate_spec_against_snapshot(
                            case_id=source["case_id"],
                            spec=validation["spec"],
                            adapters=adapters,
                            workspace=artifact_workspaces[source["case_id"]],
                            sandbox_runner=run_macos_sandbox,
                        )
                        oracle_evaluations = candidate_execution["evaluations"]
                        for evaluation in oracle_evaluations:
                            evaluation["arm"] = "candidate"
                baseline_spec = None
                baseline_evaluations = []
                unmatched_candidate_oracle_ids = []
                if proposal_mode == "phase6_preflight_v2" and oracle_execution.get(
                    "enabled"
                ):
                    baseline_spec = build_registered_baseline_spec(
                        case=source, adapters=adapters
                    )
                    baseline_execution = evaluate_spec_against_snapshot(
                        case_id=source["case_id"],
                        spec=baseline_spec,
                        adapters=adapters,
                        workspace=artifact_workspaces[source["case_id"]],
                        sandbox_runner=run_macos_sandbox,
                    )
                    baseline_evaluations = baseline_execution["evaluations"]
                    for evaluation in baseline_evaluations:
                        evaluation["arm"] = "baseline"
                    unmatched_candidate_oracle_ids = candidate_execution[
                        "unmatched_candidate_oracle_ids"
                    ]
                record = {
                    "schema_version": "commandagent.goal_verify.phase6_live_record.v0",
                    "pair_id": pair_id,
                    "source_case_id": source["case_id"],
                    "record_path": record_reference,
                    "response": response,
                    "normalized_proposal": normalized_raw,
                    "validation": validation,
                    "execution_classification": execution_classification,
                    "oracle_evaluations": oracle_evaluations,
                    "unmatched_candidate_oracle_ids": unmatched_candidate_oracle_ids,
                    "baseline_spec": baseline_spec,
                    "baseline_oracle_evaluations": baseline_evaluations,
                }
                _atomic_json(record_path, record)
                ledger_head = _append_record_ledger(
                    ledger_path=ledger_path,
                    entries=ledger_entries,
                    previous=ledger_head,
                    pair_id=pair_id,
                    source_case_id=source["case_id"],
                    record_reference=record_reference,
                    record_path=record_path,
                )
            _validate_record_identity(
                record,
                pair_id=pair_id,
                source_case_id=source["case_id"],
                record_reference=record_reference,
            )
            if proposal_mode == "phase6_preflight_v2" and oracle_execution.get(
                "enabled"
            ):
                baseline = snapshot_case_v2(
                    source=source,
                    pair_id=pair_id,
                    spec=record["baseline_spec"],
                    evaluations=record["baseline_oracle_evaluations"],
                    source_reference=record_reference,
                )
                response = record.get("response", {})
                ollama = (
                    response.get("response", {})
                    if response.get("status") == "completed"
                    else {}
                )
                wall_ns = ollama.get(
                    "total_duration", response.get("client_wall_time_ns", 0)
                )
                candidate = snapshot_case_v2(
                    source=source,
                    pair_id=pair_id,
                    spec=record["validation"].get("spec")
                    if record["validation"].get("valid")
                    else None,
                    evaluations=record["oracle_evaluations"],
                    source_reference=record_reference,
                    proposal_wall_time_ms=int(wall_ns or 0) // 1_000_000,
                    input_tokens=int(ollama.get("prompt_eval_count", 0) or 0),
                    output_tokens=int(ollama.get("eval_count", 0) or 0),
                    schema_valid=bool(record["validation"].get("valid")),
                )
            else:
                baseline = copy.deepcopy(source)
                baseline["case_id"] = pair_id
                candidate = (
                    candidate_case_v2(source, pair_id, record)
                    if proposal_mode == "phase6_preflight_v2"
                    else _candidate_case(source, pair_id, record)
                )
            baseline_cases.append(baseline)
            candidate_cases.append(candidate)
            completed_count += 1
            _write_corpora(
                corpus,
                baseline_cases,
                candidate_cases,
                run_dir,
                completed_count,
                target,
                ledger_head,
            )
    result = _write_corpora(
        corpus,
        baseline_cases,
        candidate_cases,
        run_dir,
        completed_count,
        target,
        ledger_head,
    )
    run_lock.close()
    return result


def _write_corpora(
    source_corpus: dict[str, Any],
    baseline_cases: list[dict[str, Any]],
    candidate_cases: list[dict[str, Any]],
    run_dir: Path,
    completed: int,
    target: int,
    ledger_head: str,
) -> dict[str, Any]:
    baseline = copy.deepcopy(source_corpus)
    baseline["cases"] = baseline_cases
    candidate = copy.deepcopy(source_corpus)
    preflight_v2 = bool(baseline_cases) and all(
        case.get("preflight_only", {}).get("measurement")
        == "proposal_oracle_contract_integration"
        for case in baseline_cases
    )
    if preflight_v2:
        baseline["annotation_protocol"] = {
            "method": "deterministic registered proposal evaluated against synthetic snapshot",
            "label_author": "phase6-v2-host",
            "reviewer": "pending-semantic-blind-review",
            "reviewed_at": "pending",
            "status": "pending",
            "disagreements": [],
        }
        candidate_method = (
            "raw provider proposal independently evaluated against identical synthetic snapshot; "
            "variant-blind review pending"
        )
    else:
        candidate_method = (
            "provider generation projected mechanically; variant-blind review pending"
        )
    candidate["annotation_protocol"] = {
        "method": candidate_method,
        "label_author": "phase6-live-runner",
        "reviewer": "pending-blind-review",
        "reviewed_at": "pending",
        "status": "pending",
        "disagreements": [],
    }
    candidate["cases"] = candidate_cases
    _atomic_json(run_dir / "baseline-corpus.json", baseline)
    _atomic_json(run_dir / "candidate-corpus.draft.json", candidate)
    summary = {
        "completed_pairs": completed,
        "target_pairs": target,
        "complete": completed == target,
        "valid_candidate_specs": sum(
            1 for case in candidate_cases if case["observation"]["schema_valid"]
        ),
        "record_ledger_entries": completed,
        "record_ledger_head_sha256": ledger_head,
    }
    _atomic_json(run_dir / "campaign-summary.json", summary)
    return summary
