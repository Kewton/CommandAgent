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
    data = {"required_paths": []}
    in_required_paths = False
    with open(path, encoding="utf-8") as handle:
        for raw in handle:
            line = raw.rstrip("\n")
            stripped = line.strip()
            if not stripped:
                continue
            if not line.startswith(" ") and ":" in line:
                key, value = line.split(":", 1)
                key = key.strip()
                if key == "id":
                    data["id"] = unquote(value.strip())
                in_required_paths = False
            elif stripped == "required_paths:":
                in_required_paths = True
            elif in_required_paths and stripped.startswith("- "):
                data["required_paths"].append(unquote(stripped[2:].strip()))
    return data


def unquote(value):
    if len(value) >= 2 and value[0] == value[-1] and value[0] in {'"', "'"}:
        return value[1:-1]
    return value


def read_summary(path):
    with open(path, encoding="utf-8", newline="") as handle:
        return list(csv.DictReader(handle, delimiter="\t"))


def write_summary(path, rows):
    fieldnames = ["case_id", "run", "rc", "elapsed_ms", "success", "reason"]
    with open(path, "w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, delimiter="\t", fieldnames=fieldnames)
        writer.writeheader()
        writer.writerows(rows)


def recheck(root, cases):
    rows = []
    for meta_path in sorted(root.glob("*/*/meta.json")):
        meta = json.loads(meta_path.read_text(encoding="utf-8"))
        case = cases.get(meta["case_id"], {"required_paths": []})
        workspace = meta_path.parent / "workspace"
        missing = [
            path for path in case["required_paths"] if not (workspace / path).exists()
        ]
        rc = int(meta.get("rc", 1))
        success = rc == 0 and not missing
        rows.append(
            {
                "case_id": meta["case_id"],
                "run": str(meta["run_index"]),
                "rc": str(rc),
                "elapsed_ms": str(meta.get("elapsed_ms", 0)),
                "success": str(success).lower(),
                "reason": "ok"
                if success
                else ("missing:" + ",".join(missing) if missing else f"rc:{rc}"),
            }
        )
    out = root / "recheck_summary.tsv"
    write_summary(out, rows)
    return rows, out


def categorize(reason):
    if reason == "ok":
        return "ok"
    if reason.startswith("missing:"):
        return "missing_artifact"
    if reason.startswith("rc:"):
        return "rc_nonzero"
    return "check_failed"


def render_report(rows):
    total = len(rows)
    success = sum(1 for row in rows if row["success"] == "true")
    categories = {}
    by_case = {}
    for row in rows:
        categories[categorize(row["reason"])] = categories.get(categorize(row["reason"]), 0) + 1
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
