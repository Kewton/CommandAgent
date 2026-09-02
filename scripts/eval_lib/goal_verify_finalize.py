from __future__ import annotations

import copy
import subprocess
from pathlib import Path
from typing import Any

from eval_lib.goal_verify_blind import validate_blind_evidence
from eval_lib.goal_verify_live import (
    _atomic_json,
    _load_record_ledger,
    load_json,
    sha256_file,
)


def _relative(root: Path, path: Path) -> str:
    return str(path.resolve().relative_to(root.resolve()))


def build_live_matrix(
    *,
    root: Path,
    template: dict[str, Any],
    campaign_dir: Path,
    blind_dir: Path,
    baseline_report: Path,
    candidate_report: Path,
) -> dict[str, Any]:
    matrix = copy.deepcopy(template)
    matrix["matrix_id"] = "phase6-live-ab-20260826-v2"
    matrix["baseline"] = {
        "label": "phase0-current-authority-paired-replay",
        "corpus": _relative(root, campaign_dir / "baseline-corpus.json"),
        "report": _relative(root, baseline_report),
    }
    matrix["candidate"] = {
        "label": "verification-spec-v0-live-shadow",
        "corpus": _relative(root, blind_dir / "candidate-corpus.json"),
        "report": _relative(root, candidate_report),
    }
    matrix["evidence_lanes"] = {
        "blind_review": {
            "required": True,
            "status": "available",
            "references": [
                _relative(root, blind_dir / "blind-review-manifest.json"),
                _relative(root, blind_dir / "blind-review-report.json"),
            ],
        },
        "ci": {
            "required": True,
            "status": "available",
            "references": ["eval/goal_verify/v0/exact-sha-ci-b8474aad.json"],
        },
        "offline_local": {
            "required": True,
            "status": "available",
            "references": [
                _relative(root, campaign_dir / "campaign-manifest.json"),
                _relative(root, campaign_dir / "campaign-summary.json"),
                _relative(root, campaign_dir / "baseline-corpus.json"),
                _relative(root, blind_dir / "candidate-corpus.json"),
            ],
        },
        "approved_live": {
            "required": True,
            "status": "available",
            "authorized": True,
            "references": [
                "eval/goal_verify/v0/phase6-paired-contract.json",
                _relative(root, campaign_dir / "campaign-manifest.json"),
                _relative(root, campaign_dir / "campaign-summary.json"),
            ],
        },
    }
    return matrix


def _run(command: list[str], *, root: Path) -> None:
    subprocess.run(command, cwd=root, check=True)


def finalize(
    *,
    root: Path,
    campaign_dir: Path,
    blind_dir: Path,
    template_path: Path,
    config_path: Path,
    attempt_id: str,
) -> dict[str, Any]:
    summary = load_json(campaign_dir / "campaign-summary.json")
    if not summary.get("complete") or summary.get("completed_pairs") != 360:
        raise ValueError("campaign is not complete at 360 pairs")
    contract = load_json(root / "eval/goal_verify/v0/phase6-paired-contract.json")
    ledger_entries, ledger_head = _load_record_ledger(
        root=root,
        run_dir=campaign_dir,
        ledger_path=campaign_dir / contract["integrity"]["record_ledger"],
    )
    if len(ledger_entries) != 360 or summary.get("record_ledger_entries") != 360:
        raise ValueError("campaign ledger is not complete at 360 records")
    if summary.get("record_ledger_head_sha256") != ledger_head:
        raise ValueError("campaign summary is not bound to the record ledger head")
    blind = validate_blind_evidence(
        root=root,
        baseline_path=campaign_dir / "baseline-corpus.json",
        candidate_draft_path=campaign_dir / "candidate-corpus.draft.json",
        contract_path=root / "eval/goal_verify/v0/phase6-blind-review-contract.json",
        run_dir=blind_dir,
    )
    if blind.get("reviewed_pairs") != 360:
        raise ValueError("blind review is not complete at 360 pairs")
    candidate_path = blind_dir / "candidate-corpus.json"

    if not attempt_id or "/" in attempt_id or ".." in attempt_id:
        raise ValueError("finalization attempt ID must be a simple non-empty name")
    finalization_dir = campaign_dir / "finalizations" / attempt_id
    if finalization_dir.exists() and any(finalization_dir.iterdir()):
        raise FileExistsError(f"finalization attempt must be new or empty: {finalization_dir}")
    baseline_eval = finalization_dir / "baseline-eval"
    candidate_eval = finalization_dir / "candidate-eval"
    _run(
        [
            "python3",
            "scripts/eval-goal-verify-baseline.py",
            "--corpus",
            _relative(root, campaign_dir / "baseline-corpus.json"),
            "--config",
            _relative(root, config_path),
            "--run-dir",
            _relative(root, baseline_eval),
        ],
        root=root,
    )
    _run(
        [
            "python3",
            "scripts/eval-goal-verify-baseline.py",
            "--corpus",
            _relative(root, blind_dir / "candidate-corpus.json"),
            "--config",
            _relative(root, config_path),
            "--run-dir",
            _relative(root, candidate_eval),
        ],
        root=root,
    )
    matrix = build_live_matrix(
        root=root,
        template=load_json(template_path),
        campaign_dir=campaign_dir,
        blind_dir=blind_dir,
        baseline_report=baseline_eval / "baseline.json",
        candidate_report=candidate_eval / "baseline.json",
    )
    matrix_path = finalization_dir / "phase6-live-matrix.json"
    _atomic_json(matrix_path, matrix)
    decision_a = finalization_dir / "decision-run-a"
    decision_b = finalization_dir / "decision-run-b"
    command = [
        "python3",
        "scripts/eval-goal-verify-phase6.py",
        "--manifest",
        _relative(root, matrix_path),
        "--config",
        _relative(root, config_path),
    ]
    _run([*command, "--run-dir", _relative(root, decision_a)], root=root)
    _run([*command, "--run-dir", _relative(root, decision_b)], root=root)
    compared = ["phase6-report.json", "failure-cases.json"]
    for name in compared:
        if (decision_a / name).read_bytes() != (decision_b / name).read_bytes():
            raise ValueError(f"same-script replay differs: {name}")
    report = load_json(decision_a / "phase6-report.json")
    final = {
        "schema_version": "commandagent.goal_verify.phase6_finalization.v0",
        "attempt_id": attempt_id,
        "same_script_replay_byte_identical": True,
        "compared_files": compared,
        "bootstrap_samples": load_json(config_path)["bootstrap_samples"],
        "final_decision": report["final_decision"],
        "failure_count": len(report["failure_cases"]),
        "input_sha256": {
            "campaign_summary": sha256_file(campaign_dir / "campaign-summary.json"),
            "blind_review_report": sha256_file(blind_dir / "blind-review-report.json"),
            "candidate_corpus": sha256_file(candidate_path),
            "matrix": sha256_file(matrix_path),
        },
        "decision_sha256": {
            name: sha256_file(decision_a / name) for name in compared
        },
    }
    _atomic_json(finalization_dir / "finalization-summary.json", final)
    return final
