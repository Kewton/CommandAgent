#!/usr/bin/env python3
import argparse
import csv
import json
from pathlib import Path


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
            }
        )
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
    if reason == "ok":
        return "ok"
    if (
        reason.startswith("planning:")
        or reason.startswith("plan_lint")
        or "invalid ultra plan" in reason
        or "invalid step plan" in reason
    ):
        return "planning"
    if (
        reason.startswith("provider_transport")
        or reason.startswith("provider_parse")
        or "JSON parse failed" in reason
        or "tool call is missing a tool name" in reason
    ):
        return "provider_transport"
    if (
        reason.startswith("tool_args_")
        or reason.startswith("tool_protocol")
        or reason.startswith("tool_protocol_failure:")
    ):
        return "tool_protocol"
    if reason.startswith("step_policy:") or reason == "read_only_step_mutation":
        return "step_policy"
    if reason.startswith("profile_verification:"):
        return "profile"
    if (
        reason == "dependency_missing"
        or reason.startswith("setup:")
        or reason.startswith("dependency_setup:")
    ):
        return "setup"
    if (
        reason.startswith("quality:")
        or reason.startswith("app_quality:")
        or "blank_ui" in reason
        or "visual" in reason
    ):
        return "quality"
    if reason.startswith("missing:"):
        return "planning"
    if reason.startswith("semantic_missing:") or reason.startswith("semantic_mismatch:"):
        return "quality"
    if reason.startswith("rc:"):
        return "verifier"
    if reason.startswith("command_failed:") or reason.startswith("blocked:"):
        return "verifier"
    return "unknown"


def contract_layer(reason):
    category = categorize(reason)
    if category == "planning":
        return "planning_contract"
    if category in ("provider_transport", "tool_protocol", "step_policy"):
        return "execution_contract"
    if category == "profile":
        return "profile_contract"
    if category == "setup":
        return "setup_bootstrap_contract"
    if category == "verifier":
        return "verification_contract"
    if category == "quality":
        return "eval_success_contract"
    if category == "ok":
        return "ok"
    return "unknown_contract"


def render_report(rows):
    total = len(rows)
    success = sum(1 for row in rows if row["success"] == "true")
    categories = {}
    layers = {}
    by_case = {}
    for row in rows:
        category = row.get("failure_category") or categorize(row["reason"])
        layer = row.get("contract_layer") or contract_layer(row["reason"])
        categories[category] = categories.get(category, 0) + 1
        layers[layer] = layers.get(layer, 0) + 1
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
    lines.extend(["", "## Contract Layers"])
    for name, count in sorted(layers.items()):
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
