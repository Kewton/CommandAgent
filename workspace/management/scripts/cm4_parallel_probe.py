#!/usr/bin/env python3
"""Run four isolated Community headless processes concurrently."""
from __future__ import annotations

import argparse
import concurrent.futures
import hashlib
import json
import os
import subprocess
import time
from pathlib import Path
from typing import Any

import bench
import cm3_matrix


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def tree_hash(root: Path) -> tuple[str, list[dict[str, str]]]:
    rows = []
    for path in sorted(item for item in root.rglob("*") if item.is_file()):
        rows.append(
            {"path": str(path.relative_to(root)), "sha256": sha256_file(path)}
        )
    digest = hashlib.sha256()
    for row in rows:
        digest.update(row["path"].encode())
        digest.update(b"\0")
        digest.update(row["sha256"].encode())
        digest.update(b"\n")
    return digest.hexdigest(), rows


def final_json_line(stdout: str) -> dict[str, Any]:
    for line in reversed(stdout.splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict) and value.get("schema_version") == (
            "commandagent.headless-summary/v1"
        ):
            return value
    raise ValueError("headless summary JSON was not the final machine-readable line")


def verify_isolation(records: list[dict[str, Any]]) -> dict[str, Any]:
    workspace_paths = [record["workspace"] for record in records]
    state_paths = [record["state_dir"] for record in records]
    run_ids = [record["summary"]["run_id"] for record in records]
    workspace_unique = len(set(workspace_paths)) == len(records)
    state_unique = len(set(state_paths)) == len(records)
    run_id_unique = len(set(run_ids)) == len(records)
    path_binding = all(
        record["summary"]["artifacts_dir"] == record["workspace"]
        and record["summary"]["events_path"]
        == str(Path(record["state_dir"]) / "events.jsonl")
        for record in records
    )
    cross_references = []
    all_roots = workspace_paths + state_paths
    for record in records:
        own = {record["workspace"], record["state_dir"]}
        events_text = Path(record["summary"]["events_path"]).read_text(
            encoding="utf-8", errors="replace"
        )
        for root in all_roots:
            if root not in own and root in events_text:
                cross_references.append(
                    {"run": record["name"], "foreign_root": root}
                )
    return {
        "workspace_paths_unique": workspace_unique,
        "state_paths_unique": state_unique,
        "run_ids_unique": run_id_unique,
        "summary_paths_bound_to_owner": path_binding,
        "foreign_path_references": cross_references,
        "cross_contamination_zero": all(
            (workspace_unique, state_unique, run_id_unique, path_binding)
        )
        and not cross_references,
    }


def run_one(
    binary: Path,
    suite: bench.SuiteDefinition,
    run: bench.RunSpec,
    workspace_root: Path,
    ollama_host: str,
) -> dict[str, Any]:
    workspace = workspace_root / "workspaces" / run.name
    state_dir = workspace_root / "states" / run.name
    log_dir = workspace_root / "logs"
    result = bench._procure_empty_run(workspace)
    result = bench._supply_community_measurement_inputs(
        result, bench.repository_root(), workspace
    )
    if not result.ok:
        raise RuntimeError(f"{run.name} procurement failed: {result.reason}")
    state_dir.mkdir(parents=True, exist_ok=False)
    log_dir.mkdir(parents=True, exist_ok=True)
    command = bench.build_command(suite, run, ollama_host)
    command[0] = str(binary)
    command[-1:-1] = [
        "--cwd",
        str(workspace),
        "--state-dir",
        str(state_dir),
        "--summary-json",
        "--no-footer",
    ]
    environment = os.environ.copy()
    events_path = state_dir / "events.jsonl"
    environment["COMMANDAGENT_EVAL_EVENTS"] = str(events_path)
    started = time.time()
    process = subprocess.run(
        command,
        cwd=workspace,
        env=environment,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
    )
    ended = time.time()
    (log_dir / f"{run.name}.stdout").write_text(process.stdout, encoding="utf-8")
    (log_dir / f"{run.name}.stderr").write_text(process.stderr, encoding="utf-8")
    summary = final_json_line(process.stdout)
    artifact_hash, artifact_files = tree_hash(workspace)
    scrub = bench.scrub_path(workspace)
    return {
        "name": run.name,
        "goal_id": run.goal_id,
        "workspace": str(workspace),
        "state_dir": str(state_dir),
        "command_argv": command,
        "started_epoch": started,
        "ended_epoch": ended,
        "duration_secs": ended - started,
        "exit_code": process.returncode,
        "stdout_sha256": hashlib.sha256(process.stdout.encode()).hexdigest(),
        "stderr_sha256": hashlib.sha256(process.stderr.encode()).hexdigest(),
        "summary": summary,
        "events_sha256": sha256_file(events_path),
        "artifact_tree_sha256": artifact_hash,
        "artifact_files": artifact_files,
        "scrub_ok": scrub.ok,
        "scrub_findings": list(scrub.findings),
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--binary-sha256", required=True)
    parser.add_argument("--suite", type=Path, required=True)
    parser.add_argument("--suite-sha256", required=True)
    parser.add_argument("--workspace-root", type=Path, required=True)
    parser.add_argument("--ollama-host", default="http://127.0.0.1:11434")
    parser.add_argument("--single-p50", type=float, required=True)
    parser.add_argument("--output", type=Path, required=True)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    binary = args.binary.resolve()
    suite = bench.load_suite(args.suite.resolve())
    if sha256_file(binary) != args.binary_sha256:
        raise SystemExit("parallel preflight: binary SHA-256 mismatch")
    if bench.sha256_file(suite.path) != args.suite_sha256:
        raise SystemExit("parallel preflight: suite SHA-256 mismatch")
    if suite.think is not None:
        raise SystemExit("parallel preflight: qwen3.6 control must omit think")
    if len(suite.runs) < 4:
        raise SystemExit("parallel preflight: suite requires at least four runs")
    workspace_root = args.workspace_root.resolve()
    workspace_root.mkdir(parents=True, exist_ok=False)
    provider_gate = bench.provider_reachability_preflight(
        suite, binary, bench.repository_root(), args.ollama_host
    )
    selected = suite.runs[:4]
    started = time.time()
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
        futures = [
            executor.submit(
                run_one, binary, suite, run, workspace_root, args.ollama_host
            )
            for run in selected
        ]
        records = [future.result() for future in futures]
    ended = time.time()
    records.sort(key=lambda record: record["name"])
    isolation = verify_isolation(records)
    durations = [float(record["duration_secs"]) for record in records]
    makespan = ended - started
    document = {
        "schema_version": "commandagent.cm4-parallel-probe/v1",
        "binary_sha256": args.binary_sha256,
        "suite_sha256": args.suite_sha256,
        "planner_model": suite.planner_model,
        "planner_provider": suite.planner_provider,
        "think": None,
        "executor_model": selected[0].executor,
        "executor_provider": suite.provider,
        "provider_gate": provider_gate,
        "parallelism": 4,
        "parallel_started_epoch": started,
        "parallel_ended_epoch": ended,
        "parallel_makespan_secs": makespan,
        "individual_duration_secs": cm3_matrix.distribution(durations),
        "historical_single_p50_secs": args.single_p50,
        "makespan_to_single_p50_ratio": makespan / args.single_p50,
        "individual_p50_to_single_p50_ratio": (
            cm3_matrix.distribution(durations)["p50"] / args.single_p50
        ),
        "effective_speedup_vs_sequential_observed": sum(durations) / makespan,
        "isolation": isolation,
        "all_scrub_ok": all(record["scrub_ok"] for record in records),
        "records": records,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(document, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    if not isolation["cross_contamination_zero"]:
        raise SystemExit("parallel isolation violation")
    if not document["all_scrub_ok"]:
        raise SystemExit("parallel artifact scrub failed")


if __name__ == "__main__":
    main()
