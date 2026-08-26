from __future__ import annotations

import hashlib
import json
import shutil
import sys
from collections.abc import Callable
from pathlib import Path
from typing import Any

from eval_lib.goal_verify_sandbox import run_macos_sandbox
from eval_lib.goal_verify_v2 import _plan_hash

CommandRunner = Callable[[dict[str, Any]], dict[str, Any]]

_IGNORED_PARTS = {
    ".anvil",
    ".commandagent-state",
    ".commandagent-eval-home",
    ".commandagent-eval-tmp",
    ".git",
    "node_modules",
    "target",
}
_SHELL_PROGRAMS = {"bash", "dash", "env", "fish", "sh", "xargs", "zsh"}
_COMMANDISH = {"command", "exit_code", "stdout", "stderr"}
_SENSITIVE_NAMES = {".env", ".npmrc", ".pypirc", "credentials", "credentials.json"}


def workspace_manifest(workspace: Path) -> dict[str, Any]:
    root = workspace.resolve()
    entries: list[dict[str, Any]] = []
    for path in sorted(root.rglob("*")):
        relative = path.relative_to(root)
        if any(part in _IGNORED_PARTS for part in relative.parts):
            continue
        if any(part in _SENSITIVE_NAMES or part.startswith(".env.") for part in relative.parts):
            continue
        if path.is_symlink():
            target = path.resolve()
            if not target.is_relative_to(root):
                continue
            entries.append(
                {"path": relative.as_posix(), "kind": "symlink", "target": str(path.readlink())}
            )
        elif path.is_file():
            entries.append(
                {
                    "path": relative.as_posix(),
                    "kind": "file",
                    "sha256": _sha256_file(path),
                    "size": path.stat().st_size,
                }
            )
    digest = hashlib.sha256(_canonical_json(entries).encode()).hexdigest()
    return {
        "schema_version": "commandagent.goal_verify.workspace_manifest.v4",
        "entries": entries,
        "snapshot_sha256": digest,
    }


def candidate_visible_manifest(manifest: dict[str, Any]) -> dict[str, Any]:
    return {
        "schema_version": manifest["schema_version"],
        "snapshot_sha256": manifest["snapshot_sha256"],
        "entries": [
            {
                key: value
                for key, value in row.items()
                if key in {"path", "kind", "sha256", "size"}
            }
            for row in manifest["entries"]
        ],
    }


def concretize_candidate_oracle(
    *, oracle: dict[str, Any], claim: dict[str, Any], manifest: dict[str, Any]
) -> dict[str, Any]:
    strategy = oracle.get("strategy")
    observation = oracle.get("observation")
    if not isinstance(observation, dict):
        return _rejected("concretization_failure", "observation_missing")
    paths = {
        row["path"]: row for row in manifest.get("entries", []) if row.get("kind") == "file"
    }
    stage = _claim_stage(claim)
    if strategy in _COMMANDISH:
        setup = oracle.get("setup")
        argv = setup.get("argv") if isinstance(setup, dict) else None
        reason = _argv_error(argv)
        if reason:
            return _rejected("policy_rejected", reason)
        cwd = setup.get("cwd", ".")
        if not _safe_relative(cwd):
            return _rejected("policy_rejected", "cwd_unsafe")
        fixture_error = _fixture_error(oracle.get("input"), paths)
        if fixture_error:
            return _rejected("concretization_failure", fixture_error)
        return {
            "classification": "executable",
            "reason": None,
            "stage": stage,
            "plan": {
                "kind": "command",
                "argv": list(argv),
                "cwd": cwd,
                "timeout_ms": int(oracle.get("timeout_ms", 30_000)),
                "observation": observation,
            },
            "gold_used_for_concretization": False,
        }
    if strategy in {"file", "fixture"}:
        path = observation.get("path")
        if not _safe_relative(path):
            return _rejected("policy_rejected", "observation_path_unsafe")
        return {
            "classification": "executable",
            "reason": None,
            "stage": stage,
            "plan": {
                "kind": "file_probe",
                "path": path,
                "exists": observation.get("exists"),
            },
            "gold_used_for_concretization": False,
        }
    return _rejected("executor_unavailable", f"strategy_not_implemented:{strategy}")


def execute_candidate_plan(
    plan: dict[str, Any], *, workspace: Path, runner: CommandRunner = run_macos_sandbox
) -> dict[str, Any]:
    root = workspace.resolve()
    if plan["kind"] == "file_probe":
        path = (root / plan["path"]).resolve()
        if not path.is_relative_to(root):
            return {"executed": False, "result": "oracle_error", "reason": "path_escape"}
        actual = path.exists()
        passed = actual is plan["exists"]
        return {
            "executed": True,
            "result": "pass" if passed else "fail",
            "reason": "observation_match" if passed else "observation_mismatch",
            "actual": actual,
            "observed_strength": "deterministic" if passed else None,
        }
    cwd = (root / plan["cwd"]).resolve()
    if not cwd.is_relative_to(root) or not cwd.is_dir():
        return {"executed": False, "result": "oracle_error", "reason": "cwd_missing"}
    resolved = _resolve_executable(plan["argv"][0])
    if resolved is None:
        return {
            "executed": False,
            "result": "oracle_error",
            "reason": "executable_unavailable",
        }
    argv = [resolved, *plan["argv"][1:]]
    command_plan = {
        "schema_version": "commandagent.goal_verify.command_plan.v4",
        "oracle_id": "candidate-v4",
        "source": "host_validated_candidate_v4",
        "workspace_root": str(root),
        "cwd": str(cwd),
        "argv": argv,
        "timeout_ms": plan["timeout_ms"],
        "observation": plan["observation"],
        "raw_provider_argv_used": False,
        "read_scope": "workspace_and_runtime",
    }
    command_plan["plan_sha256"] = _plan_hash(command_plan)
    outcome = runner(command_plan)
    if outcome.get("runner_error"):
        return {
            **outcome,
            "executed": False,
            "result": "oracle_error",
            "reason": str(outcome["runner_error"]),
        }
    if outcome.get("timed_out"):
        return {**outcome, "executed": False, "result": "blocked", "reason": "timeout"}
    observation = plan["observation"]
    actual = _observed_value(observation, outcome, root)
    expected = observation.get("expected", observation.get("exists"))
    passed = actual == expected
    return {
        **outcome,
        "executed": True,
        "result": "pass" if passed else "fail",
        "reason": "observation_match" if passed else "observation_mismatch",
        "actual": actual,
        "observed_strength": "runtime" if passed else None,
    }


def evaluate_candidate_spec_v4(
    *,
    spec: dict[str, Any],
    workspaces: dict[str, Path],
    frozen_snapshot_sha256: dict[str, str],
    runner: CommandRunner = run_macos_sandbox,
) -> dict[str, Any]:
    claims = {row.get("id"): row for row in spec.get("claims", [])}
    evaluations = []
    for oracle in spec.get("oracles", []):
        claim = claims.get(oracle.get("claim_id"))
        if claim is None:
            evaluations.append(
                {
                    "oracle_id": oracle.get("id"),
                    "claim_id": oracle.get("claim_id"),
                    "classification": "concretization_failure",
                    "executed": False,
                    "result": "unverified",
                    "reason": "claim_missing",
                    "gold_used_for_execution": False,
                }
            )
            continue
        stage = _claim_stage(claim)
        workspace = workspaces.get(stage)
        if workspace is None:
            evaluations.append(
                {
                    "oracle_id": oracle.get("id"),
                    "claim_id": oracle.get("claim_id"),
                    "classification": "concretization_failure",
                    "executed": False,
                    "result": "unverified",
                    "reason": f"workspace_missing:{stage}",
                    "gold_used_for_execution": False,
                }
            )
            continue
        manifest = workspace_manifest(workspace)
        if manifest["snapshot_sha256"] != frozen_snapshot_sha256.get(stage):
            evaluations.append(
                {
                    "oracle_id": oracle.get("id"),
                    "claim_id": oracle.get("claim_id"),
                    "classification": "concretization_failure",
                    "executed": False,
                    "result": "unverified",
                    "reason": f"snapshot_hash_mismatch:{stage}",
                    "gold_used_for_execution": False,
                }
            )
            continue
        concrete = concretize_candidate_oracle(
            oracle=oracle, claim=claim, manifest=manifest
        )
        base = {
            "oracle_id": oracle.get("id"),
            "claim_id": oracle.get("claim_id"),
            "classification": concrete["classification"],
            "stage": concrete["stage"],
            "execution_snapshot_sha256": manifest["snapshot_sha256"],
            "gold_used_for_execution": False,
        }
        if concrete["classification"] != "executable":
            evaluations.append(
                {
                    **base,
                    "executed": False,
                    "result": "unverified",
                    "reason": concrete["reason"],
                }
            )
            continue
        evaluations.append(
            {
                **base,
                "execution_plan": concrete["plan"],
                **execute_candidate_plan(
                    concrete["plan"], workspace=workspace, runner=runner
                ),
            }
        )
    return {
        "evaluations": evaluations,
        "same_snapshot": all(
            row.get("execution_snapshot_sha256")
            == frozen_snapshot_sha256.get(row.get("stage"))
            for row in evaluations
            if row.get("classification") == "executable"
        ),
        "reference_fallback_count": sum(
            row.get("stage") in {"reference", "after"} for row in evaluations
        ),
        "gold_used_for_execution_count": sum(
            row.get("gold_used_for_execution") is True for row in evaluations
        ),
    }


def combine_evaluations(
    *,
    case: dict[str, Any],
    adapters: list[dict[str, Any]],
    baseline_evaluations: list[dict[str, Any]],
    candidate_evaluations: list[dict[str, Any]],
    baseline_status: str,
) -> dict[str, Any]:
    baseline = _score(case, adapters, baseline_evaluations)
    combined_rows = _union_by_adapter(baseline_evaluations, candidate_evaluations)
    combined = _score(case, adapters, combined_rows)
    candidate_failures = [
        row
        for row in candidate_evaluations
        if row.get("executed") is True and row.get("result") == "fail"
    ]
    candidate_unknown = [
        row
        for row in candidate_evaluations
        if row.get("classification") != "executable" or row.get("executed") is not True
    ]
    if baseline_status != "completed" or candidate_failures:
        shadow = "failure"
    elif candidate_unknown or any(row["status"] != "strong" for row in combined["claims"]):
        shadow = "unverified"
    else:
        shadow = "pass"
    return {
        "baseline_score": baseline,
        "combined_score": combined,
        "paired_delta": {
            "required_claim_recall": combined["required_claim_observation_recall"]
            - baseline["required_claim_observation_recall"],
            "strong_binding": combined["strong_binding_by_observation"]
            - baseline["strong_binding_by_observation"],
            "unverified_rate": _unverified_rate(combined) - _unverified_rate(baseline),
            "recovered_claim_count": sum(
                before["status"] == "unverified" and after["status"] == "strong"
                for before, after in zip(baseline["claims"], combined["claims"], strict=True)
            ),
        },
        "shadow_verdict": shadow,
        "baseline_failure_overridden": False,
        "candidate_failure_count": len(candidate_failures),
        "candidate_invalid_or_unexecutable_count": len(candidate_unknown),
    }


def score_candidate_outcomes(
    *,
    case_id: str,
    lane: str,
    oracles: list[dict[str, Any]],
    outcomes: list[dict[str, Any]],
    adapters: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    rows = []
    for oracle, outcome in zip(oracles, outcomes, strict=True):
        matches = [
            adapter
            for adapter in adapters
            if adapter["case_id"] == case_id
            and (lane != "contract_conformance" or adapter["claim_id"] == oracle.get("claim_id"))
            and oracle.get("strategy") in adapter["proposal"]["strategies"]
            and oracle.get("expected_polarity") in adapter["proposal"]["polarities"]
            and oracle.get("observation", {}).get("kind")
            in adapter["proposal"]["observation_kinds"]
        ]
        matching_outcome = [
            adapter for adapter in matches if _outcome_matches_adapter(outcome, adapter)
        ]
        adapter = matching_outcome[0] if len(matching_outcome) == 1 else None
        observation_match = bool(adapter) and _outcome_matches_adapter(outcome, adapter)
        rows.append(
            {
                **outcome,
                "oracle_id": oracle.get("id"),
                "claim_id": oracle.get("claim_id"),
                "adapter_id": adapter.get("adapter_id") if adapter else None,
                "observation_match": observation_match,
                "observed_strength": outcome.get("observed_strength")
                if observation_match
                else None,
                "gold_used_for_execution": False,
                "gold_used_for_scoring": True,
            }
        )
    return rows


def _outcome_matches_adapter(outcome: dict[str, Any], adapter: dict[str, Any]) -> bool:
    if outcome.get("executed") is not True or outcome.get("result") != "pass":
        return False
    proposal = adapter["proposal"]
    actual = outcome.get("actual")
    if "expected_values" in proposal:
        return str(actual) in {str(value) for value in proposal["expected_values"]}
    if "expected_contains" in proposal:
        text = json.dumps(actual, ensure_ascii=False, sort_keys=True)
        return all(fragment in text for fragment in proposal["expected_contains"])
    return True


def _score(case, adapters, evaluations):
    from eval_lib.goal_verify_observation_match_v3 import score_claim_coverage

    return score_claim_coverage(case=case, adapters=adapters, evaluations=evaluations)


def _union_by_adapter(baseline, candidate):
    rows: dict[str, dict[str, Any]] = {}
    for row in [*baseline, *candidate]:
        adapter_id = row.get("adapter_id")
        if not adapter_id:
            continue
        current = rows.get(adapter_id)
        if current is None or (not current.get("observation_match") and row.get("observation_match")):
            rows[adapter_id] = row
    return [rows[key] for key in sorted(rows)]


def _unverified_rate(score):
    claims = score["claims"]
    return sum(row["status"] == "unverified" for row in claims) / len(claims) if claims else 1.0


def _observed_value(observation, outcome, workspace):
    kind = observation["kind"]
    if kind == "file":
        path = (workspace / observation["path"]).resolve()
        return path.is_relative_to(workspace) and path.exists()
    return outcome.get(kind)


def _claim_stage(claim):
    origin = claim.get("origin", {})
    return "before" if origin.get("stage") == "before" else "product"


def _argv_error(argv):
    if not isinstance(argv, list) or not argv:
        return "argv_missing"
    if any(not isinstance(value, str) or not value or "\x00" in value for value in argv):
        return "argv_invalid"
    executable = Path(argv[0])
    if executable.is_absolute() or executable.name in _SHELL_PROGRAMS:
        return "argv_program_unsafe"
    if executable.name.startswith(("python", "pypy")) and "-c" in argv[1:]:
        return "argv_inline_code_unsafe"
    if executable.name in {"node", "perl", "ruby"} and "-e" in argv[1:]:
        return "argv_inline_code_unsafe"
    if any(any(operator in value for operator in ("&&", "||", "$(", "`", ";")) for value in argv):
        return "argv_shell_syntax_unsafe"
    if any(Path(value).is_absolute() for value in argv[1:]):
        return "argv_absolute_argument_unsafe"
    if any(".." in Path(value).parts for value in argv[1:]):
        return "argv_parent_traversal_unsafe"
    return None


def _resolve_executable(argv0):
    name = Path(argv0).name
    if name.startswith(("python", "pypy")):
        return str(Path(sys.executable).resolve())
    resolved = shutil.which(argv0)
    return str(Path(resolved).resolve()) if resolved else None


def _fixture_error(input_value, paths):
    if not isinstance(input_value, dict) or input_value.get("kind") != "fixture":
        return None
    path = input_value.get("path")
    if not _safe_relative(path):
        return "fixture_path_unsafe"
    entry = paths.get(path)
    if entry is None:
        return "fixture_missing"
    expected = input_value.get("sha256")
    if expected != entry.get("sha256"):
        return "fixture_hash_mismatch"
    return None


def _safe_relative(value):
    if not isinstance(value, str) or not value:
        return False
    path = Path(value)
    return not path.is_absolute() and ".." not in path.parts


def _rejected(classification, reason):
    return {
        "classification": classification,
        "reason": reason,
        "stage": None,
        "plan": None,
        "gold_used_for_concretization": False,
    }


def _canonical_json(value):
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True)


def _sha256_file(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()
