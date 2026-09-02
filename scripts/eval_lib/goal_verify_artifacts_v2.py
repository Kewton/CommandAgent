from __future__ import annotations

import copy
import hashlib
import json
import shutil
from collections.abc import Callable
from pathlib import Path
from typing import Any

from eval_lib.goal_verify_v2 import (
    canonicalize_v2_proposal,
    concretize_registered_command,
    evaluate_concretized_command,
)


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def validate_snapshot_registry(
    *, root: Path, registry: dict[str, Any], corpus: dict[str, Any]
) -> list[str]:
    errors = []
    expected_cases = {
        case["case_id"]
        for case in corpus["cases"]
        if case["intent"] in {"create", "fix"}
    }
    cases = registry.get("cases")
    if not isinstance(cases, list):
        return ["snapshot registry cases must be an array"]
    case_ids = [case.get("case_id") for case in cases if isinstance(case, dict)]
    if len(case_ids) != len(set(case_ids)):
        errors.append("snapshot registry contains duplicate case IDs")
    if set(case_ids) != expected_cases:
        errors.append("snapshot registry case IDs differ from create/fix corpus cases")
    for case in cases:
        for file in case.get("files", []):
            source = root / file.get("source", "")
            if not source.is_file():
                errors.append(f"snapshot source missing: {file.get('source')}")
            elif _sha256(source) != file.get("sha256"):
                errors.append(f"snapshot source hash mismatch: {file.get('source')}")
            destination = Path(file.get("destination", ""))
            if (
                not file.get("destination")
                or destination.is_absolute()
                or ".." in destination.parts
            ):
                errors.append(f"snapshot destination unsafe: {file.get('destination')}")
    return errors


def validate_adapter_registry(
    *, adapters: dict[str, Any], corpus: dict[str, Any]
) -> list[str]:
    errors = []
    cases = {
        case["case_id"]: {claim["id"] for claim in case["required_claims"]}
        for case in corpus["cases"]
        if case["intent"] in {"create", "fix"}
    }
    rows = adapters.get("adapters")
    if not isinstance(rows, list) or not rows:
        return ["adapter registry must contain adapters"]
    ids = [row.get("adapter_id") for row in rows if isinstance(row, dict)]
    if len(ids) != len(set(ids)):
        errors.append("adapter registry contains duplicate adapter IDs")
    covered = set()
    for row in rows:
        case_id = row.get("case_id")
        claim_id = row.get("claim_id")
        if case_id not in cases or claim_id not in cases[case_id]:
            errors.append(f"adapter references unknown claim: {case_id}/{claim_id}")
        else:
            covered.add((case_id, claim_id))
        proposal = row.get("proposal")
        executor = row.get("executor")
        if not isinstance(proposal, dict) or not all(
            isinstance(proposal.get(field), list) and proposal[field]
            for field in ("strategies", "polarities", "observation_kinds")
        ):
            errors.append(f"adapter proposal contract invalid: {row.get('adapter_id')}")
        if not isinstance(executor, dict) or not executor.get("kind"):
            errors.append(f"adapter executor invalid: {row.get('adapter_id')}")
    expected = {
        (case_id, claim_id)
        for case_id, claim_ids in cases.items()
        for claim_id in claim_ids
    }
    if covered != expected:
        errors.append("adapter registry does not cover every create/fix required claim")
    return errors


def prepare_snapshot_workspace(
    *, root: Path, snapshot_case: dict[str, Any], destination: Path
) -> Path:
    expected_snapshot = (
        json.dumps(
            snapshot_case["artifact"], ensure_ascii=False, indent=2, sort_keys=True
        )
        + "\n"
    )
    if destination.exists() and any(destination.iterdir()):
        snapshot = destination / "artifact-snapshot.json"
        marker = destination / ".commandagent-eval-isolated.json"
        if not snapshot.is_file() or not marker.is_file():
            raise FileExistsError(
                f"snapshot destination is not reusable: {destination}"
            )
        marker_value = json.loads(marker.read_text(encoding="utf-8"))
        if (
            snapshot.read_text(encoding="utf-8") != expected_snapshot
            or marker_value.get("case_id") != snapshot_case["case_id"]
            or marker_value.get("snapshot_sha256") != _sha256(snapshot)
        ):
            raise ValueError(f"prepared snapshot integrity mismatch: {destination}")
        for file in snapshot_case.get("files", []):
            target = destination / file["destination"]
            if not target.is_file() or _sha256(target) != file["sha256"]:
                raise ValueError(f"prepared snapshot file mismatch: {target}")
        return destination
    destination.mkdir(parents=True, exist_ok=True)
    for file in snapshot_case.get("files", []):
        source = root / file["source"]
        if _sha256(source) != file["sha256"]:
            raise ValueError(f"snapshot source hash mismatch: {file['source']}")
        target = destination / file["destination"]
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
    snapshot = destination / "artifact-snapshot.json"
    snapshot.write_text(expected_snapshot, encoding="utf-8")
    marker = destination / ".commandagent-eval-isolated.json"
    marker.write_text(
        json.dumps(
            {
                "schema_version": "commandagent.goal_verify.isolated_snapshot.v2",
                "case_id": snapshot_case["case_id"],
                "snapshot_sha256": _sha256(snapshot),
            },
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    return destination


def _pointer(value: Any, pointer: str) -> Any:
    current = value
    if pointer == "":
        return current
    if not pointer.startswith("/"):
        raise ValueError(f"invalid JSON pointer: {pointer}")
    for raw_part in pointer[1:].split("/"):
        part = raw_part.replace("~1", "/").replace("~0", "~")
        if isinstance(current, list):
            current = current[int(part)]
        elif isinstance(current, dict):
            current = current[part]
        else:
            raise TypeError(f"JSON pointer traverses a scalar: {pointer}")
    return current


def _proposal_matches(adapter: dict[str, Any], oracle: dict[str, Any]) -> bool:
    proposal = adapter["proposal"]
    observation = oracle.get("observation")
    if not isinstance(observation, dict):
        return False
    if oracle.get("strategy") not in proposal["strategies"]:
        return False
    if oracle.get("expected_polarity") not in proposal["polarities"]:
        return False
    if observation.get("kind") not in proposal["observation_kinds"]:
        return False
    expected = observation.get("expected")
    if "expected_values" in proposal and expected not in proposal["expected_values"]:
        return False
    return not (
        "expected_contains" in proposal
        and (
            not isinstance(expected, str)
            or any(
                fragment not in expected
                for fragment in proposal["expected_contains"]
            )
        )
    )


def _snapshot_evaluation(
    *, adapter: dict[str, Any], snapshot: dict[str, Any], oracle_id: str
) -> dict[str, Any]:
    executor = adapter["executor"]
    kind = executor["kind"]
    if kind == "snapshot_value":
        actual = _pointer(snapshot, executor["pointer"])
        passed = actual == executor["expected"]
    elif kind == "snapshot_multi_value":
        actual = [_pointer(snapshot, check["pointer"]) for check in executor["checks"]]
        passed = all(
            value == check["expected"]
            for value, check in zip(actual, executor["checks"], strict=True)
        )
    elif kind == "snapshot_equal":
        left = _pointer(snapshot, executor["left_pointer"])
        right = _pointer(snapshot, executor["right_pointer"])
        actual = left == right
        passed = actual == executor["expected"]
    else:
        raise ValueError(f"not a snapshot executor: {kind}")
    return {
        "oracle_id": oracle_id,
        "adapter_id": adapter["adapter_id"],
        "executed": True,
        "result": "pass" if passed else "fail",
        "observed_strength": executor["observed_strength"],
        "reason": "snapshot_observation_match"
        if passed
        else "snapshot_observation_mismatch",
        "actual": actual,
        "evidence_provenance": "preregistered_synthetic_snapshot",
    }


def evaluate_spec_against_snapshot(
    *,
    case_id: str,
    spec: dict[str, Any],
    adapters: list[dict[str, Any]],
    workspace: Path,
    sandbox_runner: Callable[[dict[str, Any]], dict[str, Any]],
) -> dict[str, Any]:
    snapshot = json.loads(
        (workspace / "artifact-snapshot.json").read_text(encoding="utf-8")
    )
    candidate_oracles = spec.get("oracles", [])
    unused = set(range(len(candidate_oracles)))
    evaluations = []
    for adapter in [row for row in adapters if row["case_id"] == case_id]:
        matching = [
            index
            for index in sorted(unused)
            if candidate_oracles[index].get("claim_id") == adapter["claim_id"]
            and _proposal_matches(adapter, candidate_oracles[index])
        ]
        if not matching:
            evaluations.append(
                {
                    "oracle_id": None,
                    "adapter_id": adapter["adapter_id"],
                    "claim_id": adapter["claim_id"],
                    "executed": False,
                    "result": "blocked",
                    "observed_strength": None,
                    "reason": "candidate_oracle_contract_not_matched",
                }
            )
            continue
        index = matching[0]
        unused.remove(index)
        oracle = candidate_oracles[index]
        if adapter["executor"]["kind"] == "sandbox_command":
            concrete_adapter = {
                "oracle_id": oracle["id"],
                **{
                    key: value
                    for key, value in adapter["executor"].items()
                    if key != "kind"
                },
            }
            plan = concretize_registered_command(
                oracle={**oracle, "strategy": "command"},
                adapter=concrete_adapter,
                workspace_root=workspace,
            )
            evaluation = evaluate_concretized_command(plan, runner=sandbox_runner)
            evaluation["adapter_id"] = adapter["adapter_id"]
            evaluation["evidence_provenance"] = "preregistered_synthetic_snapshot"
        else:
            evaluation = _snapshot_evaluation(
                adapter=adapter, snapshot=snapshot, oracle_id=oracle["id"]
            )
        evaluation["claim_id"] = adapter["claim_id"]
        evaluations.append(evaluation)
    return {
        "case_id": case_id,
        "evaluations": evaluations,
        "unmatched_candidate_oracle_ids": [
            candidate_oracles[index].get("id") for index in sorted(unused)
        ],
    }


def _baseline_observation(adapter: dict[str, Any]) -> dict[str, Any]:
    proposal = adapter["proposal"]
    kind = proposal["observation_kinds"][0]
    if kind == "exit_code":
        expected = adapter["executor"].get("expected", 0)
        return {"kind": kind, "expected": expected if isinstance(expected, int) else 0}
    if kind in {"stdout", "stderr", "dom", "interaction"}:
        expected = proposal.get("expected_values")
        if isinstance(expected, list):
            expected = expected[0]
        if expected is None:
            expected = proposal.get("expected_contains")
            if isinstance(expected, list):
                expected = " ".join(str(value) for value in expected)
        return {"kind": kind, "expected": str(expected or adapter["adapter_id"])}
    if kind == "file":
        return {"kind": "file", "path": "artifact-snapshot.json", "exists": True}
    if kind == "http_status":
        return {"kind": kind, "expected": 200}
    if kind == "existing_binding":
        return {"kind": kind, "artifact_path": "artifact-snapshot.json"}
    raise ValueError(f"unsupported baseline observation kind: {kind}")


def build_registered_baseline_spec(
    *, case: dict[str, Any], adapters: list[dict[str, Any]]
) -> dict[str, Any]:
    """Build a deterministic proposal arm from the reviewed executable annotations."""
    case_adapters = [row for row in adapters if row["case_id"] == case["case_id"]]
    by_claim: dict[str, list[dict[str, Any]]] = {
        claim["id"]: [] for claim in case["required_claims"]
    }
    for adapter in case_adapters:
        by_claim[adapter["claim_id"]].append(adapter)
    if any(not rows for rows in by_claim.values()):
        raise ValueError(f"baseline adapters do not cover case {case['case_id']}")
    claims = []
    oracles = []
    for required in case["required_claims"]:
        claim_id = required["id"]
        claim_oracle_ids = []
        for index, adapter in enumerate(by_claim[claim_id], 1):
            oracle_id = f"baseline-{claim_id}-{index:02d}"
            claim_oracle_ids.append(oracle_id)
            oracles.append(
                {
                    "id": oracle_id,
                    "claim_id": claim_id,
                    "strategy": adapter["proposal"]["strategies"][0],
                    "expected_polarity": adapter["proposal"]["polarities"][0],
                    "minimum_strength": required["min_strength"],
                    "observed_strength": None,
                    "setup": {"argv": [], "cwd": ".", "fixture_paths": []},
                    "input": {"kind": "none"},
                    "observation": _baseline_observation(adapter),
                    "timeout_ms": 5000,
                    "lifecycle": "proposed",
                    "result": "unverified",
                    "lineage": {
                        "proposed_binding_sha256": "0" * 64,
                        "concretized_binding_sha256": "0" * 64,
                        "semantic_equivalence": True,
                        "repair_kind": None,
                    },
                }
            )
        claims.append(
            {
                "id": claim_id,
                "origin": {"source_kind": "goal", "start_byte": 0, "end_byte": 1},
                "normalized_requirement": required["oracle"]["expected"],
                "required": True,
                "kind": "behavior",
                "oracle_ids": claim_oracle_ids,
            }
        )
    raw = {
        "schema_version": "commandagent.verification_spec.v0",
        "prompt_version": "commandagent.verification_spec.prompt.v0",
        "goal": case["goal"],
        "intent": case["intent"],
        "profile": case["profile"],
        "generation": {
            "provider": "deterministic-registered-baseline",
            "model": "none",
            "request_id": f"baseline:{case['case_id']}",
            "raw_response_sha256": "",
        },
        "claims": claims,
        "oracles": oracles,
    }
    return canonicalize_v2_proposal(json.dumps(raw, ensure_ascii=False), case=case)


def snapshot_case_v2(
    *,
    source: dict[str, Any],
    pair_id: str,
    spec: dict[str, Any] | None,
    evaluations: list[dict[str, Any]],
    source_reference: str,
    proposal_wall_time_ms: int = 0,
    input_tokens: int = 0,
    output_tokens: int = 0,
    schema_valid: bool = True,
) -> dict[str, Any]:
    """Build preflight observations without projecting product authority."""
    case = copy.deepcopy(source)
    case["case_id"] = pair_id
    claimed = [claim["id"] for claim in (spec or {}).get("claims", [])]
    by_claim: dict[str, list[dict[str, Any]]] = {
        claim["id"]: [] for claim in source["required_claims"]
    }
    for evaluation in evaluations:
        claim_id = evaluation.get("claim_id")
        if claim_id in by_claim:
            by_claim[claim_id].append(evaluation)
    strength_order = {"weak": 0, "deterministic": 1, "runtime": 2}
    verified = []
    for claim_id, rows in by_claim.items():
        if rows and all(row.get("result") == "pass" for row in rows):
            strengths = [row.get("observed_strength") for row in rows]
            if all(strength in strength_order for strength in strengths):
                weakest = min(strengths, key=strength_order.__getitem__)
                verified.append(
                    {"claim_id": claim_id, "strength": weakest, "executed": True}
                )
    verify_runtime_ms = sum(int(row.get("runtime_ms") or 0) for row in evaluations)
    case["observation"] = {
        "claimed_claim_ids": claimed,
        "verified_claims": verified,
        "verdict": "unverified",
        "final_acceptance": False,
        "schema_valid": schema_valid,
        "wall_time_ms": proposal_wall_time_ms + verify_runtime_ms,
        "verify_runtime_ms": verify_runtime_ms,
        "input_tokens": input_tokens,
        "output_tokens": output_tokens,
        "planner_calls": 0,
        "retries": 0,
        "repairs": 0,
        "flake_trials": [],
        "policy_rejection": False,
        "dependency_blocked": any(
            row.get("result") in {"blocked", "oracle_error"} for row in evaluations
        ),
        "source_reference": source_reference,
    }
    case["preflight_only"] = {
        "measurement": "proposal_oracle_contract_integration",
        "product_task_success_evidence": False,
        "artifact_provenance": "preregistered_synthetic_snapshot",
    }
    return case
