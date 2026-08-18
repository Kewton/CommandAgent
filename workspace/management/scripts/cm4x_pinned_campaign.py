"""Run a BoN extension with an already-sealed historical product binary."""
from __future__ import annotations

import argparse
import shutil
import subprocess
import time
from pathlib import Path
from typing import Any

import bench


def verify_file_sha(path: Path, expected: str, label: str) -> str:
    observed = bench.sha256_file(path)
    if observed != expected:
        raise bench.BenchError(
            f"pinned campaign {label} SHA-256 mismatch: "
            f"expected {expected}, observed {observed}"
        )
    return observed


def git_output(repo: Path, *args: str) -> str:
    result = subprocess.run(
        ["git", *args],
        cwd=repo,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
    )
    if result.returncode != 0:
        raise bench.BenchError(
            f"pinned campaign git {' '.join(args)} failed: {result.stderr.strip()}"
        )
    return result.stdout.strip()


def frozen_preflight(
    execution_repo: Path,
    suite: bench.SuiteDefinition,
    predeclaration: bench.BonSeriesPredeclaration,
    source_binary: Path,
    campaign_dir: Path,
    ollama_host: str,
) -> tuple[dict[str, Any], Path]:
    started = int(time.time())
    head = git_output(execution_repo, "rev-parse", "HEAD")
    if head != predeclaration.execution_revision:
        raise bench.BenchError(
            "pinned campaign execution revision mismatch: "
            f"expected {predeclaration.execution_revision}, observed {head}"
        )
    status = git_output(
        execution_repo, "status", "--porcelain", "--untracked-files=all"
    )
    if status:
        raise bench.BenchError("pinned campaign execution repository is dirty")
    source_sha = verify_file_sha(
        source_binary, predeclaration.binary_sha256, "source binary"
    )
    if bench.sha256_file(suite.path) != predeclaration.suite_sha256:
        raise bench.BenchError("pinned campaign suite SHA-256 changed")

    version = subprocess.run(
        [str(source_binary), "--version"],
        cwd=execution_repo,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
    )
    version_text = (version.stdout + version.stderr).strip()
    if version.returncode != 0 or "+dirty" in version_text:
        raise bench.BenchError(
            f"pinned campaign binary identity rejected: {version_text}"
        )

    provider_gate = bench.provider_reachability_preflight(
        suite, source_binary, execution_repo, ollama_host
    )
    binary_dir = campaign_dir / "bin"
    binary_dir.mkdir(parents=True, exist_ok=True)
    installed_binary = binary_dir / "commandagent"
    if not installed_binary.exists():
        shutil.copy2(source_binary, installed_binary)
        installed_binary.chmod(0o755)
    installed_sha = verify_file_sha(
        installed_binary, predeclaration.binary_sha256, "installed binary"
    )
    return (
        {
            "started_epoch": started,
            "completed_epoch": int(time.time()),
            "head_sha": head,
            "head_log": git_output(execution_repo, "log", "-1", "--oneline"),
            "git_status": status,
            "bon_series_pin": {
                "series_id": predeclaration.series_id,
                "execution_revision_expected": predeclaration.execution_revision,
                "execution_revision_observed": head,
                "suite_sha256_expected": predeclaration.suite_sha256,
                "suite_sha256_observed": bench.sha256_file(suite.path),
                "binary_sha256_expected": predeclaration.binary_sha256,
                "binary_sha256_observed": source_sha,
            },
            "binary_sha256": {"source": source_sha, "installed": installed_sha},
            "binary_source": str(source_binary),
            "binary_install_dir": str(binary_dir),
            "path_commandagent": str(installed_binary),
            "version_text": version_text,
            "provider_reachability": provider_gate,
            "cargo_test": {
                "skipped": True,
                "reason": "reuse exact sealed historical binary; final current full suite is separate",
            },
        },
        binary_dir,
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--execution-repo", type=Path, required=True)
    parser.add_argument("--suite", type=Path, required=True)
    parser.add_argument("--predeclaration", type=Path, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--workspace-root", type=Path, required=True)
    parser.add_argument("--ollama-host", default="http://127.0.0.1:11434")
    parser.add_argument("--resume", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    execution_repo = args.execution_repo.resolve()
    suite = bench.load_suite(args.suite.resolve())
    predeclaration = bench.load_bon_predeclaration(
        args.predeclaration.resolve(), suite
    )
    workspace_root = args.workspace_root.resolve()
    if args.resume:
        campaign_dir = bench.find_resume_campaign(workspace_root, suite)
    else:
        campaign_dir = bench.create_campaign(workspace_root, suite.suite_id, False)
    preflight, binary_dir = frozen_preflight(
        execution_repo,
        suite,
        predeclaration,
        args.binary.resolve(),
        campaign_dir,
        args.ollama_host,
    )
    deviations = [
        {
            "code": "historical_binary_reused",
            "detail": "exact sealed binary reused to keep the E extension in the same instrument generation",
        }
    ]
    if args.resume:
        metadata = bench.load_resume_metadata(campaign_dir, suite)
        metadata["preflight"] = preflight
        metadata["deviations"] = deviations
        metadata.setdefault("resume_epochs", []).append(int(time.time()))
    else:
        metadata = bench.new_metadata(
            suite,
            campaign_dir.name,
            "run",
            execution_repo,
            preflight,
            deviations,
            args.ollama_host,
        )
    metadata_path = campaign_dir / "uat-meta.json"
    bench.write_metadata(metadata_path, metadata)
    print(f"campaign: {campaign_dir}")
    bench.process_runs(
        suite,
        execution_repo,
        campaign_dir,
        metadata,
        dry_run=False,
        resume=args.resume,
        binary_dir=binary_dir,
        ollama_host=args.ollama_host,
    )
    report_path = bench.generate_report(campaign_dir, metadata)
    print(f"metadata: {metadata_path}")
    print(f"report skeleton: {report_path}")
    if any(record.get("status") == "blocked" for record in metadata["runs"]):
        return 3
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except bench.BenchError as error:
        print(f"cm4x pinned campaign: {error}")
        raise SystemExit(2) from error
