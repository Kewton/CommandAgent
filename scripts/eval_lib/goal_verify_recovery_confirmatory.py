"""Forward-only confirmatory runner for the registered Recovery task corpus."""
from __future__ import annotations

import hashlib
import json
import shutil
from pathlib import Path
from typing import Any
from unittest.mock import patch

from eval_lib import goal_verify_recovery_deterministic_pair as instrument
from eval_lib.goal_verify_recovery_confirmatory_design import (
    analyze_pair_results,
    design_errors,
    materialize_pair_ids,
)

ROOT = Path(__file__).resolve().parents[2]
SCHEMA_VERSION = "commandagent.goal_verify.recovery_confirmatory.v1"
REPORT_SCHEMA_VERSION = "commandagent.goal_verify.recovery_confirmatory_report.v1"
SOURCE_PATHS = (
    "scripts/eval-goal-verify-recovery-confirmatory.py",
    "scripts/eval_lib/goal_verify_recovery_confirmatory.py",
    "scripts/eval_lib/goal_verify_recovery_confirmatory_design.py",
    "scripts/eval_lib/goal_verify_recovery_deterministic_pair.py",
    "scripts/eval_lib/goal_verify_recovery_deterministic_smoke.py",
)
TASK_TARGETS = {
    "generic": Path("fixture/task-02.json"),
    "data": Path("data/task-02.csv"),
    "nextjs": Path("fixture/task-02.json"),
}
TASK_SOURCES = {
    "generic": "fixture/task-{task_id}.json",
    "data": "data/task-{task_id}.csv",
    "nextjs": "fixture/task-{task_id}.json",
}
SCENARIO_IDS = {
    "generic": "generic-fix",
    "data": "data-fix",
    "nextjs": "nextjs-fix",
}
TASK_ROOTS = {
    "generic": ROOT / "tests/fixtures/goal_verify_v4/main/fix-generic-fixtures/before",
    "data": ROOT / "tests/fixtures/goal_verify_v4/a15/fix-data-reconciliation/before",
    "nextjs": ROOT / "tests/fixtures/goal_verify_v4/a15/fix-nextjs-route-label/before",
}


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while chunk := handle.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def _canonical_sha256(value: Any) -> str:
    payload = json.dumps(
        value, ensure_ascii=False, separators=(",", ":"), sort_keys=True
    ).encode()
    return hashlib.sha256(payload).hexdigest()


def task_corpus_sha256(task_ids: list[str]) -> str:
    manifest = {}
    for profile in ("generic", "data", "nextjs"):
        for task_id in task_ids:
            relative = TASK_SOURCES[profile].format(task_id=task_id)
            path = TASK_ROOTS[profile] / relative
            if not path.is_file():
                raise ValueError(f"registered task source is missing:{path}")
            manifest[f"{profile}/{relative}"] = sha256_file(path)
    return _canonical_sha256(manifest)


def contract_errors(contract: dict[str, Any]) -> list[str]:
    errors = design_errors(contract.get("design", {}))
    if contract.get("schema_version") != SCHEMA_VERSION:
        errors.append("schema_version_invalid")
    if contract.get("status") != "frozen":
        errors.append("contract_not_frozen")
    authorization = contract.get("authorization", {})
    if authorization.get("confirmatory_collection_authorized") is not True:
        errors.append("confirmatory_collection_not_authorized")
    for field in ("generalization_claim_allowed", "default_rollout_allowed"):
        if contract.get(field) is not False:
            errors.append(f"{field}_must_be_false")
    if contract.get("conditional_effect_claim_allowed") is not True:
        errors.append("conditional_effect_claim_not_authorized")
    evidence_relative = contract.get("exact_sha_ci_evidence")
    evidence_relative_path = Path(str(evidence_relative))
    evidence_path = ROOT / evidence_relative_path
    unsafe_evidence_path = (
        not isinstance(evidence_relative, str)
        or evidence_relative_path.is_absolute()
        or ".." in evidence_relative_path.parts
    )
    try:
        if unsafe_evidence_path:
            raise OSError("unsafe evidence path")
        evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        errors.append("exact_sha_ci_evidence_invalid")
    else:
        workflows = evidence.get("workflows", [])
        required = {"CI", "acceptance"}
        successful = {
            row.get("name")
            for row in workflows
            if row.get("head_sha") == contract.get("code_sha")
            and row.get("status") == "completed"
            and row.get("conclusion") == "success"
        }
        if (
            evidence.get("head_sha") != contract.get("code_sha")
            or not required.issubset(successful)
        ):
            errors.append("exact_sha_ci_evidence_invalid")
    task_ids = contract.get("design", {}).get("task_ids", [])
    try:
        actual_task_corpus = task_corpus_sha256(task_ids)
    except ValueError:
        actual_task_corpus = None
    if contract.get("task_corpus_sha256") != actual_task_corpus:
        errors.append("task_corpus_hash_invalid")
    source_hashes = contract.get("authoritative_source_sha256", {})
    for relative in SOURCE_PATHS:
        path = ROOT / relative
        if not path.is_file() or source_hashes.get(relative) != sha256_file(path):
            errors.append(f"authoritative_source_hash_invalid:{relative}")
    return errors


def _task_prepare(task_id: str):
    original_prepare = instrument._prepare

    def prepare(
        scenario: instrument.Scenario, workspace: Path, node_modules_source: Path
    ):
        result = original_prepare(scenario, workspace, node_modules_source)
        source = workspace / TASK_SOURCES[scenario.profile].format(task_id=task_id)
        target = workspace / TASK_TARGETS[scenario.profile]
        if not source.is_file():
            raise ValueError(f"registered task source is missing:{source}")
        if source != target:
            shutil.copyfile(source, target)
        return result

    return prepare


def run_task_arm(
    *,
    commandagent_bin: Path,
    node_modules_source: Path,
    profile: str,
    task_id: str,
    arm: str,
    output_dir: Path,
    execution_root: Path | None,
    timeout_sec: int,
) -> dict[str, Any]:
    scenario = instrument.SCENARIOS[SCENARIO_IDS[profile]]
    with patch.object(instrument, "_prepare", _task_prepare(task_id)):
        report = instrument._run_arm(
            commandagent_bin=commandagent_bin,
            scenario=scenario,
            arm=arm,
            output_dir=output_dir,
            execution_root=execution_root,
            node_modules_source=node_modules_source,
            timeout_sec=timeout_sec,
        )
    report["registered_task_id"] = task_id
    (output_dir / "arm-report.json").write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    return report


def _stale_failure_kind_count(events_path: Path, treatment: dict[str, Any]) -> int:
    if not treatment.get("registered_endpoint_success"):
        return 0
    rows = [json.loads(line) for line in events_path.read_text(encoding="utf-8").splitlines()]
    terminal = [row for row in rows if row.get("event") == "tui_command_stop"]
    return int(
        len(terminal) != 1
        or terminal[0].get("ok") is not True
        or terminal[0].get("failure_kind") not in (None, "")
        or terminal[0].get("stop_reason") != "completed"
    )


def build_pair_row(
    pair_id: str,
    control: dict[str, Any],
    treatment: dict[str, Any],
    treatment_events: Path,
) -> dict[str, Any]:
    profile = pair_id.split("--", 1)[0]
    control_checks = control.get("checks", {})
    treatment_checks = treatment.get("checks", {})
    return {
        "pair_id": pair_id,
        "profile": profile,
        "control_endpoint": bool(control.get("registered_endpoint_success")),
        "treatment_endpoint": bool(treatment.get("registered_endpoint_success")),
        "pair_valid": bool(
            control.get("arm_valid")
            and treatment.get("arm_valid")
            and control.get("input_snapshot_sha256")
            == treatment.get("input_snapshot_sha256")
            and control.get("boundary_signature_sha256")
            == treatment.get("boundary_signature_sha256")
        ),
        "regression_count": int(
            not control_checks.get("registered_regressions_passed", False)
        )
        + int(not treatment_checks.get("all_registered_commands_passed", False)),
        "artifact_harm_count": int(
            not control_checks.get("protected_paths_unchanged", False)
        )
        + int(not treatment_checks.get("protected_paths_unchanged", False)),
        "discarded_valid_treatment_count": int(
            treatment.get("registered_endpoint_success") is True
            and treatment_checks.get("treatment_promoted") is not True
        ),
        "stale_failure_kind_count": _stale_failure_kind_count(
            treatment_events, treatment
        ),
        "control_arm_valid": bool(control.get("arm_valid")),
        "treatment_arm_valid": bool(treatment.get("arm_valid")),
        "same_input_snapshot": control.get("input_snapshot_sha256")
        == treatment.get("input_snapshot_sha256"),
        "same_failure_boundary": control.get("boundary_signature_sha256")
        == treatment.get("boundary_signature_sha256"),
    }


def _append_ledger(path: Path, entry: dict[str, Any], previous: str) -> str:
    payload = {**entry, "previous_entry_sha256": previous}
    entry_sha = _canonical_sha256(payload)
    with path.open("a", encoding="utf-8") as handle:
        handle.write(
            json.dumps({**payload, "entry_sha256": entry_sha}, sort_keys=True) + "\n"
        )
    return entry_sha


def run_confirmatory(
    *,
    contract: dict[str, Any],
    contract_path: Path,
    commandagent_bin: Path,
    node_modules_source: Path,
    run_dir: Path,
    execution_root: Path | None = None,
    timeout_sec: int = 180,
) -> dict[str, Any]:
    errors = contract_errors(contract)
    if errors:
        raise ValueError("invalid confirmatory contract:" + ",".join(errors))
    commandagent_bin = commandagent_bin.resolve()
    node_modules_source = node_modules_source.resolve()
    if sha256_file(commandagent_bin) != contract.get("binary_sha256"):
        raise ValueError("commandagent binary SHA-256 does not match contract")
    if instrument.provisioning_manifest_sha256(
        node_modules_source
    ) != contract.get("nextjs_node_modules_manifest_sha256"):
        raise ValueError("Next.js provisioning manifest does not match contract")
    run_dir = run_dir.resolve()
    if run_dir.name != contract.get("run_id"):
        raise ValueError("run directory name does not match contract")
    run_dir.mkdir(parents=True, exist_ok=False)
    (run_dir / "contract-copy.json").write_text(
        contract_path.read_text(encoding="utf-8"), encoding="utf-8"
    )
    ledger_path = run_dir / "record-ledger.jsonl"
    previous = "0" * 64
    pair_rows = []
    design = contract["design"]
    for pair_id in materialize_pair_ids(
        design["allocation_seed"], design["pairs_per_profile"]
    ):
        profile, pair_suffix = pair_id.split("--pair-", 1)
        task_id = pair_suffix
        reports = {}
        for arm in ("control", "treatment"):
            output_dir = run_dir / "arms" / pair_id / arm
            reports[arm] = run_task_arm(
                commandagent_bin=commandagent_bin,
                node_modules_source=node_modules_source,
                profile=profile,
                task_id=task_id,
                arm=arm,
                output_dir=output_dir,
                execution_root=execution_root,
                timeout_sec=timeout_sec,
            )
            previous = _append_ledger(
                ledger_path,
                {
                    "pair_id": pair_id,
                    "arm": arm,
                    "arm_report_sha256": sha256_file(output_dir / "arm-report.json"),
                },
                previous,
            )
        pair_rows.append(
            build_pair_row(
                pair_id,
                reports["control"],
                reports["treatment"],
                run_dir / "arms" / pair_id / "treatment/events.jsonl",
            )
        )
    analysis = analyze_pair_results(design, pair_rows)
    report = {
        "schema_version": REPORT_SCHEMA_VERSION,
        "contract_id": contract.get("contract_id"),
        "run_id": contract.get("run_id"),
        "code_sha": contract.get("code_sha"),
        "binary_sha256": contract.get("binary_sha256"),
        "contract_sha256": sha256_file(contract_path),
        "ledger_tail_sha256": previous,
        "pair_rows": pair_rows,
        **analysis,
    }
    report["evidence_sha256"] = {
        path.relative_to(run_dir).as_posix(): sha256_file(path)
        for path in sorted(run_dir.rglob("*"))
        if path.is_file() and path.name != "report.json"
    }
    (run_dir / "report.json").write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    return report
