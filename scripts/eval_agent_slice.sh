#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

python3 - "$repo_root" "$@" <<'PY'
import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path


def parse_args():
    parser = argparse.ArgumentParser(description="Run a CommandAgent eval slice")
    parser.add_argument("--cases-dir", default="eval/cases/smoke")
    parser.add_argument("--out", default="eval/runs")
    parser.add_argument("--runs", type=int, default=1)
    parser.add_argument("--binary", default="target/release/commandagent")
    parser.add_argument("--provider", default="ollama")
    parser.add_argument("--model", default=os.environ.get("COMMANDAGENT_MODEL", "default"))
    parser.add_argument("--timeout-secs", type=int, default=900)
    parser.add_argument("--dry-run", action="store_true")
    return parser.parse_args(sys.argv[2:])


def read_case(path):
    data = {
        "expected_artifacts": [],
        "verify": [],
        "mode": "plan-run",
        "fixture": None,
        "success_check": {
            "required_paths": [],
            "must_include": {},
        },
    }
    current_list = None
    in_success_check = False
    in_must_include = False
    current_must_include_path = None
    with open(path, encoding="utf-8") as handle:
        for raw in handle:
            line = raw.rstrip("\n")
            stripped = line.strip()
            if not stripped or stripped.startswith("#"):
                continue
            indent = len(line) - len(line.lstrip(" "))
            if not line.startswith(" ") and ":" in line:
                key, value = line.split(":", 1)
                key = key.strip()
                value = unquote(value.strip())
                in_success_check = key == "success_check"
                in_must_include = False
                current_must_include_path = None
                if key in {"id", "title", "profile", "style", "intent", "prompt", "mode", "fixture"}:
                    data[key] = value
                    current_list = None
                elif key in {"expected_artifacts", "verify"}:
                    current_list = key
                else:
                    current_list = None
            elif in_success_check:
                if indent == 2 and stripped.startswith("type:"):
                    data["success_check"]["type"] = unquote(stripped.split(":", 1)[1].strip())
                    current_list = None
                    in_must_include = False
                elif indent == 2 and stripped == "required_paths:":
                    current_list = "success_required_paths"
                    in_must_include = False
                elif indent == 2 and stripped == "must_include:":
                    current_list = None
                    in_must_include = True
                    current_must_include_path = None
                elif current_list == "success_required_paths" and stripped.startswith("- "):
                    data["success_check"]["required_paths"].append(unquote(stripped[2:].strip()))
                elif in_must_include and indent == 4 and stripped.endswith(":"):
                    current_must_include_path = unquote(stripped[:-1].strip())
                    data["success_check"]["must_include"].setdefault(current_must_include_path, [])
                elif in_must_include and indent >= 6 and stripped.startswith("- ") and current_must_include_path:
                    data["success_check"]["must_include"][current_must_include_path].append(
                        unquote(stripped[2:].strip())
                    )
            elif current_list and stripped.startswith("- "):
                data[current_list].append(unquote(stripped[2:].strip()))
    for required in ["id", "profile", "style", "prompt"]:
        if required not in data:
            raise SystemExit(f"{path}: missing required field {required}")
    if data["mode"] not in {"plan-run", "ultra-plan-run"}:
        raise SystemExit(f"{path}: unsupported mode {data['mode']}")
    return data


def unquote(value):
    if len(value) >= 2 and value[0] == value[-1] and value[0] in {'"', "'"}:
        return value[1:-1]
    return value


def git_value(repo, *args):
    try:
        return subprocess.check_output(["git", *args], cwd=repo, text=True).strip()
    except Exception:
        return "unknown"


def failure_evidence(workdir, stdout, stderr):
    parts = [stdout, stderr]
    repairs_dir = workdir / ".commandagent" / "repairs"
    if repairs_dir.is_dir():
        for path in sorted(repairs_dir.glob("*.md")):
            try:
                parts.append(path.read_text(encoding="utf-8", errors="replace"))
            except OSError:
                pass
    return "\n".join(parts)


def runtime_failure_reason(evidence):
    profile_match = re.search(
        r"profile verification failed[^\n]*?:\s*([a-z0-9_]+):", evidence
    )
    if profile_match:
        return "profile_verification:" + profile_match.group(1)
    tool_missing_match = re.search(
        r"tool_args_missing_required_field[\s\S]*?required string field `([^`]+)` was missing",
        evidence,
    )
    if tool_missing_match:
        return "tool_args_missing_required_field:" + tool_missing_match.group(1)
    tool_missing_match = re.search(r"missing string field `([^`]+)`", evidence)
    if "invalid tool arguments" in evidence and tool_missing_match:
        return "tool_args_missing_required_field:" + tool_missing_match.group(1)
    if "tool_args_invalid_json" in evidence or "arguments are not valid JSON" in evidence:
        return "tool_args_invalid_json"
    if "dependency_missing" in evidence:
        return "dependency_missing"
    return None


def semantic_failures(workdir, case):
    check = case.get("success_check") or {}
    missing = [
        path for path in check.get("required_paths", []) if not (workdir / path).exists()
    ]
    mismatches = []
    for path, needles in check.get("must_include", {}).items():
        target = workdir / path
        if not target.exists():
            if path not in missing:
                missing.append(path)
            continue
        text = target.read_text(encoding="utf-8", errors="replace")
        for needle in needles:
            if needle not in text:
                mismatches.append(f"{path}:{needle}")
    return missing, mismatches


def success_reason(workdir, rc, missing, semantic_missing, semantic_mismatches, stdout, stderr):
    evidence = ""
    if rc != 0:
        evidence = failure_evidence(workdir, stdout, stderr)
        runtime_reason = runtime_failure_reason(evidence)
        if runtime_reason:
            return runtime_reason
    if missing:
        return "missing:" + ",".join(missing)
    if semantic_missing:
        return "semantic_missing:" + ",".join(semantic_missing)
    if semantic_mismatches:
        return "semantic_mismatch:" + ",".join(semantic_mismatches)
    if rc == 0:
        return "ok"

    return f"rc:{rc}"


def run_case(repo, root, binary, case, run_index, args):
    run_dir = root / case["id"] / f"run-{run_index}"
    workdir = run_dir / "workspace"
    workdir.mkdir(parents=True, exist_ok=True)
    stdout_path = run_dir / "stdout.txt"
    stderr_path = run_dir / "stderr.txt"
    meta_path = run_dir / "meta.json"

    started = time.time()
    if case.get("fixture"):
        fixture = (repo / case["fixture"]).resolve()
        if not fixture.is_dir():
            raise SystemExit(f"{case['id']}: fixture not found: {fixture}")
        shutil.copytree(fixture, workdir, dirs_exist_ok=True)

    mode = case.get("mode", "plan-run")
    option_parts = [f"/{mode}", "--profile", case["profile"], "--style", case["style"]]
    if case.get("intent"):
        option_parts.extend(["--intent", case["intent"]])
    for artifact in case["expected_artifacts"]:
        option_parts.extend(["--artifact", artifact])
    prompt = " ".join(option_parts + [case["prompt"]])
    command = [
        str((repo / binary).resolve()),
        "--provider",
        args.provider,
        "--model",
        args.model,
        "--max-iterations",
        "8",
        "--yes",
        prompt,
    ]
    if args.dry_run:
        rc = 0
        stdout = f"dry-run: {case['id']}\n"
        stderr = ""
    else:
        process = subprocess.run(
            command,
            cwd=workdir,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=args.timeout_secs,
        )
        rc = process.returncode
        stdout = process.stdout
        stderr = process.stderr
    elapsed_ms = int((time.time() - started) * 1000)
    stdout_path.write_text(stdout, encoding="utf-8")
    stderr_path.write_text(stderr, encoding="utf-8")

    missing = [
        path for path in case["expected_artifacts"] if not (workdir / path).exists()
    ]
    semantic_missing, semantic_mismatches = semantic_failures(workdir, case)
    success = rc == 0 and not missing and not semantic_missing and not semantic_mismatches
    reason = success_reason(
        workdir, rc, missing, semantic_missing, semantic_mismatches, stdout, stderr
    )

    meta = {
        "case_id": case["id"],
        "run_index": run_index,
        "provider": args.provider,
        "model": args.model,
        "profile": case.get("profile"),
        "style": case.get("style"),
        "intent": case.get("intent"),
        "expected_artifacts": case.get("expected_artifacts", []),
        "success_check": case.get("success_check", {}),
        "mode": mode,
        "fixture": case.get("fixture"),
        "prompt": prompt,
        "binary": str((repo / binary).resolve()),
        "commit": git_value(repo, "rev-parse", "HEAD"),
        "dirty": bool(git_value(repo, "status", "--short")),
        "dry_run": args.dry_run,
        "timeout_secs": args.timeout_secs,
        "elapsed_ms": elapsed_ms,
        "rc": rc,
        "success": success,
        "success_check_reason": reason,
    }
    meta_path.write_text(json.dumps(meta, indent=2) + "\n", encoding="utf-8")
    return [case["id"], str(run_index), str(rc), str(elapsed_ms), str(success).lower(), reason]


def main():
    repo = Path(sys.argv[1]).resolve()
    args = parse_args()
    cases_dir = (repo / args.cases_dir).resolve()
    cases = [read_case(path) for path in sorted(cases_dir.glob("*.yaml"))]
    stamp = time.strftime("%Y%m%dT%H%M%S")
    root = (repo / args.out / stamp).resolve()
    root.mkdir(parents=True, exist_ok=True)
    rows = [["case_id", "run", "rc", "elapsed_ms", "success", "reason"]]
    for case in cases:
        for run_index in range(1, args.runs + 1):
            rows.append(run_case(repo, root, args.binary, case, run_index, args))
    summary = "\n".join("\t".join(row) for row in rows) + "\n"
    (root / "summary.tsv").write_text(summary, encoding="utf-8")
    print(root)


if __name__ == "__main__":
    main()
PY
