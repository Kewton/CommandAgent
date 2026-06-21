#!/usr/bin/env python3
import argparse
import csv
import json
import sys
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from eval_failure_observation import (  # noqa: E402
    OBSERVATION_FIELD_NAMES,
    category_for_reason,
    contract_layer_for_reason,
    normalize_observation,
)


def parse_args():
    parser = argparse.ArgumentParser(description="Report or recheck CommandAgent eval roots")
    parser.add_argument("root")
    parser.add_argument("--cases-dir", default="eval/cases/smoke")
    parser.add_argument("--recheck", action="store_true")
    return parser.parse_args()


def read_cases(cases_dir):
    cases = {}
    for path in Path(cases_dir).glob("*.yaml"):
        case = read_case(path)
        cases[case["id"]] = case
    return cases


def read_case(path):
    data = {"required_paths": [], "must_include": {}, "type": "semantic"}
    in_success_check = False
    in_required_paths = False
    in_must_include = False
    current_must_include_path = None
    with open(path, encoding="utf-8") as handle:
        for raw in handle:
            line = raw.rstrip("\n")
            stripped = line.strip()
            if not stripped:
                continue
            indent = len(line) - len(line.lstrip(" "))
            if not line.startswith(" ") and ":" in line:
                key, value = line.split(":", 1)
                key = key.strip()
                in_success_check = key == "success_check"
                in_required_paths = False
                in_must_include = False
                current_must_include_path = None
                if key == "id":
                    data["id"] = unquote(value.strip())
            elif in_success_check and indent == 2 and stripped == "required_paths:":
                in_required_paths = True
                in_must_include = False
            elif in_success_check and indent == 2 and stripped.startswith("type:"):
                data["type"] = unquote(stripped.split(":", 1)[1].strip())
                in_required_paths = False
                in_must_include = False
            elif in_success_check and indent == 2 and stripped == "must_include:":
                in_required_paths = False
                in_must_include = True
                current_must_include_path = None
            elif in_required_paths and stripped.startswith("- "):
                data["required_paths"].append(unquote(stripped[2:].strip()))
            elif in_must_include and indent == 4 and stripped.endswith(":"):
                current_must_include_path = unquote(stripped[:-1].strip())
                data["must_include"].setdefault(current_must_include_path, [])
            elif in_must_include and indent >= 6 and stripped.startswith("- ") and current_must_include_path:
                data["must_include"][current_must_include_path].append(
                    unquote(stripped[2:].strip())
                )
    return data


def unquote(value):
    if len(value) >= 2 and value[0] == value[-1] and value[0] in {'"', "'"}:
        return value[1:-1]
    return value


def read_summary(path):
    with open(path, encoding="utf-8", newline="") as handle:
        return list(csv.DictReader(handle, delimiter="\t"))


def write_summary(path, rows):
    fieldnames = [
        "case_id",
        "run",
        "rc",
        "elapsed_ms",
        "success",
        "reason",
        "failure_category",
        "contract_layer",
        *OBSERVATION_FIELD_NAMES,
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
    with open(path, "w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, delimiter="\t", fieldnames=fieldnames)
        writer.writeheader()
        writer.writerows(rows)


def recheck(root, cases):
    rows = []
    for meta_path in sorted(root.glob("*/*/meta.json")):
        meta = json.loads(meta_path.read_text(encoding="utf-8"))
        case = cases.get(
            meta["case_id"],
            {"required_paths": [], "must_include": {}, "type": "semantic"},
        )
        workspace = meta_path.parent / "workspace"
        missing = [
            path for path in case["required_paths"] if not (workspace / path).exists()
        ]
        mismatches = semantic_mismatches(workspace, case, missing)
        rc = int(meta.get("rc", 1))
        success = rc == 0 and not missing and not mismatches
        reason = (
            "ok"
            if success
            else (
                "semantic_missing:" + ",".join(missing)
                if missing
                else (
                    "semantic_mismatch:" + ",".join(mismatches)
                    if mismatches
                    else f"rc:{rc}"
                )
            )
        )
        rows.append(
            {
                "case_id": meta["case_id"],
                "run": str(meta["run_index"]),
                "rc": str(rc),
                "elapsed_ms": str(meta.get("elapsed_ms", 0)),
                "success": str(success).lower(),
                "reason": reason,
                "failure_category": categorize(reason),
                "contract_layer": contract_layer(reason),
                "active_job": meta.get("active_job", derive_active_job(reason)),
                "recovery_owner": meta.get("recovery_owner", derive_recovery_owner(reason)),
                "target_path": meta.get("target_path", first_reason_target(reason)),
                "target_role": meta.get("target_role", artifact_role_for_path(first_reason_target(reason))),
                "repair_action": meta.get("repair_action", derive_repair_action(reason)),
                "tool_policy": meta.get("tool_policy", derive_tool_policy(reason)),
                "attempt_outcome": meta.get("attempt_outcome", "not_attempted" if reason != "ok" else "passed"),
                "evidence_binding_status": meta.get("evidence_binding_status", "unknown" if reason != "ok" else "bound"),
                "completion_evidence_status": meta.get("completion_evidence_status", "failed" if reason != "ok" else "passed"),
                "explicit_stop_reason": meta.get("explicit_stop_reason", ""),
            }
        )
        observation = normalize_observation(
            {
                **meta,
                **rows[-1],
                "reason": reason,
                "success": success,
                "rc": rc,
            }
        )
        rows[-1].update(
            {name: observation.get(name, "") for name in OBSERVATION_FIELD_NAMES}
        )
        rows[-1]["failure_category"] = observation["failure_category"]
        rows[-1]["contract_layer"] = observation["contract_layer"]
    out = root / "recheck_summary.tsv"
    write_summary(out, rows)
    return rows, out


def semantic_mismatches(workspace, case, missing):
    mismatches = []
    for path, needles in case.get("must_include", {}).items():
        target = workspace / path
        if not target.exists():
            if path not in missing:
                missing.append(path)
            continue
        text = target.read_text(encoding="utf-8", errors="replace")
        for needle in needles:
            if not semantic_contains(text, needle, case):
                mismatches.append(f"{path}:{needle}")
    return mismatches


def semantic_contains(text, needle, case):
    if case.get("type") == "semantic":
        return needle.casefold() in text.casefold()
    return needle in text


def categorize(reason):
    return category_for_reason(reason)


def contract_layer(reason):
    return contract_layer_for_reason(reason)


def derive_active_job(reason):
    category = categorize(reason)
    if reason == "ok":
        return "none"
    if category == "setup":
        return "setup_bootstrap"
    if category == "tool_protocol":
        return "tool_protocol_correction"
    if category == "profile" and ("route" in reason or "integration" in reason):
        return "route_integration_repair"
    if category == "planning" and first_reason_target(reason):
        role = artifact_role_for_path(first_reason_target(reason))
        if role == "test":
            return "test_artifact_completion"
        if role == "docs":
            return "documentation_repair"
        if role in {"setup_manifest", "setup_config"}:
            return "manifest_repair"
        return "scaffold_materialization"
    if category == "planning":
        return "verifier_contract_correction"
    if category in {"quality", "verifier", "profile"}:
        return "source_implementation_repair"
    return "explicit_stop"


def derive_recovery_owner(reason):
    job = derive_active_job(reason)
    if job == "setup_bootstrap":
        return "setup"
    if job == "manifest_repair":
        return "manifest"
    if job == "scaffold_materialization":
        return "scaffold"
    if job == "route_integration_repair":
        return "route_integration"
    if job == "test_artifact_completion":
        return "test"
    if job == "documentation_repair":
        return "docs"
    if job == "tool_protocol_correction":
        return "tool_protocol"
    if job == "verifier_contract_correction":
        return "verifier_contract"
    if job == "none":
        return "none"
    if job == "explicit_stop":
        return "explicit_stop"
    return "source"


def derive_repair_action(reason):
    job = derive_active_job(reason)
    if job == "setup_bootstrap":
        return "install_or_prepare_dependencies"
    if job == "manifest_repair":
        return "add_missing_manifest_dependency"
    if job in {"scaffold_materialization", "test_artifact_completion"}:
        return "create_required_artifact"
    if job == "route_integration_repair":
        return "connect_existing_artifact_to_entrypoint"
    if job == "documentation_repair":
        return "update_docs_literal"
    if job == "tool_protocol_correction":
        return "correct_tool_protocol"
    if job == "verifier_contract_correction":
        return "replace_invalid_verifier_command"
    if job == "explicit_stop":
        return "stop_with_structured_evidence"
    if job == "none":
        return "none"
    return "edit_source_for_diagnostic"


def derive_tool_policy(reason):
    job = derive_active_job(reason)
    if job == "setup_bootstrap":
        return "verifier_owned_setup_only"
    if job == "manifest_repair":
        return "setup_config_mutation_only"
    if job in {"tool_protocol_correction", "explicit_stop", "none"}:
        return job
    if job == "verifier_contract_correction":
        return "read_only"
    return "file_mutation_repair"


def first_reason_target(reason):
    for prefix in ["missing:", "semantic_missing:", "semantic_mismatch:"]:
        if reason.startswith(prefix):
            value = reason[len(prefix):].split(",", 1)[0]
            if ":" in value and prefix == "semantic_mismatch:":
                value = value.split(":", 1)[0]
            if "/" in value or "." in value:
                return value
    return ""


def artifact_role_for_path(path):
    if not path:
        return ""
    name = path.rsplit("/", 1)[-1]
    if name in {"package.json", "Cargo.toml", "pyproject.toml"} or name.startswith("requirements"):
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


def render_report(rows):
    total = len(rows)
    success = sum(1 for row in rows if row["success"] == "true")
    categories = {}
    layers = {}
    terminal_states = {}
    diagnostics = {}
    by_case = {}
    recovery_jobs = {}
    for row in rows:
        observation = normalize_observation(row)
        category = row.get("failure_category") or observation["failure_category"]
        layer = row.get("contract_layer") or observation["contract_layer"]
        terminal_state = row.get("terminal_state") or observation["terminal_state"]
        diagnostic_code = row.get("diagnostic_code") or observation["diagnostic_code"]
        job = row.get("active_job") or derive_active_job(row["reason"])
        categories[category] = categories.get(category, 0) + 1
        layers[layer] = layers.get(layer, 0) + 1
        terminal_states[terminal_state] = terminal_states.get(terminal_state, 0) + 1
        diagnostics[diagnostic_code] = diagnostics.get(diagnostic_code, 0) + 1
        recovery_jobs[job] = recovery_jobs.get(job, 0) + 1
        stats = by_case.setdefault(row["case_id"], [0, 0])
        stats[1] += 1
        if row["success"] == "true":
            stats[0] += 1

    lines = [
        "# Eval Report",
        "",
        f"success: {success}/{total}",
        "",
        "## Failure Categories",
    ]
    for name, count in sorted(categories.items()):
        lines.append(f"- {name}: {count}")
    lines.extend(["", "## Terminal States"])
    for name, count in sorted(terminal_states.items()):
        lines.append(f"- {name}: {count}")
    lines.extend(["", "## Contract Layers"])
    for name, count in sorted(layers.items()):
        lines.append(f"- {name}: {count}")
    lines.extend(["", "## Lifecycle Funnel"])
    for name, count in sorted(terminal_states.items()):
        lines.append(f"- {name}: {count}")
    lines.extend(["", "## Diagnostic Codes"])
    for name, count in sorted(diagnostics.items()):
        lines.append(f"- {name}: {count}")
    lines.extend(["", "## Recovery Jobs"])
    for name, count in sorted(recovery_jobs.items()):
        lines.append(f"- {name}: {count}")
    lines.extend(["", "## By Case"])
    for case_id, (case_success, case_total) in sorted(by_case.items()):
        lines.append(f"- {case_id}: {case_success}/{case_total}")
    return "\n".join(lines) + "\n"


def main():
    args = parse_args()
    root = Path(args.root)
    cases = read_cases(args.cases_dir)
    if args.recheck:
        rows, out = recheck(root, cases)
        print(f"wrote {out}")
    else:
        rows = read_summary(root / "summary.tsv")
    print(render_report(rows))


if __name__ == "__main__":
    main()
