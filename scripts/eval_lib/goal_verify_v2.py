from __future__ import annotations

import copy
import hashlib
import json
import re
from collections.abc import Callable
from pathlib import Path
from typing import Any

CLAIM_KINDS = (
    "behavior",
    "state",
    "negative_condition",
    "regression",
    "reproducer_observation",
    "diagnosis_binding",
)
INPUT_KINDS = ("none", "text", "fixture", "http", "dom")
OBSERVATION_KINDS = (
    "exit_code",
    "stdout",
    "stderr",
    "file",
    "http_status",
    "dom",
    "interaction",
    "existing_binding",
)
STRATEGIES = (
    "command",
    "fixture",
    "exit_code",
    "stdout",
    "stderr",
    "file",
    "http",
    "dom",
    "interaction",
    "existing_fix_evidence",
    "existing_investigation_binding",
)
UNVERIFIABLE_REASONS = (
    "executor_capability_unavailable",
    "safe_execution_unavailable",
    "workspace_binding_unavailable",
)

_SAFE_ID = re.compile(r"^[A-Za-z0-9_.-]{1,64}$")


def _canonical_json(value: Any) -> bytes:
    return json.dumps(
        value, ensure_ascii=False, separators=(",", ":"), sort_keys=True
    ).encode("utf-8")


def _evidence_registry(case: dict[str, Any]) -> list[dict[str, Any]]:
    override = case.get("existing_evidence_registry")
    if case["intent"] == "create":
        return []
    registry = []
    fix_requirements = {
        "before-after": "before_fails",
        "regressions": "no_regression",
        "exact-reproducer": "before_fails",
        "after-executed": "after_passes",
        "full-regression-set": "no_regression",
        "bug-reproducer": "after_passes",
    }
    investigation_requirements = {
        "reproducer-defect": "reproducer_fails",
        "location-exists": "diagnosis_bound",
        "causal-intervention": "diagnosis_bound",
        "intent-unresolved": "diagnosis_bound",
        "timeout-boundary": "reproducer_fails",
    }
    for claim in case["required_claims"]:
        if case["intent"] == "fix":
            requirement_id = fix_requirements[claim["id"]]
            registry.append(
                {
                    "claim_id": claim["id"],
                    "artifact_path": "evidence/fix-evidence.json",
                    "requirement_id": requirement_id,
                    "stage": "before" if requirement_id == "before_fails" else "after",
                    "expected_polarity": (
                        "failure" if requirement_id == "before_fails" else "success"
                    ),
                    "lineage": case["case_id"],
                    "epoch": 1,
                }
            )
        else:
            requirement_id = investigation_requirements[claim["id"]]
            registry.append(
                {
                    "claim_id": claim["id"],
                    "artifact_path": "evidence/investigation-evidence.json",
                    "requirement_id": requirement_id,
                    "binding_id": f"{case['case_id']}:{claim['id']}",
                    "stage": (
                        "reproduce"
                        if requirement_id == "reproducer_fails"
                        else "diagnosis"
                    ),
                    "lineage": case["case_id"],
                    "epoch": 1,
                }
            )
    if not isinstance(override, list):
        return registry
    override_by_claim = {
        row.get("claim_id"): copy.deepcopy(row)
        for row in override
        if isinstance(row, dict)
    }
    return [override_by_claim.get(row["claim_id"], row) for row in registry]


def build_v2_prompt(
    base_prompt: str, case: dict[str, Any], request_id: str, shape_example: str
) -> str:
    request: dict[str, Any] = {
        "goal": case["goal"],
        "intent": case["intent"],
        "profile": case["profile"],
        "required_claims": copy.deepcopy(case["required_claims"]),
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
    vocabulary = {
        "claim.kind": CLAIM_KINDS,
        "oracle.strategy": STRATEGIES,
        "oracle.input.kind": INPUT_KINDS,
        "oracle.observation.kind": OBSERVATION_KINDS,
    }
    return (
        f"{base_prompt.rstrip()}\n\n"
        "CONTRACT V2 RULES:\n"
        "- Emit exactly the registered required claim IDs in INPUT JSON; do not rename, "
        "omit, or invent claim IDs.\n"
        "- Use only the following closed vocabulary:\n"
        f"{json.dumps(vocabulary, ensure_ascii=False, sort_keys=True)}\n"
        "- Host code owns claim origins, oracle IDs, lifecycle/result, and lineage hashes. "
        "Provider values in those fields are schema-shaped proposals and are ignored.\n"
        "- Use an existing_* strategy only with the matching frozen evidence registry entry.\n\n"
        "The following object is a shape example only. Copy field structure, not values.\n"
        f"SHAPE EXAMPLE:\n{shape_example.rstrip()}\n\n"
        "Return JSON only. Copy goal, intent, profile, generation, and every registered claim "
        "ID from INPUT JSON exactly.\n"
        f"INPUT JSON:\n{json.dumps(request, ensure_ascii=False)}\n"
    )


def _origin_for_claim(case: dict[str, Any], claim_id: str) -> dict[str, Any]:
    if case["intent"] == "create":
        return {
            "source_kind": "goal",
            "start_byte": 0,
            "end_byte": len(case["goal"].encode("utf-8")),
        }
    registry_by_claim = {row["claim_id"]: row for row in _evidence_registry(case)}
    registry = registry_by_claim[claim_id]
    if case["intent"] == "fix":
        return {
            "source_kind": "fix_requirement",
            **{k: v for k, v in registry.items() if k != "claim_id"},
        }
    return {
        "source_kind": "investigation_requirement",
        **{k: v for k, v in registry.items() if k != "claim_id"},
    }


def _oracle_id(claim_id: str, index: int) -> str:
    prefix = re.sub(r"[^A-Za-z0-9_.-]", "-", claim_id)[:40]
    return f"oracle-{prefix}-{index:02d}"


def _binding_hash(oracle: dict[str, Any]) -> str:
    semantic_binding = {
        key: value
        for key, value in oracle.items()
        if key not in {"id", "lineage", "lifecycle", "result", "observed_strength"}
    }
    return hashlib.sha256(_canonical_json(semantic_binding)).hexdigest()


def canonicalize_v2_proposal(
    raw: str, *, case: dict[str, Any], allow_unverifiable_claims: bool = False
) -> dict[str, Any]:
    """Replace provider-owned deterministic fields without repairing semantic choices."""
    value = json.loads(raw)
    if not isinstance(value, dict):
        raise TypeError("provider proposal must be an object")
    claims = value.get("claims")
    oracles = value.get("oracles")
    if not isinstance(claims, list) or not isinstance(oracles, list):
        raise TypeError("claims and oracles must be arrays")

    required_ids = [claim["id"] for claim in case["required_claims"]]
    claimed_ids = [claim.get("id") for claim in claims if isinstance(claim, dict)]
    if len(claimed_ids) != len(claims) or len(set(claimed_ids)) != len(claimed_ids):
        raise ValueError("claim IDs must be present and unique")
    if set(claimed_ids) != set(required_ids) or len(claimed_ids) != len(required_ids):
        raise ValueError(
            "proposal must contain exactly the registered required claim IDs"
        )
    if any(
        not isinstance(claim_id, str) or not _SAFE_ID.fullmatch(claim_id)
        for claim_id in claimed_ids
    ):
        raise ValueError("claim ID is outside the closed ID syntax")

    canonical = copy.deepcopy(value)
    claims_by_id = {claim["id"]: claim for claim in canonical["claims"]}
    oracles_by_claim: dict[str, list[dict[str, Any]]] = {
        claim_id: [] for claim_id in required_ids
    }
    for oracle in canonical["oracles"]:
        if (
            not isinstance(oracle, dict)
            or oracle.get("claim_id") not in oracles_by_claim
        ):
            raise ValueError("oracle must reference a registered claim ID")
        oracles_by_claim[oracle["claim_id"]].append(oracle)
    for claim_id, rows in oracles_by_claim.items():
        reason = claims_by_id[claim_id].get("unverifiable_reason")
        if rows and reason is not None:
            raise ValueError("a claim with an oracle cannot be marked unverifiable")
        if rows:
            continue
        if not allow_unverifiable_claims or reason not in UNVERIFIABLE_REASONS:
            raise ValueError("every required claim must have at least one oracle")

    ordered_oracles: list[dict[str, Any]] = []
    for claim_id in required_ids:
        claim = claims_by_id[claim_id]
        claim["origin"] = _origin_for_claim(case, claim_id)
        claim["required"] = True
        claim["oracle_ids"] = []
        for index, oracle in enumerate(oracles_by_claim[claim_id], 1):
            oracle_id = _oracle_id(claim_id, index)
            oracle["id"] = oracle_id
            oracle["claim_id"] = claim_id
            oracle["observed_strength"] = None
            oracle["lifecycle"] = "proposed"
            oracle["result"] = "unverified"
            digest = _binding_hash(oracle)
            oracle["lineage"] = {
                "proposed_binding_sha256": digest,
                "concretized_binding_sha256": digest,
                "semantic_equivalence": True,
                "repair_kind": None,
            }
            claim["oracle_ids"].append(oracle_id)
            ordered_oracles.append(oracle)
    canonical["claims"] = [claims_by_id[claim_id] for claim_id in required_ids]
    canonical["oracles"] = ordered_oracles
    return canonical


def normalize_v2_proposal(
    raw: str,
    *,
    case: dict[str, Any],
    model: str,
    request_id: str,
    allow_unverifiable_claims: bool = False,
) -> str:
    value = canonicalize_v2_proposal(
        raw, case=case, allow_unverifiable_claims=allow_unverifiable_claims
    )
    value["generation"] = {
        "provider": "ollama-local",
        "model": model,
        "request_id": request_id,
        "raw_response_sha256": hashlib.sha256(raw.encode("utf-8")).hexdigest(),
    }
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"))


def classify_oracle_execution(oracle: dict[str, Any]) -> dict[str, Any]:
    """Return an execution boundary decision; this function never launches a process."""
    strategy = oracle.get("strategy")
    if strategy in {"existing_fix_evidence", "existing_investigation_binding"}:
        lane = "reference_validation"
        executor = "frozen_evidence_registry"
    elif strategy in {"command", "exit_code", "stdout", "stderr", "fixture"}:
        lane = "executable"
        executor = "bounded_command_after_concretization"
    else:
        lane = "executor_unavailable"
        executor = None
    return {
        "oracle_id": oracle.get("id"),
        "lane": lane,
        "executor": executor,
        "may_execute_raw_provider_argv": False,
    }


def resolve_evidence_reference(*, execution_root: Path, artifact_path: str) -> Path:
    root = execution_root.resolve()
    candidate = (root / artifact_path).resolve()
    if not candidate.is_relative_to(root) or not candidate.is_file():
        raise ValueError("evidence reference is missing or escapes the execution root")
    return candidate


def _plan_hash(plan: dict[str, Any]) -> str:
    unsigned = {key: value for key, value in plan.items() if key != "plan_sha256"}
    return hashlib.sha256(_canonical_json(unsigned)).hexdigest()


def concretize_registered_command(
    *,
    oracle: dict[str, Any],
    adapter: dict[str, Any],
    workspace_root: Path,
) -> dict[str, Any]:
    """Create a host-owned command plan without copying provider argv or cwd."""
    if oracle.get("strategy") not in {"command", "exit_code", "stdout", "stderr"}:
        raise ValueError("oracle is not eligible for the command executor")
    if adapter.get("oracle_id") != oracle.get("id"):
        raise ValueError("adapter is not registered for this oracle")
    argv = adapter.get("argv")
    if (
        not isinstance(argv, list)
        or not argv
        or any(not isinstance(arg, str) or not arg for arg in argv)
    ):
        raise ValueError("registered adapter argv must be a non-empty string array")
    if Path(argv[0]).is_absolute() or any("\x00" in arg for arg in argv):
        raise ValueError("registered adapter argv is unsafe")
    root = workspace_root.resolve()
    cwd = (root / adapter.get("cwd", ".")).resolve()
    if not cwd.is_relative_to(root) or not cwd.is_dir():
        raise ValueError("registered adapter cwd escapes or is missing")
    timeout_ms = adapter.get("timeout_ms", oracle.get("timeout_ms"))
    if not isinstance(timeout_ms, int) or not 1 <= timeout_ms <= 300_000:
        raise ValueError("registered adapter timeout is outside the frozen boundary")
    observation = adapter.get("observation")
    if not isinstance(observation, dict) or observation.get("kind") not in {
        "exit_code",
        "stdout",
        "stderr",
    }:
        raise ValueError("registered adapter observation is not command-evaluable")
    plan = {
        "schema_version": "commandagent.goal_verify.command_plan.v2",
        "oracle_id": oracle["id"],
        "source": "frozen_host_adapter",
        "workspace_root": str(root),
        "cwd": str(cwd),
        "argv": argv,
        "timeout_ms": timeout_ms,
        "observation": observation,
        "raw_provider_argv_used": False,
    }
    plan["plan_sha256"] = _plan_hash(plan)
    return plan


def evaluate_concretized_command(
    plan: dict[str, Any],
    *,
    runner: Callable[[dict[str, Any]], dict[str, Any]],
) -> dict[str, Any]:
    """Evaluate a frozen plan through an injected sandbox runner."""
    if plan.get("source") != "frozen_host_adapter" or plan.get(
        "raw_provider_argv_used"
    ):
        raise ValueError("only a frozen host-adapter plan may execute")
    if plan.get("plan_sha256") != _plan_hash(plan):
        raise ValueError("command plan integrity check failed")
    result = runner(copy.deepcopy(plan))
    if not isinstance(result, dict):
        raise TypeError("sandbox runner result must be an object")
    base = {
        "oracle_id": plan["oracle_id"],
        "plan_sha256": plan["plan_sha256"],
        "runtime_ms": result.get("runtime_ms"),
    }
    if result.get("timed_out"):
        return {
            **base,
            "executed": True,
            "result": "blocked",
            "observed_strength": None,
            "reason": "timeout",
        }
    if result.get("runner_error"):
        return {
            **base,
            "executed": False,
            "result": "oracle_error",
            "observed_strength": None,
            "reason": str(result["runner_error"]),
        }
    observation = plan["observation"]
    kind = observation["kind"]
    actual = result.get(kind)
    passed = actual == observation.get("expected")
    return {
        **base,
        "executed": True,
        "result": "pass" if passed else "fail",
        "observed_strength": "runtime",
        "reason": "observation_match" if passed else "observation_mismatch",
        "actual": actual,
    }


def _load_evidence_rows(path: Path) -> list[dict[str, Any]]:
    text = path.read_text(encoding="utf-8")
    try:
        value = json.loads(text)
    except json.JSONDecodeError:
        value = [json.loads(line) for line in text.splitlines() if line.strip()]
    if isinstance(value, dict):
        rows = value.get("observations", value.get("evidence", [value]))
    else:
        rows = value
    if not isinstance(rows, list) or any(not isinstance(row, dict) for row in rows):
        raise ValueError("evidence artifact does not contain object rows")
    return rows


def evaluate_existing_evidence(
    *,
    claim: dict[str, Any],
    oracle: dict[str, Any],
    execution_root: Path,
) -> dict[str, Any]:
    """Validate an existing-binding oracle against its provenance-bearing artifact."""
    if oracle.get("strategy") not in {
        "existing_fix_evidence",
        "existing_investigation_binding",
    }:
        raise ValueError("oracle does not reference existing evidence")
    origin = claim.get("origin")
    if not isinstance(origin, dict):
        raise TypeError("claim lacks a host-owned evidence origin")
    observation = oracle.get("observation")
    if (
        not isinstance(observation, dict)
        or observation.get("kind") != "existing_binding"
    ):
        raise ValueError(
            "existing evidence oracle lacks an existing_binding observation"
        )
    if observation.get("artifact_path") != origin.get("artifact_path"):
        raise ValueError("oracle artifact does not match the registered claim origin")
    path = resolve_evidence_reference(
        execution_root=execution_root, artifact_path=origin["artifact_path"]
    )
    keys = ["requirement_id", "stage", "lineage", "epoch"]
    if origin.get("source_kind") == "investigation_requirement":
        keys.append("binding_id")
    matches = [
        row
        for row in _load_evidence_rows(path)
        if all(row.get(key) == origin.get(key) for key in keys)
    ]
    if len(matches) != 1:
        return {
            "oracle_id": oracle["id"],
            "executed": False,
            "result": "unverified",
            "observed_strength": None,
            "reason": "registered_evidence_match_count_not_one",
            "match_count": len(matches),
        }
    row = matches[0]
    expected_outcome = origin.get("expected_polarity")
    expected_outcome = {"success": "success", "failure": "failure"}.get(
        expected_outcome
    )
    outcome_matches = expected_outcome is None or row.get("outcome") == expected_outcome
    passed = row.get("executed") is True and outcome_matches
    return {
        "oracle_id": oracle["id"],
        "executed": row.get("executed") is True,
        "result": "pass" if passed else "fail",
        "observed_strength": "runtime" if row.get("executed") is True else None,
        "reason": "provenance_match" if passed else "evidence_outcome_mismatch",
        "artifact_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
    }


def candidate_case_v2(
    source: dict[str, Any], pair_id: str, record: dict[str, Any]
) -> dict[str, Any]:
    """Build a candidate case without projecting any baseline observation authority."""
    case = copy.deepcopy(source)
    case["case_id"] = pair_id
    response = record.get("response", {})
    ollama = (
        response.get("response", {}) if response.get("status") == "completed" else {}
    )
    validation = record.get("validation", {})
    spec = validation.get("spec") if validation.get("valid") else None
    claimed = [claim["id"] for claim in spec.get("claims", [])] if spec else []
    evaluations = record.get("oracle_evaluations", [])
    verified = [
        {
            "claim_id": oracle["claim_id"],
            "strength": evaluation["observed_strength"],
            "executed": True,
        }
        for oracle in (spec or {}).get("oracles", [])
        for evaluation in evaluations
        if evaluation.get("oracle_id") == oracle.get("id")
        and evaluation.get("result") == "pass"
        and evaluation.get("observed_strength") is not None
    ]
    wall_ns = ollama.get("total_duration", response.get("client_wall_time_ns", 0))
    case["observation"] = {
        "claimed_claim_ids": claimed,
        "verified_claims": verified,
        "verdict": "unverified",
        "final_acceptance": False,
        "schema_valid": bool(validation.get("valid")),
        "wall_time_ms": int(wall_ns or 0) // 1_000_000,
        "verify_runtime_ms": sum(
            int(evaluation.get("runtime_ms") or 0) for evaluation in evaluations
        ),
        "input_tokens": int(ollama.get("prompt_eval_count", 0) or 0),
        "output_tokens": int(ollama.get("eval_count", 0) or 0),
        "planner_calls": 0,
        "retries": 0,
        "repairs": 0,
        "flake_trials": [],
        "policy_rejection": False,
        "dependency_blocked": any(
            evaluation.get("result") == "blocked" for evaluation in evaluations
        ),
        "source_reference": record["record_path"],
    }
    return case


def evaluate_v2_oracles(
    *,
    spec: dict[str, Any],
    adapters: dict[str, dict[str, Any]],
    execution_root: Path,
    sandbox_runner: Callable[[dict[str, Any]], dict[str, Any]],
) -> list[dict[str, Any]]:
    """Evaluate each oracle through its preregistered lane without baseline projection."""
    claims = {claim["id"]: claim for claim in spec.get("claims", [])}
    evaluations = []
    for oracle in spec.get("oracles", []):
        classification = classify_oracle_execution(oracle)
        if classification["lane"] == "reference_validation":
            try:
                evaluation = evaluate_existing_evidence(
                    claim=claims[oracle["claim_id"]],
                    oracle=oracle,
                    execution_root=execution_root,
                )
            except (
                KeyError,
                TypeError,
                ValueError,
                OSError,
                json.JSONDecodeError,
            ) as error:
                evaluation = {
                    "oracle_id": oracle.get("id"),
                    "executed": False,
                    "result": "oracle_error",
                    "observed_strength": None,
                    "reason": f"reference_validation_failed:{type(error).__name__}",
                }
        elif classification["lane"] == "executable":
            adapter = adapters.get(oracle.get("id"))
            if adapter is None:
                evaluation = {
                    "oracle_id": oracle.get("id"),
                    "executed": False,
                    "result": "blocked",
                    "observed_strength": None,
                    "reason": "registered_adapter_missing",
                }
            else:
                try:
                    plan = concretize_registered_command(
                        oracle=oracle,
                        adapter=adapter,
                        workspace_root=execution_root,
                    )
                    evaluation = evaluate_concretized_command(
                        plan, runner=sandbox_runner
                    )
                except (KeyError, TypeError, ValueError, OSError) as error:
                    evaluation = {
                        "oracle_id": oracle.get("id"),
                        "executed": False,
                        "result": "oracle_error",
                        "observed_strength": None,
                        "reason": f"concretization_failed:{type(error).__name__}",
                    }
        else:
            evaluation = {
                "oracle_id": oracle.get("id"),
                "executed": False,
                "result": "unverified",
                "observed_strength": None,
                "reason": "executor_unavailable",
            }
        evaluations.append({**evaluation, "lane": classification["lane"]})
    return evaluations
