#!/usr/bin/env python3
"""Dry-run planner for CommandAgent Codex issue orchestration.

The initial CommandAgent port is intentionally planning-only. It writes
inspectable run artifacts and refuses mutating phases until they are implemented
with separate tests and explicit flags.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable, Sequence

REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_REPO = "Kewton/CommandAgent"
DEFAULT_BASE = "origin/develop"
DEFAULT_RUNS_DIR = REPO_ROOT / "workspace" / "management" / "runs"

MUTATING_FLAGS = (
    "create_worktrees",
    "dispatch_commandmate",
    "create_prs",
    "merge_prs",
    "write_uat",
    "create_uat_fix_worktrees",
)


@dataclass(frozen=True)
class Issue:
    number: int
    title: str
    body: str
    labels: tuple[str, ...] = ()


@dataclass(frozen=True)
class IssueAnalysis:
    issue: Issue
    objective: str
    acceptance_criteria: tuple[str, ...]
    suspected_files: tuple[str, ...]
    reference_files: tuple[str, ...]
    test_expectations: tuple[str, ...]
    dependency_hints: tuple[str, ...]
    questions: tuple[str, ...]
    branch_name: str
    worktree_path: str


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("issues", nargs="+", type=int, help="GitHub issue numbers")
    parser.add_argument("--dry-run", action="store_true", help="Write planning artifacts only")
    parser.add_argument("--issue-json", type=Path, help="Issue fixture JSON for offline planning")
    parser.add_argument("--run-id", help="Stable run id override")
    parser.add_argument("--runs-dir", type=Path, default=DEFAULT_RUNS_DIR)
    parser.add_argument("--repo", default=DEFAULT_REPO)
    parser.add_argument("--base", default=DEFAULT_BASE)
    parser.add_argument("--max-parallel", type=int, default=3)
    parser.add_argument("--skip-enhance", action="store_true")
    parser.add_argument("--create-worktrees", action="store_true")
    parser.add_argument("--dispatch-commandmate", action="store_true")
    parser.add_argument("--create-prs", action="store_true")
    parser.add_argument("--merge-prs", action="store_true")
    parser.add_argument("--write-uat", action="store_true")
    parser.add_argument("--create-uat-fix-worktrees", action="store_true")
    return parser


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    return build_parser().parse_args(argv)


def slugify(value: str, *, max_len: int = 48) -> str:
    lowered = value.lower()
    normalized = re.sub(r"[^a-z0-9]+", "-", lowered).strip("-")
    compact = re.sub(r"-{2,}", "-", normalized)
    return (compact[:max_len].strip("-") or "task")


def load_issues(numbers: Sequence[int], fixture_path: Path | None, repo: str) -> list[Issue]:
    if fixture_path is not None:
        return load_issues_from_fixture(numbers, fixture_path)
    return [fetch_issue_with_gh(number, repo) for number in numbers]


def load_issues_from_fixture(numbers: Sequence[int], fixture_path: Path) -> list[Issue]:
    raw = json.loads(fixture_path.read_text(encoding="utf-8"))
    items = raw["issues"] if isinstance(raw, dict) and "issues" in raw else raw
    if not isinstance(items, list):
        raise ValueError("--issue-json must contain a list or an object with an 'issues' list")

    by_number: dict[int, Issue] = {}
    for item in items:
        if not isinstance(item, dict):
            continue
        number = int(item["number"])
        labels = normalize_labels(item.get("labels", []))
        by_number[number] = Issue(
            number=number,
            title=str(item.get("title", "")),
            body=str(item.get("body", "")),
            labels=labels,
        )

    missing = [number for number in numbers if number not in by_number]
    if missing:
        raise ValueError(f"fixture does not contain issues: {missing}")
    return [by_number[number] for number in numbers]


def normalize_labels(raw: object) -> tuple[str, ...]:
    if not isinstance(raw, list):
        return ()
    labels: list[str] = []
    for item in raw:
        if isinstance(item, str):
            labels.append(item)
        elif isinstance(item, dict) and "name" in item:
            labels.append(str(item["name"]))
    return tuple(labels)


def fetch_issue_with_gh(number: int, repo: str) -> Issue:
    completed = subprocess.run(
        [
            "gh",
            "issue",
            "view",
            str(number),
            "--repo",
            repo,
            "--json",
            "number,title,body,labels",
        ],
        cwd=REPO_ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    raw = json.loads(completed.stdout)
    return Issue(
        number=int(raw["number"]),
        title=str(raw.get("title", "")),
        body=str(raw.get("body", "")),
        labels=normalize_labels(raw.get("labels", [])),
    )


def analyze_issue(issue: Issue, repo_name: str, *, skip_enhance: bool) -> IssueAnalysis:
    text = f"{issue.title}\n\n{issue.body}"
    objective = first_nonempty_line(issue.body) or issue.title
    acceptance = extract_acceptance_criteria(issue.body)
    suspected, references = classify_file_candidates(extract_file_candidates(text))
    tests = extract_test_expectations(text)
    hints = extract_dependency_hints(text)
    questions: list[str] = []

    if not acceptance and not skip_enhance:
        questions.append("Acceptance criteria are unclear; add 1-3 concrete completion checks.")
    if not suspected and not skip_enhance:
        questions.append("Affected files are unclear; add likely modules or paths.")

    slug = slugify(issue.title)
    return IssueAnalysis(
        issue=issue,
        objective=objective,
        acceptance_criteria=tuple(acceptance),
        suspected_files=tuple(suspected),
        reference_files=tuple(references),
        test_expectations=tuple(tests),
        dependency_hints=tuple(hints),
        questions=tuple(questions[:3]),
        branch_name=f"feature/issue-{issue.number}-{slug}",
        worktree_path=f"../{repo_name}-issue-{issue.number}-{slug}",
    )


def first_nonempty_line(value: str) -> str:
    for line in value.splitlines():
        stripped = line.strip(" -#\t")
        if stripped:
            return stripped
    return ""


def extract_acceptance_criteria(body: str) -> list[str]:
    lines = body.splitlines()
    out: list[str] = []
    in_section = False
    heading_re = re.compile(r"^#{1,6}\s+")
    trigger_re = re.compile(r"(acceptance|criteria|受入|受け入れ|完了条件|期待結果)", re.I)
    for line in lines:
        stripped = line.strip()
        if heading_re.match(stripped):
            in_section = bool(trigger_re.search(stripped))
            continue
        if in_section and stripped.startswith(("-", "*")):
            out.append(stripped.lstrip("-* ").strip())
        elif in_section and re.match(r"^\d+\.\s+", stripped):
            out.append(re.sub(r"^\d+\.\s+", "", stripped).strip())
    return [item for item in out if item]


def extract_file_candidates(text: str) -> list[str]:
    patterns = [
        r"`([^`\s]+\.(?:rs|md|toml|json|yaml|yml|py|sh|ts|tsx|js|jsx))`",
        (
            r"\b((?:src|tests|scripts|docs|eval|\.github|\.codex)/"
            r"[A-Za-z0-9_./-]+)\b"
        ),
        (
            r"\b([A-Za-z0-9_.-]+/(?:[A-Za-z0-9_.-]+/)*"
            r"[A-Za-z0-9_.-]+\.(?:rs|md|toml|json|yaml|yml|py|sh|ts|tsx|js|jsx))\b"
        ),
    ]
    seen: set[str] = set()
    out: list[str] = []
    for pattern in patterns:
        for match in re.finditer(pattern, text):
            candidate = match.group(1).strip()
            if ".." in candidate or candidate.startswith("/"):
                continue
            if candidate.split("/", 1)[0] in {"Users", "home", "tmp", "private", "var"}:
                continue
            if candidate not in seen:
                seen.add(candidate)
                out.append(candidate)
    return out


def classify_file_candidates(candidates: Iterable[str]) -> tuple[list[str], list[str]]:
    suspected: list[str] = []
    references: list[str] = []
    for candidate in candidates:
        first = candidate.split("/", 1)[0]
        if first in {"http:", "https:", "Users", "home", "tmp", "private", "var"}:
            references.append(candidate)
        else:
            suspected.append(candidate)
    return suspected, references


def extract_test_expectations(text: str) -> list[str]:
    commands = []
    for command in (
        "cargo fmt",
        "cargo test",
        "cargo build",
        "cargo clippy",
        "python3 -m py_compile",
        "python3 -m unittest",
        "bash scripts/",
    ):
        if command in text:
            commands.append(command)
    return commands


def extract_dependency_hints(text: str) -> list[str]:
    lowered = text.lower()
    hints: list[str] = []
    if any(word in lowered for word in ("schema", "contract", "record", "plan yaml")):
        hints.append("contract")
    if any(word in lowered for word in ("storage", "session", "migration", ".commandagent")):
        hints.append("storage")
    if any(word in lowered for word in ("redact", "secret", "token", "api key")):
        hints.append("sanitizer")
    if any(word in lowered for word in ("provider", "client", "openai", "gemini", "ollama")):
        hints.append("provider")
    if any(word in lowered for word in ("codex", "worktree", "commandmate", "orchestrate")):
        hints.append("harness")
    return hints


def direct_dependencies(
    analysis: IssueAnalysis, analyses: Sequence[IssueAnalysis]
) -> list[IssueAnalysis]:
    hints = set(analysis.dependency_hints)
    deps: list[IssueAnalysis] = []
    for other in analyses:
        if other.issue.number == analysis.issue.number:
            continue
        other_hints = set(other.dependency_hints)
        if "storage" in hints and "contract" in other_hints:
            deps.append(other)
        elif "provider" in hints and "contract" in other_hints:
            deps.append(other)
        elif "harness" in hints and "contract" in other_hints:
            deps.append(other)
    return deps


def has_file_overlap(left: IssueAnalysis, right: IssueAnalysis) -> bool:
    return bool(set(left.suspected_files) & set(right.suspected_files))


def classify_issue(analysis: IssueAnalysis, analyses: Sequence[IssueAnalysis]) -> str:
    if direct_dependencies(analysis, analyses):
        return "strong-dependency"
    if any(has_file_overlap(analysis, other) for other in analyses if other != analysis):
        return "weak-conflict"
    return "independent"


def dependency_reason(analysis: IssueAnalysis, analyses: Sequence[IssueAnalysis]) -> str:
    deps = direct_dependencies(analysis, analyses)
    if deps:
        return "depends on " + ", ".join(f"#{item.issue.number}" for item in deps)
    if any(has_file_overlap(analysis, other) for other in analyses if other != analysis):
        return "shared file risk"
    return "no direct dependency detected"


def classify_batches(
    analyses: Sequence[IssueAnalysis], *, max_parallel: int
) -> tuple[list[list[int]], list[int]]:
    remaining = list(analyses)
    completed: set[int] = set()
    batches: list[list[int]] = []
    while remaining:
        ready = [
            item
            for item in remaining
            if all(dep.issue.number in completed for dep in direct_dependencies(item, analyses))
        ]
        if not ready:
            ready = [remaining[0]]

        batch: list[IssueAnalysis] = []
        for item in ready:
            if len(batch) >= max_parallel:
                break
            if any(has_file_overlap(item, existing) for existing in batch):
                continue
            batch.append(item)
        if not batch:
            batch = [ready[0]]

        batches.append([item.issue.number for item in batch])
        completed.update(item.issue.number for item in batch)
        used = {item.issue.number for item in batch}
        remaining = [item for item in remaining if item.issue.number not in used]
    return batches, [number for batch in batches for number in batch]


def make_run_id(explicit: str | None) -> str:
    if explicit:
        return explicit
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def write_run_artifacts(
    run_dir: Path,
    *,
    analyses: Sequence[IssueAnalysis],
    batches: Sequence[Sequence[int]],
    merge_order: Sequence[int],
    repo: str,
    base: str,
    mutating_requested: Sequence[str],
) -> None:
    run_dir.mkdir(parents=True, exist_ok=True)
    (run_dir / "manifest.md").write_text(
        render_manifest(
            analyses,
            batches=batches,
            merge_order=merge_order,
            repo=repo,
            base=base,
            mutating_requested=mutating_requested,
        ),
        encoding="utf-8",
    )
    (run_dir / "issue-analysis.md").write_text(
        render_issue_analysis(analyses),
        encoding="utf-8",
    )
    (run_dir / "dependency-plan.md").write_text(
        render_dependency_plan(analyses, batches=batches, merge_order=merge_order),
        encoding="utf-8",
    )


def render_manifest(
    analyses: Sequence[IssueAnalysis],
    *,
    batches: Sequence[Sequence[int]],
    merge_order: Sequence[int],
    repo: str,
    base: str,
    mutating_requested: Sequence[str],
) -> str:
    lines = [
        "# CommandAgent Codex Orchestration Manifest",
        "",
        f"- Repository: `{repo}`",
        f"- Base: `{base}`",
        "- Mode: dry-run",
        f"- Issues: {', '.join(f'#{item.issue.number}' for item in analyses)}",
        f"- Merge order: {', '.join(f'#{number}' for number in merge_order)}",
        f"- Mutating flags requested: {', '.join(mutating_requested) if mutating_requested else 'none'}",
        "",
        "## Batches",
        "",
    ]
    for index, batch in enumerate(batches, start=1):
        lines.append(f"- Batch {index}: " + ", ".join(f"#{number}" for number in batch))
    lines.extend(["", "## Planned Worktrees", ""])
    for item in analyses:
        lines.append(f"- #{item.issue.number}: `{item.branch_name}` at `{item.worktree_path}`")
    lines.extend(
        [
            "",
            "## Safety",
            "",
            "- This run did not create worktrees, dispatch CommandMate, create PRs, merge PRs, or write UAT fixes.",
            "- Re-run with one explicit mutating flag only after reviewing these artifacts.",
            "",
        ]
    )
    return "\n".join(lines)


def render_issue_analysis(analyses: Sequence[IssueAnalysis]) -> str:
    lines = ["# Issue Analysis", ""]
    for item in analyses:
        lines.extend(
            [
                f"## #{item.issue.number} {item.issue.title}",
                "",
                f"- Objective: {item.objective}",
                f"- Branch: `{item.branch_name}`",
                f"- Worktree: `{item.worktree_path}`",
                f"- Labels: {', '.join(item.issue.labels) if item.issue.labels else 'none'}",
                f"- Dependency hints: {', '.join(item.dependency_hints) if item.dependency_hints else 'none'}",
                "",
                "Acceptance criteria:",
            ]
        )
        lines.extend(list_items(item.acceptance_criteria))
        lines.append("")
        lines.append("Suspected files:")
        lines.extend(list_items(item.suspected_files))
        lines.append("")
        lines.append("Test expectations:")
        lines.extend(list_items(item.test_expectations))
        lines.append("")
        lines.append("Questions:")
        lines.extend(list_items(item.questions))
        lines.append("")
    return "\n".join(lines)


def render_dependency_plan(
    analyses: Sequence[IssueAnalysis],
    *,
    batches: Sequence[Sequence[int]],
    merge_order: Sequence[int],
) -> str:
    lines = ["# Dependency Plan", "", "## Classification", ""]
    for item in analyses:
        lines.append(
            f"- #{item.issue.number}: {classify_issue(item, analyses)} ({dependency_reason(item, analyses)})"
        )
    lines.extend(["", "## Parallel Batches", ""])
    for index, batch in enumerate(batches, start=1):
        lines.append(f"- Batch {index}: " + ", ".join(f"#{number}" for number in batch))
    lines.extend(["", "## Merge Order", ""])
    for index, number in enumerate(merge_order, start=1):
        lines.append(f"{index}. #{number}")
    lines.append("")
    return "\n".join(lines)


def list_items(items: Sequence[str]) -> list[str]:
    if not items:
        return ["- none"]
    return [f"- {item}" for item in items]


def requested_mutations(args: argparse.Namespace) -> list[str]:
    return [name.replace("_", "-") for name in MUTATING_FLAGS if getattr(args, name)]


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    mutations = requested_mutations(args)
    if mutations and not args.dry_run:
        print(
            "error: mutating orchestration phases are not implemented in the CommandAgent port; "
            "run with --dry-run to plan safely",
        )
        return 2

    repo_name = args.repo.split("/")[-1] if args.repo else "CommandAgent"
    issues = load_issues(args.issues, args.issue_json, args.repo)
    analyses = [analyze_issue(issue, repo_name, skip_enhance=args.skip_enhance) for issue in issues]
    batches, merge_order = classify_batches(analyses, max_parallel=max(1, args.max_parallel))
    run_id = make_run_id(args.run_id)
    run_dir = args.runs_dir / run_id
    write_run_artifacts(
        run_dir,
        analyses=analyses,
        batches=batches,
        merge_order=merge_order,
        repo=args.repo,
        base=args.base,
        mutating_requested=mutations,
    )
    print(f"wrote dry-run artifacts to {run_dir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
