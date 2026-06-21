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

REPO_ROOT = Path(sys.argv[1]).resolve()
sys.path.insert(0, str(REPO_ROOT / "scripts"))
from eval_failure_observation import (  # noqa: E402
    OBSERVATION_FIELD_NAMES,
    category_for_reason,
    contract_layer_for_reason,
    normalize_observation,
    terminal_state_from_reason,
)


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
    terminal_state = terminal_state_from_reason("rc:1", evidence, {"success": False})
    if terminal_state in {
        "port_in_use",
        "provider_transport_failed",
        "provider_parse_failed",
        "step_policy_failed",
        "dependency_missing",
        "setup_failed",
    }:
        return terminal_state
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


RECOVERY_FIELD_NAMES = [
    "active_job",
    "recovery_owner",
    "target_path",
    "target_role",
    "repair_action",
    "tool_policy",
    "attempt_outcome",
    "evidence_binding_status",
    "completion_evidence_status",
    "explicit_stop_reason",
]


def recovery_fields(reason, evidence, case):
    fields = derived_recovery_fields(reason, case)
    for key in RECOVERY_FIELD_NAMES:
        parsed = first_contract_value(evidence, key)
        if parsed:
            fields[key] = parsed
    if not fields.get("tool_policy"):
        fields["tool_policy"] = first_contract_value(evidence, "tool_policy_projection")
    if not fields.get("target_path"):
        fields["target_path"] = (
            first_contract_value(evidence, "repair_target")
            or first_contract_value(evidence, "target_path")
        )
    if not fields.get("target_role"):
        fields["target_role"] = first_contract_value(evidence, "artifact_role")
    if not fields.get("evidence_binding_status"):
        fields["evidence_binding_status"] = status_from_contract_list(
            evidence, "evidence_binding"
        )
    if not fields.get("completion_evidence_status"):
        fields["completion_evidence_status"] = status_from_contract_list(
            evidence, "completion_evidence"
        )
    if not fields.get("attempt_outcome"):
        fields["attempt_outcome"] = status_from_contract_list(evidence, "attempt_outcomes")
    return {key: fields.get(key, "") for key in RECOVERY_FIELD_NAMES}


def first_contract_value(evidence, key):
    patterns = [
        rf"^- {re.escape(key)}:\s*(.+)$",
        rf"\b{re.escape(key)}=([^\s,]+)",
    ]
    for pattern in patterns:
        match = re.search(pattern, evidence, flags=re.MULTILINE)
        if match:
            return match.group(1).strip()
    return ""


def status_from_contract_list(evidence, key):
    line = first_contract_value(evidence, key)
    if not line:
        return ""
    match = re.search(r"\bstatus=([a-z_]+)", line)
    if match:
        return match.group(1)
    match = re.search(r"\boutcome=([a-z_]+)", line)
    if match:
        return match.group(1)
    return "present"


def derived_recovery_fields(reason, case):
    category = failure_category(reason)
    layer = contract_layer(reason)
    target = first_reason_target(reason)
    role = artifact_role_for_path(target)
    fields = {
        "active_job": "",
        "recovery_owner": "",
        "target_path": target,
        "target_role": role,
        "repair_action": "",
        "tool_policy": "",
        "attempt_outcome": "not_attempted" if reason != "ok" else "passed",
        "evidence_binding_status": "unknown" if reason != "ok" else "bound",
        "completion_evidence_status": "failed" if reason != "ok" else "passed",
        "explicit_stop_reason": "",
    }
    if reason == "ok":
        fields["active_job"] = "none"
        fields["recovery_owner"] = "none"
        fields["repair_action"] = "none"
        fields["tool_policy"] = "none"
        return fields
    if category == "setup":
        fields.update(
            active_job="setup_bootstrap",
            recovery_owner="setup",
            repair_action="install_or_prepare_dependencies",
            tool_policy="verifier_owned_setup_only",
        )
    elif category == "profile":
        if "route" in reason or "integration" in reason:
            fields.update(
                active_job="route_integration_repair",
                recovery_owner="route_integration",
                repair_action="connect_existing_artifact_to_entrypoint",
                tool_policy="file_mutation_repair",
            )
        else:
            fields.update(
                active_job="source_implementation_repair",
                recovery_owner="source",
                repair_action="edit_source_for_diagnostic",
                tool_policy="file_mutation_repair",
            )
    elif category == "tool_protocol":
        fields.update(
            active_job="tool_protocol_correction",
            recovery_owner="tool_protocol",
            repair_action="correct_tool_protocol",
            tool_policy="tool_protocol_correction",
        )
    elif category == "planning":
        if target:
            job, owner, action = missing_artifact_recovery(role)
            fields.update(active_job=job, recovery_owner=owner, repair_action=action)
        else:
            fields.update(
                active_job="verifier_contract_correction",
                recovery_owner="verifier_contract",
                repair_action="replace_invalid_verifier_command",
            )
        fields["tool_policy"] = "read_only"
    elif category == "quality":
        fields.update(
            active_job="source_implementation_repair",
            recovery_owner="source",
            repair_action="edit_source_for_diagnostic",
            tool_policy="file_mutation_repair",
        )
    elif category == "verifier":
        fields.update(
            active_job="source_implementation_repair",
            recovery_owner="source",
            repair_action="edit_source_for_diagnostic",
            tool_policy="file_mutation_repair",
        )
    else:
        fields.update(
            active_job="explicit_stop",
            recovery_owner="explicit_stop",
            repair_action="stop_with_structured_evidence",
            tool_policy="explicit_stop",
            explicit_stop_reason=f"unclassified_{layer}",
        )
    return fields


def first_reason_target(reason):
    for prefix in [
        "missing:",
        "semantic_missing:",
        "semantic_mismatch:",
        "profile_verification:",
    ]:
        if reason.startswith(prefix):
            value = reason[len(prefix) :].split(",", 1)[0]
            if ":" in value and prefix == "semantic_mismatch:":
                value = value.split(":", 1)[0]
            if "/" in value or "." in value:
                return value
    return ""


def artifact_role_for_path(path):
    if not path:
        return ""
    name = path.rsplit("/", 1)[-1]
    if name in {"package.json", "Cargo.toml", "pyproject.toml"} or name.startswith(
        "requirements"
    ):
        return "setup_manifest"
    if name.startswith(("next.config.", "postcss.config.", "tailwind.config.")):
        return "setup_config"
    if path in {"app/page.tsx", "src/app/page.tsx", "app/layout.tsx"}:
        return "entrypoint"
    if path.startswith("tests/") or "test" in name or name.endswith("_test.rs"):
        return "test"
    if name.endswith(".md"):
        return "docs"
    if name.endswith((".json", ".csv", ".yaml", ".yml")):
        return "structured_data"
    if name.endswith((".ts", ".tsx", ".js", ".jsx", ".rs", ".py")):
        return "implementation"
    return "unknown"


def missing_artifact_recovery(role):
    if role in {"setup_manifest", "setup_config"}:
        return "manifest_repair", "manifest", "add_missing_manifest_dependency"
    if role == "test":
        return "test_artifact_completion", "test", "create_required_artifact"
    if role == "docs":
        return "documentation_repair", "docs", "update_docs_literal"
    return "scaffold_materialization", "scaffold", "create_required_artifact"


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
            if not semantic_contains(text, needle, check):
                mismatches.append(f"{path}:{needle}")
    return missing, mismatches


def semantic_contains(text, needle, check):
    if check.get("type") == "semantic":
        return needle.casefold() in text.casefold()
    return needle in text


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


def failure_category(reason):
    return category_for_reason(reason)


def contract_layer(reason):
    return contract_layer_for_reason(reason)


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
    evidence = failure_evidence(workdir, stdout, stderr)
    reason = success_reason(
        workdir, rc, missing, semantic_missing, semantic_mismatches, stdout, stderr
    )
    category = failure_category(reason)
    layer = contract_layer(reason)
    recovery = recovery_fields(reason, evidence, case)
    observation = normalize_observation(
        {
            "reason": reason,
            "rc": rc,
            "success": success,
            "stdout": stdout,
            "stderr": stderr,
            "evidence": evidence,
            "command": " ".join(command),
            "failure_category": category,
            "contract_layer": layer,
            **recovery,
        }
    )
    category = observation["failure_category"]
    layer = observation["contract_layer"]

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
        "failure_category": category,
        "contract_layer": layer,
        **{name: observation.get(name, "") for name in OBSERVATION_FIELD_NAMES},
        **recovery,
    }
    meta_path.write_text(json.dumps(meta, indent=2) + "\n", encoding="utf-8")
    return [
        case["id"],
        str(run_index),
        str(rc),
        str(elapsed_ms),
        str(success).lower(),
        reason,
        category,
        layer,
        *(observation.get(name, "") for name in OBSERVATION_FIELD_NAMES),
        *(recovery[name] for name in RECOVERY_FIELD_NAMES),
    ]


def main():
    repo = REPO_ROOT
    args = parse_args()
    cases_dir = (repo / args.cases_dir).resolve()
    cases = [read_case(path) for path in sorted(cases_dir.glob("*.yaml"))]
    stamp = time.strftime("%Y%m%dT%H%M%S")
    root = (repo / args.out / stamp).resolve()
    root.mkdir(parents=True, exist_ok=True)
    rows = [
        [
            "case_id",
            "run",
            "rc",
            "elapsed_ms",
            "success",
            "reason",
            "failure_category",
            "contract_layer",
            *OBSERVATION_FIELD_NAMES,
            *RECOVERY_FIELD_NAMES,
        ]
    ]
    for case in cases:
        for run_index in range(1, args.runs + 1):
            rows.append(run_case(repo, root, args.binary, case, run_index, args))
    summary = "\n".join("\t".join(row) for row in rows) + "\n"
    (root / "summary.tsv").write_text(summary, encoding="utf-8")
    print(root)


if __name__ == "__main__":
    main()
PY
