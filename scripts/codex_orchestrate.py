#!/usr/bin/env python3
"""Planner and staged runner for Codex issue orchestration.

The default mode is safe planning. Mutating steps such as worktree creation,
CommandMate dispatch, PR creation, and merging are implemented as explicit
phases and remain inspectable through generated run artifacts.
"""

from __future__ import annotations

import argparse
import difflib
import json
import re
import subprocess
import time
import urllib.error
import urllib.request
from dataclasses import dataclass, replace
from datetime import datetime, timezone
from pathlib import Path
from typing import Protocol

REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_RUNS_DIR = REPO_ROOT / "workspace" / "management" / "runs"
DEFAULT_BASE = "origin/develop"
DEFAULT_MERGE_METHOD = "merge"
PHASE_ORDER = {
    "issue": 1,
    "plan": 2,
    "dev": 3,
    "pr": 4,
    "uat": 5,
    "merge": 6,
}


class Runner(Protocol):
    def __call__(
        self,
        args: list[str],
        *,
        cwd: Path | None = None,
        check: bool = False,
        capture_output: bool = True,
        text: bool = True,
    ) -> subprocess.CompletedProcess[str]: ...


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
    enhancement_needed: bool
    questions: tuple[str, ...]
    branch_name: str
    worktree_path: str
    dependency_hints: tuple[str, ...] = ()
    explicit_dependencies: tuple[int, ...] | None = None
    approved_decision: str = ""
    worktree_issue_numbers: tuple[int, ...] = ()


@dataclass(frozen=True)
class WorktreeRow:
    lead_issue_number: int
    issue_numbers: tuple[int, ...]


@dataclass(frozen=True)
class WorktreeResult:
    issue_number: int
    branch_name: str
    worktree_path: Path
    status: str
    message: str


@dataclass(frozen=True)
class WorkerSessionResult:
    issue_number: int
    worktree_id: str
    status: str
    processing: bool | None
    running: bool | None
    message: str
    commands: tuple[str, ...]


@dataclass(frozen=True)
class WorkerWaitResult:
    issue_number: int
    worktree_id: str
    status: str
    message: str


@dataclass(frozen=True)
class SchedulerBatchResult:
    batch_index: int
    issue_numbers: tuple[int, ...]
    status: str
    message: str


@dataclass(frozen=True)
class WorkerVerificationResult:
    issue_number: int
    status: str
    report_path: Path
    message: str


@dataclass(frozen=True)
class PullRequestResult:
    issue_number: int
    branch_name: str
    status: str
    pr_number: int | None
    url: str | None
    message: str


@dataclass(frozen=True)
class MergeResult:
    pr_number: int
    status: str
    message: str
    verification_status: str = "not-run"


@dataclass(frozen=True)
class IssueEnhancementResult:
    issue_number: int
    status: str
    message: str
    diff: str


@dataclass(frozen=True)
class UatFixWorktreeResult:
    issue_number: int
    branch_name: str
    worktree_path: Path
    status: str
    message: str


@dataclass(frozen=True)
class PhotonEventResult:
    event_kind: str
    status: str
    message: str


@dataclass(frozen=True)
class UatFailure:
    issue_number: int
    scenario: str
    expected: str
    actual: str
    evidence: str


@dataclass(frozen=True)
class UatResult:
    issue_number: int
    scenario_index: int
    status: str
    actual: str
    evidence: str
    candidate_head_sha: str = ""


@dataclass(frozen=True)
class UatGateResult:
    status: str
    message: str


@dataclass(frozen=True)
class IssueCloseResult:
    issue_number: int
    pr_number: int
    status: str
    message: str


def slugify(value: str, *, max_len: int = 48) -> str:
    lowered = value.lower()
    normalized = re.sub(r"[^a-z0-9]+", "-", lowered).strip("-")
    compact = re.sub(r"-{2,}", "-", normalized)
    if not compact:
        return "task"
    return compact[:max_len].strip("-") or "task"


def positive_int(value: str) -> int:
    parsed = int(value)
    if parsed < 1:
        raise argparse.ArgumentTypeError("must be at least 1")
    return parsed


def emit_progress(message: str) -> None:
    timestamp = datetime.now(timezone.utc).isoformat(timespec="seconds")
    print(f"[{timestamp}] {message}", flush=True)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("issues", nargs="+", type=int, help="GitHub issue numbers")
    parser.add_argument(
        "--dry-run", action="store_true", help="Only write planning artifacts"
    )
    parser.add_argument("--max-parallel", type=positive_int, default=3)
    parser.add_argument(
        "--dependency-override",
        action="append",
        default=[],
        metavar="ISSUE:DEP,DEP",
        help=(
            "Authoritative dependency entry. When used, specify every planned "
            "worktree row (the lead Issue when --worktree-row is used); use ISSUE: "
            "for a row with no dependencies."
        ),
    )
    parser.add_argument(
        "--issue-decision",
        action="append",
        default=[],
        metavar="ISSUE:TEXT",
        help="Approved Issue decision to add to planning and worker instructions.",
    )
    parser.add_argument(
        "--worktree-row",
        action="append",
        default=[],
        metavar="LEAD:ISSUE,ISSUE",
        help=(
            "Authoritative worktree row. Specify every requested Issue in at least "
            "one row; a member may appear in multiple rows when its implementation "
            "is intentionally split. Row leads must be unique."
        ),
    )
    parser.add_argument(
        "--phase",
        default="plan",
        choices=("issue", "plan", "dev", "pr", "uat", "merge"),
    )
    parser.add_argument("--merge-order", default="")
    parser.add_argument("--skip-enhance", action="store_true")
    parser.add_argument(
        "--issue-json", type=Path, help="Fixture JSON for tests or offline planning"
    )
    parser.add_argument("--run-id", help="Stable run id override")
    parser.add_argument("--runs-dir", type=Path, default=DEFAULT_RUNS_DIR)
    parser.add_argument("--create-worktrees", action="store_true")
    parser.add_argument("--dispatch-commandmate", action="store_true")
    parser.add_argument("--create-prs", action="store_true")
    parser.add_argument("--merge-prs", action="store_true")
    parser.add_argument("--write-uat", action="store_true")
    parser.add_argument("--apply-issue-enhancements", action="store_true")
    parser.add_argument("--create-uat-fix-worktrees", action="store_true")
    parser.add_argument("--poll-commandmate", action="store_true")
    parser.add_argument(
        "--codex-agent-name",
        default="codex",
        help="CommandMate agent/instance name (default: codex)",
    )
    parser.add_argument("--commandmate-duration", default="3h")
    parser.add_argument("--wait-commandmate-timeout", type=int, default=10800)
    parser.add_argument("--wait-commandmate-stall-timeout", type=int, default=0)
    parser.add_argument(
        "--resume-completed-workers",
        action="store_true",
        help=(
            "reuse clean issue worktrees whose committed worker verification "
            "reports already pass"
        ),
    )
    parser.add_argument("--repo", default="")
    parser.add_argument(
        "--integration-check",
        action="append",
        default=[],
        help="Command to run after each merge. Can be specified multiple times.",
    )
    parser.add_argument(
        "--merge-method",
        choices=("merge", "squash", "rebase"),
        default=DEFAULT_MERGE_METHOD,
        help=f"GitHub pull-request merge strategy (default: {DEFAULT_MERGE_METHOD})",
    )
    parser.add_argument(
        "--pr-numbers", default="", help="Comma-separated PR numbers for merge phase"
    )
    parser.add_argument("--uat-results-json", type=Path)
    parser.add_argument(
        "--allow-uat-superset",
        action="store_true",
        help=(
            "Ignore UAT results for Issues outside this invocation; unexpected "
            "scenarios for requested Issues still block the gate."
        ),
    )
    parser.add_argument("--uat-failures-json", type=Path)
    parser.add_argument(
        "--close-issues",
        action="store_true",
        help="Close Issues whose mapped PRs were merged by this invocation.",
    )
    parser.add_argument(
        "--photon-url", default="", help="Optional PHOTON sidecar base URL"
    )
    return parser


def parse_args() -> argparse.Namespace:
    return build_parser().parse_args()


def parse_args_from_list_for_test(values: list[str]) -> argparse.Namespace:
    return build_parser().parse_args(values)


def split_issue_spec(raw: str, *, option: str) -> tuple[int, str]:
    issue_text, separator, value = raw.partition(":")
    if not separator or not issue_text.strip():
        raise ValueError(f"{option} must use ISSUE:VALUE syntax")
    try:
        issue_number = int(issue_text.strip())
    except ValueError as exc:
        raise ValueError(f"{option} Issue must be an integer: {issue_text}") from exc
    return issue_number, value.strip()


def parse_dependency_overrides(
    specs: list[str], requested_issues: list[int]
) -> dict[int, tuple[int, ...]]:
    if not specs:
        return {}
    requested = set(requested_issues)
    if len(requested) != len(requested_issues):
        raise ValueError("requested Issue list contains duplicates")
    overrides: dict[int, tuple[int, ...]] = {}
    for spec in specs:
        issue_number, raw_dependencies = split_issue_spec(
            spec, option="--dependency-override"
        )
        if issue_number in overrides:
            raise ValueError(f"duplicate dependency override for Issue #{issue_number}")
        try:
            dependencies = tuple(
                int(part.strip())
                for part in raw_dependencies.split(",")
                if part.strip()
            )
        except ValueError as exc:
            raise ValueError(
                f"dependency override for Issue #{issue_number} must contain integers"
            ) from exc
        if len(dependencies) != len(set(dependencies)):
            raise ValueError(
                f"dependency override for Issue #{issue_number} contains duplicates"
            )
        if issue_number not in requested:
            raise ValueError(
                f"dependency override targets unrequested Issue #{issue_number}"
            )
        unknown = set(dependencies) - requested
        if unknown:
            formatted = ", ".join(f"#{number}" for number in sorted(unknown))
            raise ValueError(
                f"dependency override for Issue #{issue_number} references "
                f"unrequested Issues {formatted}"
            )
        if issue_number in dependencies:
            raise ValueError(f"Issue #{issue_number} cannot depend on itself")
        overrides[issue_number] = dependencies
    missing = requested - set(overrides)
    if missing:
        formatted = ", ".join(f"#{number}" for number in sorted(missing))
        raise ValueError(
            "dependency overrides are authoritative and must include every requested "
            f"Issue; missing {formatted}"
        )
    return overrides


def parse_issue_decisions(
    specs: list[str], requested_issues: list[int]
) -> dict[int, str]:
    requested = set(requested_issues)
    decisions: dict[int, str] = {}
    for spec in specs:
        issue_number, decision = split_issue_spec(spec, option="--issue-decision")
        if issue_number not in requested:
            raise ValueError(f"decision targets unrequested Issue #{issue_number}")
        if issue_number in decisions:
            raise ValueError(f"duplicate decision for Issue #{issue_number}")
        if not decision:
            raise ValueError(f"decision for Issue #{issue_number} must not be empty")
        decisions[issue_number] = decision
    return decisions


def parse_worktree_rows(
    specs: list[str], requested_issues: list[int]
) -> list[WorktreeRow]:
    if not specs:
        return []
    requested = set(requested_issues)
    if len(requested) != len(requested_issues):
        raise ValueError("requested Issue list contains duplicates")
    rows: list[WorktreeRow] = []
    seen_leads: set[int] = set()
    covered: set[int] = set()
    for spec in specs:
        lead_issue, raw_members = split_issue_spec(spec, option="--worktree-row")
        try:
            members = tuple(
                int(part.strip()) for part in raw_members.split(",") if part.strip()
            )
        except ValueError as exc:
            raise ValueError(
                f"worktree row #{lead_issue} must contain integer Issue numbers"
            ) from exc
        if not members:
            raise ValueError(f"worktree row #{lead_issue} must contain at least one Issue")
        if len(members) != len(set(members)):
            raise ValueError(f"worktree row #{lead_issue} contains duplicate Issues")
        if lead_issue not in members:
            raise ValueError(
                f"worktree row lead #{lead_issue} must be included in its Issue list"
            )
        if lead_issue in seen_leads:
            raise ValueError(f"duplicate worktree row lead Issue #{lead_issue}")
        unknown = set(members) - requested
        if lead_issue not in requested:
            unknown.add(lead_issue)
        if unknown:
            formatted = ", ".join(f"#{number}" for number in sorted(unknown))
            raise ValueError(f"worktree row references unrequested Issues {formatted}")
        rows.append(WorktreeRow(lead_issue, members))
        seen_leads.add(lead_issue)
        covered.update(members)
    missing = requested - covered
    if missing:
        formatted = ", ".join(f"#{number}" for number in sorted(missing))
        raise ValueError(
            "worktree rows are authoritative and must cover every requested Issue; "
            f"missing {formatted}"
        )
    return rows


def load_issues(
    numbers: list[int], fixture_path: Path | None, repo: str = ""
) -> list[Issue]:
    if fixture_path is not None:
        return load_issues_from_fixture(numbers, fixture_path)
    return [fetch_issue_with_gh(number, repo) for number in numbers]


def load_issues_from_fixture(numbers: list[int], fixture_path: Path) -> list[Issue]:
    raw = json.loads(fixture_path.read_text(encoding="utf-8"))
    items = raw["issues"] if isinstance(raw, dict) and "issues" in raw else raw
    if not isinstance(items, list):
        raise TypeError(
            "--issue-json must contain a list or an object with an 'issues' list"
        )

    by_number: dict[int, Issue] = {}
    for item in items:
        if not isinstance(item, dict):
            continue
        number = int(item["number"])
        labels_raw = item.get("labels", [])
        labels = (
            tuple(str(label) for label in labels_raw)
            if isinstance(labels_raw, list)
            else ()
        )
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


def fetch_issue_with_gh(number: int, repo: str = "") -> Issue:
    args = [
        "gh",
        "issue",
        "view",
        str(number),
        "--json",
        "number,title,body,labels",
    ]
    if repo:
        args.extend(["--repo", repo])
    completed = subprocess.run(
        args,
        cwd=REPO_ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    raw = json.loads(completed.stdout)
    labels = tuple(
        label["name"] for label in raw.get("labels", []) if isinstance(label, dict)
    )
    return Issue(
        number=int(raw["number"]),
        title=str(raw.get("title", "")),
        body=str(raw.get("body", "")),
        labels=labels,
    )


def run_command(
    args: list[str],
    *,
    cwd: Path | None = None,
    check: bool = False,
    capture_output: bool = True,
    text: bool = True,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        args,
        cwd=cwd,
        check=check,
        capture_output=capture_output,
        text=text,
    )


def analyze_issue(issue: Issue, repo_name: str, *, skip_enhance: bool) -> IssueAnalysis:
    text = f"{issue.title}\n\n{issue.body}"
    objective = first_nonempty_line(issue.body) or issue.title
    acceptance = extract_acceptance_criteria(issue.body)
    path_candidates = extract_file_candidates(text)
    suspected_files, reference_files = classify_file_candidates(path_candidates)
    suspected_files = enrich_file_candidates_with_rg(text, suspected_files)
    tests = extract_test_expectations(text)
    dependency_hints = extract_dependency_hints(text)
    questions: list[str] = []

    if not acceptance and not skip_enhance:
        questions.append(
            "受入条件が明確ではありません。期待する完了条件を1-3点で補足してください。"
        )
    if not suspected_files and not skip_enhance:
        questions.append(
            "影響範囲を特定できません。想定している機能領域やファイルがあれば教えてください。"
        )

    slug = slugify(issue.title)
    branch = f"feature/issue-{issue.number}-{slug}"
    worktree = f"../{repo_name}-issue-{issue.number}-{slug}"
    return IssueAnalysis(
        issue=issue,
        objective=objective,
        acceptance_criteria=tuple(acceptance),
        suspected_files=tuple(suspected_files),
        reference_files=tuple(reference_files),
        test_expectations=tuple(tests),
        enhancement_needed=bool(questions),
        questions=tuple(questions[:3]),
        branch_name=branch,
        worktree_path=worktree,
        dependency_hints=tuple(dependency_hints),
    )


def unique_items(values: list[str]) -> tuple[str, ...]:
    return tuple(dict.fromkeys(value for value in values if value))


def apply_worktree_rows(
    analyses: list[IssueAnalysis], rows: list[WorktreeRow], repo_name: str
) -> list[IssueAnalysis]:
    if not rows:
        return analyses
    by_issue = {analysis.issue.number: analysis for analysis in analyses}
    grouped: list[IssueAnalysis] = []
    for row in rows:
        members = [by_issue[number] for number in row.issue_numbers]
        lead = by_issue[row.lead_issue_number]
        issue_suffix = "-".join(str(number) for number in row.issue_numbers)
        objective = "; ".join(
            f"#{member.issue.number}: {member.objective}" for member in members
        )
        questions = unique_items(
            [question for member in members for question in member.questions]
        )
        grouped.append(
            replace(
                lead,
                issue=replace(
                    lead.issue,
                    labels=unique_items(
                        [label for member in members for label in member.issue.labels]
                    ),
                ),
                objective=objective,
                acceptance_criteria=unique_items(
                    [
                        criterion
                        for member in members
                        for criterion in member.acceptance_criteria
                    ]
                ),
                suspected_files=unique_items(
                    [path for member in members for path in member.suspected_files]
                ),
                reference_files=unique_items(
                    [path for member in members for path in member.reference_files]
                ),
                test_expectations=unique_items(
                    [command for member in members for command in member.test_expectations]
                ),
                enhancement_needed=bool(questions),
                questions=questions,
                branch_name=f"feature/issue-{issue_suffix}",
                worktree_path=f"../{repo_name}-issue-{issue_suffix}",
                dependency_hints=unique_items(
                    [hint for member in members for hint in member.dependency_hints]
                ),
                explicit_dependencies=None,
                approved_decision="",
                worktree_issue_numbers=row.issue_numbers,
            )
        )
    return grouped


def included_issue_numbers(analysis: IssueAnalysis) -> tuple[int, ...]:
    return analysis.worktree_issue_numbers or (analysis.issue.number,)


def apply_planning_overrides(
    analyses: list[IssueAnalysis],
    dependency_overrides: dict[int, tuple[int, ...]],
    issue_decisions: dict[int, str],
) -> list[IssueAnalysis]:
    updated: list[IssueAnalysis] = []
    for analysis in analyses:
        issue_number = analysis.issue.number
        decision = issue_decisions.get(issue_number, "")
        acceptance = analysis.acceptance_criteria
        suspected_files = list(analysis.suspected_files)
        reference_files = list(analysis.reference_files)
        questions = analysis.questions
        if decision:
            decision_suspected, decision_references = classify_file_candidates(
                extract_file_candidates(decision)
            )
            if analysis.worktree_issue_numbers:
                acceptance = (f"Apply approved row decision: {decision}",)
                if decision_suspected:
                    suspected_files = list(decision_suspected)
                if decision_references:
                    reference_files = list(decision_references)
            else:
                acceptance = (*acceptance, f"Apply approved decision: {decision}")
                suspected_files.extend(
                    path for path in decision_suspected if path not in suspected_files
                )
                reference_files.extend(
                    path for path in decision_references if path not in reference_files
                )
            questions = tuple(
                question
                for question in questions
                if not question.startswith("受入条件が明確ではありません")
                and not (
                    suspected_files and question.startswith("影響範囲を特定できません")
                )
            )
        updated.append(
            replace(
                analysis,
                acceptance_criteria=acceptance,
                suspected_files=tuple(suspected_files),
                reference_files=tuple(reference_files),
                enhancement_needed=bool(questions),
                questions=questions,
                explicit_dependencies=(
                    dependency_overrides[issue_number] if dependency_overrides else None
                ),
                approved_decision=decision,
            )
        )
    return updated


def first_nonempty_line(value: str) -> str:
    for line in value.splitlines():
        if line.lstrip().startswith("#"):
            continue
        stripped = line.strip(" -#\t")
        if stripped:
            return stripped
    return ""


def extract_acceptance_criteria(body: str) -> list[str]:
    lines = body.splitlines()
    out: list[str] = []
    section_level: int | None = None
    heading_re = re.compile(r"^(#{1,6})\s+(.+?)\s*$")
    trigger_re = re.compile(
        r"(acceptance|受入|受け入れ|完了条件|期待結果)", re.IGNORECASE
    )
    list_item_re = re.compile(r"^(?:[-*+]\s+|\d+[.)]\s+)(.+)$")
    checkbox_re = re.compile(r"^\[[ xX]\]\s*")
    for line in lines:
        stripped = line.strip()
        heading = heading_re.match(stripped)
        if heading:
            level = len(heading.group(1))
            if trigger_re.search(heading.group(2)):
                section_level = (
                    level if section_level is None else min(section_level, level)
                )
            elif section_level is not None and level <= section_level:
                section_level = None
            continue
        if section_level is None or not stripped:
            continue
        item_match = list_item_re.match(stripped)
        if item_match:
            item = checkbox_re.sub("", item_match.group(1)).strip()
            if item:
                out.append(item)
    return [item for item in out if item]


def extract_file_candidates(text: str) -> list[str]:
    patterns = [
        r"`([^`\s]+\.(?:py|md|toml|json|yaml|yml|rs|ts|tsx|js|jsx|css|sh))`",
        (
            r"\b((?:src|crates|workspace|scripts|tests|docs|configs|examples|benches|"
            r"\.github|\.agents|\.codex)/[A-Za-z0-9_./-]+)\b"
        ),
        r"\b([A-Za-z0-9_.-]+/(?:[A-Za-z0-9_.-]+/)*[A-Za-z0-9_.-]+\.(?:py|md|toml|json|yaml|yml|rs|ts|tsx|js|jsx|css|sh))\b",
    ]
    seen: set[str] = set()
    out: list[str] = []
    for pattern in patterns:
        for match in re.finditer(pattern, text):
            candidate = match.group(1).strip()
            if match.start(1) > 0 and text[match.start(1) - 1] == "/":
                continue
            if ".." in candidate or candidate.startswith("/"):
                continue
            if candidate.split("/", 1)[0] in {"Users", "home", "tmp", "private", "var"}:
                continue
            if candidate not in seen:
                seen.add(candidate)
                out.append(candidate)
    return out


def classify_file_candidates(candidates: list[str]) -> tuple[list[str], list[str]]:
    suspected: list[str] = []
    references: list[str] = []
    seen_suspected: set[str] = set()
    seen_references: set[str] = set()
    for candidate in candidates:
        if is_external_reference(candidate):
            if candidate not in seen_references:
                references.append(candidate)
                seen_references.add(candidate)
            continue
        if candidate not in seen_suspected:
            suspected.append(candidate)
            seen_suspected.add(candidate)
    return suspected, references


def is_external_reference(candidate: str) -> bool:
    first = candidate.split("/", 1)[0]
    if first in {"Users", "home", "tmp", "private", "var"}:
        return True
    repo_name = commandmate_repository_name()
    if first in {REPO_ROOT.name, repo_name}:
        return True
    if first.lower().startswith("photon-"):
        return True
    return bool(re.search(r"(?:^|-)issue-\d+-", first, re.IGNORECASE))


def repo_path_exists(candidate: str) -> bool:
    return (REPO_ROOT / candidate).exists()


def enrich_file_candidates_with_rg(text: str, existing: list[str]) -> list[str]:
    """Add a few repository paths found by rg without making planning depend on it."""
    candidates = list(existing)
    seen = set(candidates)
    for phrase in extract_search_phrases(text):
        try:
            completed = run_command(
                ["rg", "-l", "--fixed-strings", phrase], cwd=REPO_ROOT
            )
        except FileNotFoundError:
            return candidates
        if completed.returncode not in (0, 1):
            continue
        matched_paths = sorted(
            {
                line.strip()
                for line in completed.stdout.splitlines()
                if line.strip()
                and not is_external_reference(line.strip())
                and not is_planning_noise_path(line.strip())
            }
        )
        for path in matched_paths[:5]:
            if path not in seen:
                candidates.append(path)
                seen.add(path)
            if len(candidates) >= 8:
                return candidates
    return candidates


def is_planning_noise_path(path: str) -> bool:
    if path.startswith(("workspace/management/", "dev-reports/")):
        return True
    relevant_roots = (
        "src/",
        "crates/",
        "tests/",
        "scripts/",
        "docs/",
        ".github/",
        ".agents/",
        ".codex/",
        "Cargo.toml",
        "Cargo.lock",
        "README.md",
        "CHANGELOG.md",
    )
    if not path.startswith(relevant_roots):
        return True
    return path in {
        "scripts/codex_orchestrate.py",
        "tests/test_codex_orchestrate.py",
    }


def extract_search_phrases(text: str) -> list[str]:
    raw = re.findall(r"\b[A-Za-z_][A-Za-z0-9_]{4,}\b", text)
    stop = {
        "Issue",
        "Acceptance",
        "Criteria",
        "schema",
        "version",
        "Implement",
        "概要",
        "対象",
        "完了条件",
    }
    phrases: list[str] = []
    seen: set[str] = set()
    for item in raw:
        if item in stop or item.lower() in {"request", "response", "event", "tests"}:
            continue
        if item not in seen:
            phrases.append(item)
            seen.add(item)
        if len(phrases) >= 4:
            break
    return phrases


def extract_dependency_hints(text: str) -> list[str]:
    hints: list[str] = []
    lowered = text.lower()
    if any(word in lowered for word in ("schema", "contract", "record")):
        hints.append("contract")
    if any(
        word in lowered for word in ("sqlite", "storage", "migration", "local-first")
    ):
        hints.append("storage")
    if any(word in lowered for word in ("sanitizer", "redact", "secret", "token")):
        hints.append("sanitizer")
    if any(word in lowered for word in ("fastapi", "endpoint", "client", "/v1/")):
        hints.append("api")
    return hints


def extract_test_expectations(text: str) -> list[str]:
    commands = []
    for command in (
        "cargo test",
        "cargo clippy",
        "cargo fmt",
        "cargo build",
        "pytest",
        "ruff",
        "mypy",
        "python -m build",
        "npm test",
    ):
        if command in text:
            commands.append(command)
    return commands


def classify_batches(
    analyses: list[IssueAnalysis], merge_order: str, max_parallel: int = 3
) -> tuple[list[list[int]], list[int]]:
    if max_parallel < 1:
        raise ValueError("max_parallel must be at least 1")

    if merge_order:
        order = [int(part.strip()) for part in merge_order.split(",") if part.strip()]
        requested = [analysis.issue.number for analysis in analyses]
        if len(order) != len(set(order)):
            raise ValueError("merge order contains duplicate issue numbers")
        if set(order) != set(requested):
            raise ValueError(
                "merge order must contain every requested issue exactly once"
            )
        completed: set[int] = set()
        by_issue = {analysis.issue.number: analysis for analysis in analyses}
        for number in order:
            dependencies = {
                dependency.issue.number
                for dependency in direct_dependencies(by_issue[number], analyses)
            }
            missing = dependencies - completed
            if missing:
                formatted = ", ".join(f"#{item}" for item in sorted(missing))
                raise ValueError(
                    f"merge order places #{number} before dependencies {formatted}"
                )
            completed.add(number)
        batches = [[number] for number in order]
        return batches, order

    remaining = list(analyses)
    completed: set[int] = set()
    batches: list[list[int]] = []
    while remaining:
        ready = [
            analysis
            for analysis in remaining
            if all(
                dep.issue.number in completed
                for dep in direct_dependencies(analysis, analyses)
            )
        ]
        if not ready:
            unresolved = ", ".join(f"#{item.issue.number}" for item in remaining)
            raise ValueError(
                f"dependency cycle or unresolved dependency among {unresolved}"
            )
        batch: list[IssueAnalysis] = []
        for analysis in ready:
            if len(batch) >= max_parallel:
                break
            if any(has_file_overlap(analysis, existing) for existing in batch):
                continue
            batch.append(analysis)
        if not batch:
            batch = [ready[0]]
        batches.append([item.issue.number for item in batch])
        completed.update(item.issue.number for item in batch)
        batch_numbers = {item.issue.number for item in batch}
        remaining = [
            item for item in remaining if item.issue.number not in batch_numbers
        ]
    order = [number for batch in batches for number in batch]
    return batches, order


def direct_dependencies(
    analysis: IssueAnalysis, analyses: list[IssueAnalysis]
) -> list[IssueAnalysis]:
    if analysis.explicit_dependencies is not None:
        by_issue = {item.issue.number: item for item in analyses}
        return [by_issue[number] for number in analysis.explicit_dependencies]
    hints = set(analysis.dependency_hints)
    dependencies: list[IssueAnalysis] = []
    for other in analyses:
        if other.issue.number == analysis.issue.number:
            continue
        other_hints = set(other.dependency_hints)
        storage_needs_sanitizer = "storage" in hints and "sanitizer" in other_hints
        storage_needs_contract = (
            "storage" in hints
            and "contract" in other_hints
            and "api" not in other_hints
        )
        if (
            storage_needs_sanitizer
            or storage_needs_contract
            or ("api" in hints and bool({"contract", "storage"} & other_hints))
        ):
            dependencies.append(other)
    return dependencies


def dependency_reason(analysis: IssueAnalysis, analyses: list[IssueAnalysis]) -> str:
    deps = direct_dependencies(analysis, analyses)
    if deps:
        prefix = (
            "explicitly depends on"
            if analysis.explicit_dependencies is not None
            else "depends on"
        )
        return prefix + " " + ", ".join(f"#{item.issue.number}" for item in deps)
    if analysis.explicit_dependencies is not None:
        return "explicitly has no dependencies"
    if any(
        has_file_overlap(analysis, other) for other in analyses if other != analysis
    ):
        return "shared implementation file risk"
    return "no direct dependency detected"


def classify_issue(analysis: IssueAnalysis, analyses: list[IssueAnalysis]) -> str:
    if direct_dependencies(analysis, analyses):
        return "strong-dependency"
    if any(
        has_file_overlap(analysis, other) for other in analyses if other != analysis
    ):
        return "weak-conflict"
    return "independent"


def merge_order_from_batches(batches: list[list[int]]) -> list[int]:
    return [number for batch in batches for number in batch]


def legacy_classify_batches(
    analyses: list[IssueAnalysis], merge_order: str
) -> tuple[list[list[int]], list[int]]:
    if merge_order:
        order = [int(part.strip()) for part in merge_order.split(",") if part.strip()]
    else:
        order = [analysis.issue.number for analysis in analyses]

    batches: list[list[int]] = []
    current: list[IssueAnalysis] = []
    for analysis in analyses:
        if any(has_file_overlap(analysis, existing) for existing in current):
            if current:
                batches.append([item.issue.number for item in current])
            current = [analysis]
        else:
            current.append(analysis)
    if current:
        batches.append([item.issue.number for item in current])
    return batches, order


def has_file_overlap(left: IssueAnalysis, right: IssueAnalysis) -> bool:
    if {"contract"} & set(left.dependency_hints) and {"sanitizer"} & set(
        right.dependency_hints
    ):
        return False
    if {"sanitizer"} & set(left.dependency_hints) and {"contract"} & set(
        right.dependency_hints
    ):
        return False
    left_files = {path for path in left.suspected_files if is_implementation_path(path)}
    right_files = {
        path for path in right.suspected_files if is_implementation_path(path)
    }
    return bool(left_files & right_files)


def is_implementation_path(path: str) -> bool:
    return not path.startswith(("workspace/", "README", "docs/"))


def current_branch() -> str:
    return run_git(["branch", "--show-current"]) or "unknown"


def current_commit() -> str:
    return run_git(["rev-parse", "--short", "HEAD"]) or "unknown"


def run_git(args: list[str]) -> str:
    completed = run_command(["git", *args], cwd=REPO_ROOT)
    return completed.stdout.strip()


def render_manifest(
    *,
    run_id: str,
    created_at: str,
    issues: list[int],
    phase: str,
    max_parallel: int,
    dry_run: bool,
    codex_agent_name: str,
    dependency_overrides: dict[int, tuple[int, ...]],
    issue_decisions: dict[int, str],
    worktree_rows: dict[int, tuple[int, ...]],
) -> str:
    dependency_summary = "; ".join(
        f"#{issue}<-{','.join(f'#{dependency}' for dependency in dependencies) or 'none'}"
        for issue, dependencies in dependency_overrides.items()
    )
    decision_summary = "; ".join(
        f"#{issue}: {decision}" for issue, decision in issue_decisions.items()
    )
    row_summary = "; ".join(
        f"#{lead}=[{','.join(f'#{number}' for number in members)}]"
        for lead, members in worktree_rows.items()
    )
    return "\n".join(
        [
            "# Orchestration Manifest",
            "",
            f"- Run ID: `{run_id}`",
            f"- Created at: `{created_at}`",
            f"- Repository: `{commandmate_repository_name()}`",
            f"- Start branch: `{current_branch()}`",
            f"- Start commit: `{current_commit()}`",
            f"- Requested issues: `{', '.join(str(issue) for issue in issues)}`",
            f"- Phase: `{phase}`",
            f"- Max parallel: `{max_parallel}`",
            f"- Dry run: `{str(dry_run).lower()}`",
            f"- Develop base: `{DEFAULT_BASE}`",
            f"- CommandMate Codex agent: `{codex_agent_name}`",
            f"- Dependency source: `{'explicit' if dependency_overrides else 'inferred'}`",
            f"- Dependency overrides: {dependency_summary or 'none'}",
            f"- Approved decisions: {decision_summary or 'none'}",
            f"- Worktree rows: {row_summary or 'one Issue per worktree'}",
            "",
            "## Generated Artifacts",
            "",
            "- `issue-analysis.md`",
            "- `dependency-plan.md`",
            "- `scheduler-report.md`",
            "",
            "## User Questions",
            "",
            "See `issue-analysis.md`.",
            "",
        ]
    )


def render_issue_analysis(analyses: list[IssueAnalysis]) -> str:
    lines = ["# Issue Analysis", ""]
    for analysis in analyses:
        issue = analysis.issue
        included = included_issue_numbers(analysis)
        heading = (
            f"Worktree row #{issue.number} "
            f"(Issues {', '.join(f'#{number}' for number in included)}): {issue.title}"
            if analysis.worktree_issue_numbers
            else f"Issue #{issue.number}: {issue.title}"
        )
        lines.extend(
            [
                f"## {heading}",
                "",
                f"- 種別: `{', '.join(issue.labels) if issue.labels else 'unknown'}`",
                f"- 目的: {analysis.objective}",
                f"- 詳細化要否: `{'yes' if analysis.enhancement_needed else 'no'}`",
                "",
                "### 受入条件",
                "",
                *bullet_or_none(analysis.acceptance_criteria),
                "",
                "### 承認済み判断",
                "",
                *bullet_or_none(
                    (analysis.approved_decision,) if analysis.approved_decision else ()
                ),
                "",
                "### 推定影響ファイル",
                "",
                *bullet_or_none(analysis.suspected_files),
                "",
                "### 参考情報",
                "",
                *bullet_or_none(analysis.reference_files),
                "",
                "### テスト期待値",
                "",
                *bullet_or_none(analysis.test_expectations),
                "",
                "### ユーザーへの質問",
                "",
                *bullet_or_none(analysis.questions),
                "",
                "### GitHub Issue 反映候補",
                "",
                "詳細化要否が `yes` の場合、ユーザー回答後に反映する。",
                "",
            ]
        )
    return "\n".join(lines)


def build_issue_body_with_orchestration_notes(analysis: IssueAnalysis) -> str:
    marker = "<!-- codex-orchestrate-notes -->"
    end_marker = "<!-- /codex-orchestrate-notes -->"
    notes = "\n".join(
        [
            marker,
            "## Orchestration Notes",
            "",
            f"- Objective: {analysis.objective}",
            "- Acceptance criteria:",
            *[f"  - {item}" for item in analysis.acceptance_criteria],
            "- Suspected files:",
            *[f"  - {item}" for item in analysis.suspected_files],
            "- References:",
            *[f"  - {item}" for item in analysis.reference_files],
            "- Test expectations:",
            *[f"  - {item}" for item in analysis.test_expectations],
            end_marker,
            "",
        ]
    )
    body = analysis.issue.body.rstrip()
    pattern = re.compile(
        rf"\n*{re.escape(marker)}.*?{re.escape(end_marker)}\n*",
        re.DOTALL,
    )
    if pattern.search(body):
        return pattern.sub(f"\n\n{notes}", body).rstrip() + "\n"
    return f"{body}\n\n{notes}" if body else notes


def apply_issue_enhancements(
    analyses: list[IssueAnalysis],
    *,
    dry_run: bool,
    runner: Runner = run_command,
) -> list[IssueEnhancementResult]:
    results: list[IssueEnhancementResult] = []
    for analysis in analyses:
        new_body = build_issue_body_with_orchestration_notes(analysis)
        if new_body == analysis.issue.body:
            results.append(
                IssueEnhancementResult(
                    analysis.issue.number,
                    "unchanged",
                    "Issue body already contains current orchestration notes",
                    "",
                )
            )
            continue
        diff = "\n".join(
            difflib.unified_diff(
                analysis.issue.body.splitlines(),
                new_body.splitlines(),
                fromfile=f"issue-{analysis.issue.number}-before.md",
                tofile=f"issue-{analysis.issue.number}-after.md",
                lineterm="",
            )
        )
        if dry_run:
            results.append(
                IssueEnhancementResult(
                    analysis.issue.number,
                    "planned",
                    "dry-run: GitHub Issue update skipped",
                    diff,
                )
            )
            continue
        runner(
            ["gh", "issue", "edit", str(analysis.issue.number), "--body", new_body],
            cwd=REPO_ROOT,
            check=True,
        )
        results.append(
            IssueEnhancementResult(
                analysis.issue.number,
                "updated",
                "GitHub Issue body updated",
                diff,
            )
        )
    return results


def render_issue_enhancement_report(results: list[IssueEnhancementResult]) -> str:
    lines = ["# Issue Enhancement Report", ""]
    if not results:
        return "# Issue Enhancement Report\n\nNot requested.\n"
    for result in results:
        lines.extend(
            [
                f"## Issue #{result.issue_number}",
                "",
                f"- Status: `{result.status}`",
                f"- Message: {result.message}",
                "",
            ]
        )
        if result.diff:
            lines.extend(["```diff", result.diff, "```", ""])
    return "\n".join(lines)


def render_dependency_plan(
    analyses: list[IssueAnalysis],
    batches: list[list[int]],
    merge_order: list[int],
) -> str:
    lines = [
        "# Dependency Plan",
        "",
        "## Parallel Batches",
        "",
    ]
    for index, batch in enumerate(batches, start=1):
        lines.append(f"- Batch {index}: {', '.join(f'#{number}' for number in batch)}")
    lines.extend(["", "## Merge Order", ""])
    lines.append(", ".join(f"#{number}" for number in merge_order))
    lines.extend(["", "## Issue Plans", ""])
    for analysis in analyses:
        classification = classify_issue(analysis, analyses)
        included = included_issue_numbers(analysis)
        lines.extend(
            [
                f"### Worktree row #{analysis.issue.number}",
                "",
                f"- Issues: {', '.join(f'#{number}' for number in included)}",
                f"- Classification: `{classification}`",
                f"- Dependency reason: {dependency_reason(analysis, analyses)}",
                f"- Dependency source: `{'explicit' if analysis.explicit_dependencies is not None else 'inferred'}`",
                f"- Approved decision: {analysis.approved_decision or 'none'}",
                f"- Branch: `{analysis.branch_name}`",
                f"- Worktree: `{analysis.worktree_path}`",
                f"- Suspected files: `{', '.join(analysis.suspected_files) or 'unknown'}`",
                f"- References: `{', '.join(analysis.reference_files) or 'none'}`",
                "",
            ]
        )
    lines.extend(["## Blocked Items", "", "None at dry-run planning time.", ""])
    return "\n".join(lines)


def bullet_or_none(items: tuple[str, ...]) -> list[str]:
    if not items:
        return ["- None"]
    return [f"- {item}" for item in items]


def write_artifacts(args: argparse.Namespace, analyses: list[IssueAnalysis]) -> Path:
    created_at = datetime.now(timezone.utc).replace(microsecond=0).isoformat()
    run_id = (
        args.run_id
        or f"{datetime.now(timezone.utc).strftime('%Y%m%d-%H%M%S')}-orchestrate"
    )
    run_dir = args.runs_dir / run_id
    run_dir.mkdir(parents=True, exist_ok=False)

    batches, merge_order = classify_batches(
        analyses, args.merge_order, max_parallel=args.max_parallel
    )
    (run_dir / "manifest.md").write_text(
        render_manifest(
            run_id=run_id,
            created_at=created_at,
            issues=args.issues,
            phase=args.phase,
            max_parallel=args.max_parallel,
            dry_run=args.dry_run,
            codex_agent_name=args.codex_agent_name,
            dependency_overrides={
                analysis.issue.number: analysis.explicit_dependencies or ()
                for analysis in analyses
                if analysis.explicit_dependencies is not None
            },
            issue_decisions={
                analysis.issue.number: analysis.approved_decision
                for analysis in analyses
                if analysis.approved_decision
            },
            worktree_rows={
                analysis.issue.number: included_issue_numbers(analysis)
                for analysis in analyses
                if analysis.worktree_issue_numbers
            },
        ),
        encoding="utf-8",
    )
    (run_dir / "issue-analysis.md").write_text(
        render_issue_analysis(analyses), encoding="utf-8"
    )
    (run_dir / "dependency-plan.md").write_text(
        render_dependency_plan(analyses, batches, merge_order),
        encoding="utf-8",
    )
    (run_dir / "issue-enhancement-report.md").write_text(
        "# Issue Enhancement Report\n\nNot requested.\n", encoding="utf-8"
    )
    (run_dir / "worker-sessions.md").write_text(
        "# Worker Sessions\n\nNot started.\n", encoding="utf-8"
    )
    (run_dir / "worker-verification.md").write_text(
        "# Worker Verification\n\nNot started.\n", encoding="utf-8"
    )
    (run_dir / "scheduler-report.md").write_text(
        "# Scheduler Report\n\nNot started.\n", encoding="utf-8"
    )
    (run_dir / "merge-report.md").write_text(
        "# Merge Report\n\nNot started.\n", encoding="utf-8"
    )
    (run_dir / "ci-report.md").write_text(
        "# CI Report\n\nNot started.\n", encoding="utf-8"
    )
    (run_dir / "uat-report.md").write_text(
        "# UAT Report\n\nNot started.\n", encoding="utf-8"
    )
    (run_dir / "uat-fix-worktrees.md").write_text(
        "# UAT Fix Worktrees\n\nNot requested.\n", encoding="utf-8"
    )
    (run_dir / "photon-events.md").write_text(
        "# PHOTON Events\n\nNot configured.\n", encoding="utf-8"
    )
    (run_dir / "final-report.md").write_text(
        "# Final Report\n\nNot completed.\n", encoding="utf-8"
    )
    return run_dir


def phase_at_least(current: str, target: str) -> bool:
    return PHASE_ORDER[current] >= PHASE_ORDER[target]


def resolve_worktree_path(raw_path: str) -> Path:
    path = Path(raw_path)
    if path.is_absolute():
        return path
    return (REPO_ROOT / path).resolve()


def branch_exists(branch_name: str, runner: Runner = run_command) -> bool:
    completed = runner(
        ["git", "show-ref", "--verify", "--quiet", f"refs/heads/{branch_name}"],
        cwd=REPO_ROOT,
    )
    return completed.returncode == 0


def worktree_is_dirty(path: Path, runner: Runner = run_command) -> bool:
    completed = runner(["git", "status", "--porcelain"], cwd=path)
    return bool(completed.stdout.strip())


def create_or_reuse_worktrees(
    analyses: list[IssueAnalysis],
    *,
    dry_run: bool,
    runner: Runner = run_command,
) -> list[WorktreeResult]:
    results: list[WorktreeResult] = []
    if dry_run:
        return [
            WorktreeResult(
                issue_number=analysis.issue.number,
                branch_name=analysis.branch_name,
                worktree_path=resolve_worktree_path(analysis.worktree_path),
                status="planned",
                message="dry-run: worktree creation skipped",
            )
            for analysis in analyses
        ]

    runner(["git", "fetch", "origin", "develop"], cwd=REPO_ROOT, check=True)
    for analysis in analyses:
        path = resolve_worktree_path(analysis.worktree_path)
        if path.exists():
            if worktree_is_dirty(path, runner):
                results.append(
                    WorktreeResult(
                        issue_number=analysis.issue.number,
                        branch_name=analysis.branch_name,
                        worktree_path=path,
                        status="blocked",
                        message="existing worktree has uncommitted changes",
                    )
                )
            else:
                results.append(
                    WorktreeResult(
                        issue_number=analysis.issue.number,
                        branch_name=analysis.branch_name,
                        worktree_path=path,
                        status="reused",
                        message="existing clean worktree reused",
                    )
                )
            continue

        if branch_exists(analysis.branch_name, runner):
            cmd = ["git", "worktree", "add", str(path), analysis.branch_name]
        else:
            cmd = [
                "git",
                "worktree",
                "add",
                "-b",
                analysis.branch_name,
                str(path),
                DEFAULT_BASE,
            ]
        runner(cmd, cwd=REPO_ROOT, check=True)
        results.append(
            WorktreeResult(
                issue_number=analysis.issue.number,
                branch_name=analysis.branch_name,
                worktree_path=path,
                status="created",
                message="worktree created",
            )
        )
    return results


def render_worker_sessions(
    results: list[WorktreeResult], dispatch_results: list[WorkerSessionResult]
) -> str:
    lines = ["# Worker Sessions", ""]
    for result in results:
        session = next(
            (
                item
                for item in dispatch_results
                if item.issue_number == result.issue_number
            ),
            None,
        )
        lines.extend(
            [
                f"## Issue #{result.issue_number}",
                "",
                f"- Branch: `{result.branch_name}`",
                f"- Worktree: `{result.worktree_path}`",
                f"- Status: `{result.status}`",
                f"- Message: {result.message}",
                f"- Worker status: `{session.status if session else 'not-dispatched'}`",
                f"- Running: `{session.running if session else 'unknown'}`",
                f"- Processing: `{session.processing if session else 'unknown'}`",
                f"- Worker message: {session.message if session else 'not dispatched'}",
                "",
            ]
        )
    commands = [command for result in dispatch_results for command in result.commands]
    if commands:
        lines.extend(
            ["## CommandMate Dispatch", "", *[f"- `{line}`" for line in commands], ""]
        )
    return "\n".join(lines)


def build_worker_prompt(
    analysis: IssueAnalysis, dependencies: list[IssueAnalysis] | None = None
) -> str:
    criteria = (
        "\n".join(f"- {item}" for item in analysis.acceptance_criteria) or "- 未整理"
    )
    suspected = (
        "\n".join(f"- {item}" for item in analysis.suspected_files) or "- 未特定"
    )
    references = "\n".join(f"- {item}" for item in analysis.reference_files) or "- なし"
    dependency_lines = (
        "\n".join(
            f"- Issue #{item.issue.number}: branch `{item.branch_name}`, "
            f"worktree `{item.worktree_path}`"
            for item in dependencies or []
        )
        or "- None"
    )
    approved_decision = analysis.approved_decision or "None"
    included = included_issue_numbers(analysis)
    task_label = (
        f"worktree row #{analysis.issue.number} for Issues "
        + ", ".join(f"#{number}" for number in included)
        if analysis.worktree_issue_numbers
        else f"Issue #{analysis.issue.number}"
    )
    return "\n".join(
        [
            f"Codex issue worker task for {task_label}",
            "",
            "If `$codex-issue-worker` is available in this worktree, follow that skill.",
            "If it is not available, treat this message as the full worker instruction.",
            "",
            "## Required Workflow",
            "",
            "1. Read the Issue summary, acceptance criteria, approved decision, suspected files, and references.",
            "2. Write a short design note before editing.",
            "3. Implement the smallest coherent change that satisfies the Issue.",
            "4. Add or update focused tests where appropriate.",
            "5. Run focused verification, and broader checks if shared contracts are touched.",
            (
                "6. Write `dev-reports/issue-<number>/design.md`, "
                "`implementation-summary.md`, and `verification.md`."
            ),
            (
                '7. In `verification.md`, record the exact line "- Status: `passed`" '
                "only when every required check passed, followed by one "
                '"- `<command>`: `passed`" entry per check. Use `blocked` when any '
                "required check fails or cannot run."
            ),
            "8. Commit the work with a clear Issue-scoped commit message.",
            "9. Report blockers only if implementation cannot safely proceed.",
            "",
            "## Issue Summary",
            "",
            f"- Included Issues: {', '.join(f'#{number}' for number in included)}",
            f"- Title: {analysis.issue.title}",
            f"- Objective: {analysis.objective}",
            "",
            "## Acceptance Criteria",
            "",
            criteria,
            "",
            "## Approved Decision",
            "",
            approved_decision,
            (
                "The approved decision is authoritative when it narrows or "
                "contradicts the original Issue narrative or inferred file scope."
            ),
            "",
            "## Suspected Files",
            "",
            suspected,
            "",
            "## References",
            "",
            references,
            "",
            "## Required Predecessors",
            "",
            dependency_lines,
            "",
            (
                "The scheduler dispatches this Issue only after every listed dependency "
                "or file-conflict predecessor completed and passed verification. Inspect "
                "their committed changes before editing; do not assume those branches "
                "are already merged into this one."
            ),
            "",
            "## Orchestration Notes",
            "",
            f"- Branch: {analysis.branch_name}",
            f"- Worktree: {analysis.worktree_path}",
            "- Keep review lightweight and ask only blocking questions.",
        ]
    )


def build_commandmate_send_command(
    worktree_id: str,
    prompt: str,
    *,
    duration: str,
    codex_agent_name: str,
) -> list[str]:
    cmd = ["commandmatedev", "send", worktree_id, prompt]
    if codex_agent_name:
        cmd.extend(["--agent", codex_agent_name])
    cmd.extend(["--auto-yes", "--duration", duration])
    return cmd


def build_commandmate_ls_command(
    *, branch_prefix: str | None = None, json_output: bool = True
) -> list[str]:
    cmd = ["commandmatedev", "ls"]
    if branch_prefix:
        cmd.extend(["--branch", branch_prefix])
    if json_output:
        cmd.append("--json")
    return cmd


def commandmate_worktree_id(branch_name: str) -> str:
    branch_leaf = branch_name.rsplit("/", 1)[-1]
    return f"{commandmate_repository_name()}-{branch_leaf}".lower()


def commandmate_repository_name() -> str:
    name = REPO_ROOT.name
    match = re.match(r"(.+)-issue-\d+-", name)
    if match:
        return match.group(1)
    if name.endswith("-develop"):
        base = name.removesuffix("-develop")
        return base or name
    return name


def dispatch_commandmate(
    analyses: list[IssueAnalysis],
    worktree_results: list[WorktreeResult],
    *,
    dry_run: bool,
    duration: str,
    codex_agent_name: str,
    poll: bool = False,
    dependency_analyses: list[IssueAnalysis] | None = None,
    dependency_context: dict[int, list[IssueAnalysis]] | None = None,
    runner: Runner = run_command,
) -> list[WorkerSessionResult]:
    results: list[WorkerSessionResult] = []
    by_issue = {result.issue_number: result for result in worktree_results}
    for analysis in analyses:
        result = by_issue.get(analysis.issue.number)
        if result is None or result.status == "blocked":
            continue
        worktree_id = commandmate_worktree_id(result.branch_name)
        dependencies = (
            dependency_context.get(analysis.issue.number, [])
            if dependency_context is not None
            else direct_dependencies(analysis, dependency_analyses or analyses)
        )
        task = build_commandmate_send_command(
            worktree_id,
            build_worker_prompt(analysis, dependencies),
            duration=duration,
            codex_agent_name=codex_agent_name,
        )
        commands = (" ".join(task),)
        if not dry_run:
            try:
                runner(task, cwd=REPO_ROOT, check=True)
            except subprocess.CalledProcessError as exc:
                message = str(exc)
                if exc.stderr:
                    message = exc.stderr.strip()
                elif exc.stdout:
                    message = exc.stdout.strip()
                results.append(
                    WorkerSessionResult(
                        issue_number=analysis.issue.number,
                        worktree_id=worktree_id,
                        status="blocked",
                        processing=None,
                        running=None,
                        message=message,
                        commands=commands,
                    )
                )
                continue
        status = WorkerSessionResult(
            issue_number=analysis.issue.number,
            worktree_id=worktree_id,
            status="planned" if dry_run else "sent",
            processing=None,
            running=None,
            message="dry-run: CommandMate dispatch skipped" if dry_run else "task sent",
            commands=commands,
        )
        if not dry_run and poll:
            status = poll_worker_startup(
                analysis.issue.number,
                worktree_id,
                codex_agent_name=codex_agent_name,
                commands=commands,
                runner=runner,
            )
        results.append(status)
    return results


def poll_worker_startup(
    issue_number: int,
    worktree_id: str,
    *,
    codex_agent_name: str,
    commands: tuple[str, ...],
    runner: Runner = run_command,
) -> WorkerSessionResult:
    state = get_commandmate_state(
        worktree_id, codex_agent_name=codex_agent_name, runner=runner
    )
    if state["processing"] is True:
        return WorkerSessionResult(
            issue_number,
            worktree_id,
            "processing",
            True,
            state["running"],
            "worker is processing",
            commands,
        )
    if state["running"] is True and state["processing"] is False:
        return WorkerSessionResult(
            issue_number,
            worktree_id,
            "started-but-idle",
            False,
            True,
            "worker session is running but not processing",
            commands,
        )
    if state["found"] is False:
        message = str(
            state.get("message") or "worktree session not found in CommandMate"
        )
    else:
        message = "worker did not enter processing state"
    return WorkerSessionResult(
        issue_number,
        worktree_id,
        "blocked",
        state["processing"],
        state["running"],
        message,
        commands,
    )


def get_commandmate_state(
    worktree_id: str, *, codex_agent_name: str, runner: Runner = run_command
) -> dict[str, object]:
    completed = runner(build_commandmate_ls_command(), cwd=REPO_ROOT)
    if completed.returncode != 0:
        return {
            "found": False,
            "running": None,
            "processing": None,
            "status": "unreachable",
            "message": classify_commandmate_failure(completed),
        }
    if not completed.stdout.strip():
        return {
            "found": False,
            "running": None,
            "processing": None,
            "status": "empty-response",
            "message": "commandmatedev ls returned no output",
        }
    try:
        raw = json.loads(completed.stdout)
    except json.JSONDecodeError:
        return {
            "found": False,
            "running": None,
            "processing": None,
            "status": "invalid-json",
            "message": "commandmatedev ls returned invalid JSON",
        }
    items = raw if isinstance(raw, list) else raw.get("worktrees", [])
    for item in items:
        if not isinstance(item, dict):
            continue
        item_id = str(item.get("id") or item.get("name") or "")
        if item_id != worktree_id:
            continue
        instance_status = item.get("sessionStatusByInstance", {})
        session_status = item.get("sessionStatusByCli", {})
        cli_status = {}
        if isinstance(instance_status, dict) and codex_agent_name:
            cli_status = instance_status.get(codex_agent_name) or {}
        if not cli_status and isinstance(session_status, dict):
            cli_status = (
                (
                    session_status.get(codex_agent_name)
                    if codex_agent_name
                    else session_status.get("codex")
                )
                or session_status.get("default")
                or next(iter(session_status.values()), {})
            )
        running = cli_status.get("isRunning") if isinstance(cli_status, dict) else None
        if running is None:
            running = bool(
                item.get("isSessionRunning")
                or item.get("isRunning")
                or str(item.get("status") or item.get("state") or "").lower()
                in {"running", "ready"}
            )
        processing = (
            cli_status.get("isProcessing") if isinstance(cli_status, dict) else None
        )
        if processing is None:
            processing = item.get("isProcessing")
        return {
            "found": True,
            "running": bool(running),
            "processing": bool(processing) if processing is not None else None,
            "status": "found",
            "message": "worktree session found",
        }
    return {
        "found": False,
        "running": None,
        "processing": None,
        "status": "not-found",
        "message": "worktree session not found in CommandMate",
    }


def classify_commandmate_failure(completed: subprocess.CompletedProcess[str]) -> str:
    output = "\n".join(
        part for part in (completed.stderr, completed.stdout) if part
    ).strip()
    lowered = output.lower()
    if (
        "server is not running" in lowered
        or "fetch failed" in lowered
        or "econnrefused" in lowered
        or "couldn't connect to server" in lowered
    ):
        return (
            "commandmate-unreachable: local CommandMate HTTP endpoint could not be reached. "
            "This can be a sandbox localhost access issue; do not infer server shutdown or start "
            "CommandMate automatically."
        )
    return output or "commandmate command failed"


def wait_for_commandmate_workers(
    results: list[WorkerSessionResult],
    *,
    timeout_seconds: int,
    stall_timeout_seconds: int,
    codex_agent_name: str,
    runner: Runner = run_command,
) -> list[WorkerWaitResult]:
    wait_results: list[WorkerWaitResult] = []
    for result in results:
        if result.status not in {"sent", "processing", "started-but-idle"}:
            continue
        cmd = [
            "commandmatedev",
            "wait",
            result.worktree_id,
            "--timeout",
            str(timeout_seconds),
        ]
        if codex_agent_name:
            cmd.extend(["--instance", codex_agent_name])
        if stall_timeout_seconds > 0:
            cmd.extend(["--stall-timeout", str(stall_timeout_seconds)])
        completed = runner(cmd, cwd=REPO_ROOT)
        if completed.returncode == 0:
            wait_results.append(
                WorkerWaitResult(
                    result.issue_number,
                    result.worktree_id,
                    "completed",
                    completed.stdout.strip() or "worker completed",
                )
            )
        else:
            wait_results.append(
                WorkerWaitResult(
                    result.issue_number,
                    result.worktree_id,
                    "blocked",
                    classify_commandmate_failure(completed),
                )
            )
    return wait_results


def schedule_commandmate_batches(
    analyses: list[IssueAnalysis],
    worktree_results: list[WorktreeResult],
    batches: list[list[int]],
    *,
    max_parallel: int,
    dry_run: bool,
    dispatch_enabled: bool,
    duration: str,
    codex_agent_name: str,
    poll: bool,
    timeout_seconds: int,
    stall_timeout_seconds: int,
    resume_completed: bool = False,
    runner: Runner = run_command,
) -> tuple[
    list[WorkerSessionResult],
    list[WorkerWaitResult],
    list[SchedulerBatchResult],
]:
    if max_parallel < 1:
        raise ValueError("max_parallel must be at least 1")
    requested = [analysis.issue.number for analysis in analyses]
    scheduled = [number for batch in batches for number in batch]
    if len(scheduled) != len(set(scheduled)) or set(scheduled) != set(requested):
        raise ValueError(
            "scheduler batches must contain every requested issue exactly once"
        )
    if any(len(batch) > max_parallel for batch in batches):
        raise ValueError("scheduler batch exceeds max_parallel")

    by_issue = {analysis.issue.number: analysis for analysis in analyses}
    dispatch_results: list[WorkerSessionResult] = []
    wait_results: list[WorkerWaitResult] = []
    batch_results: list[SchedulerBatchResult] = []
    blocked_by: str | None = None
    completed_issues: set[int] = set()

    for batch_index, issue_numbers in enumerate(batches, start=1):
        batch_analyses = [by_issue[number] for number in issue_numbers]
        if blocked_by is not None:
            for analysis in batch_analyses:
                dispatch_results.append(
                    WorkerSessionResult(
                        analysis.issue.number,
                        commandmate_worktree_id(analysis.branch_name),
                        "blocked",
                        None,
                        None,
                        f"not dispatched because {blocked_by}",
                        (),
                    )
                )
            batch_results.append(
                SchedulerBatchResult(
                    batch_index,
                    tuple(issue_numbers),
                    "blocked",
                    f"not dispatched because {blocked_by}",
                )
            )
            continue

        verified_analyses: list[IssueAnalysis] = []
        if resume_completed and not dry_run:
            existing_verification = verify_worker_reports(
                batch_analyses,
                worktree_results,
                dry_run=False,
                runner=runner,
            )
            verified_numbers = {
                result.issue_number
                for result in existing_verification
                if result.status == "passed"
            }
            verified_analyses = [
                analysis
                for analysis in batch_analyses
                if analysis.issue.number in verified_numbers
            ]
        pending_analyses = [
            analysis for analysis in batch_analyses if analysis not in verified_analyses
        ]
        processing_analyses: list[IssueAnalysis] = []
        if resume_completed and not dry_run:
            processing_analyses = [
                analysis
                for analysis in pending_analyses
                if get_commandmate_state(
                    commandmate_worktree_id(analysis.branch_name),
                    codex_agent_name=codex_agent_name,
                    runner=runner,
                )["processing"]
                is True
            ]
            pending_analyses = [
                analysis
                for analysis in pending_analyses
                if analysis not in processing_analyses
            ]
        verified_dispatch = [
            WorkerSessionResult(
                analysis.issue.number,
                commandmate_worktree_id(analysis.branch_name),
                "verified-complete",
                False,
                None,
                "clean committed worker verification already passed",
                (),
            )
            for analysis in verified_analyses
        ]
        processing_dispatch = [
            WorkerSessionResult(
                analysis.issue.number,
                commandmate_worktree_id(analysis.branch_name),
                "processing",
                True,
                True,
                "resumed existing Codex worker without redispatch",
                (),
            )
            for analysis in processing_analyses
        ]
        if not pending_analyses and not processing_analyses:
            dispatch_results.extend(verified_dispatch)
            batch_results.append(
                SchedulerBatchResult(
                    batch_index,
                    tuple(issue_numbers),
                    "completed",
                    "existing committed worker reports passed verification",
                )
            )
            completed_issues.update(issue_numbers)
            continue

        dependency_context = {
            analysis.issue.number: [
                candidate
                for candidate in analyses
                if candidate.issue.number in completed_issues
                and (
                    candidate in direct_dependencies(analysis, analyses)
                    or has_file_overlap(analysis, candidate)
                )
            ]
            for analysis in pending_analyses
        }
        batch_dispatch = dispatch_commandmate(
            pending_analyses,
            worktree_results,
            dry_run=dry_run or not dispatch_enabled,
            duration=duration,
            codex_agent_name=codex_agent_name,
            poll=poll,
            dependency_analyses=analyses,
            dependency_context=dependency_context,
            runner=runner,
        )
        dispatched_issues = {result.issue_number for result in batch_dispatch}
        for analysis in pending_analyses:
            if analysis.issue.number in dispatched_issues:
                continue
            batch_dispatch.append(
                WorkerSessionResult(
                    analysis.issue.number,
                    commandmate_worktree_id(analysis.branch_name),
                    "blocked",
                    None,
                    None,
                    "usable worktree was not available for dispatch",
                    (),
                )
            )
        batch_dispatch = verified_dispatch + processing_dispatch + batch_dispatch
        dispatch_results.extend(batch_dispatch)

        if dry_run or not dispatch_enabled:
            status = "planned" if dry_run else "not-dispatched"
            message = (
                "dry-run: bounded batch dispatch planned"
                if dry_run
                else "CommandMate dispatch was not requested"
            )
            batch_results.append(
                SchedulerBatchResult(batch_index, tuple(issue_numbers), status, message)
            )
            completed_issues.update(issue_numbers)
            continue

        active = [
            result
            for result in batch_dispatch
            if result.status in {"sent", "processing", "started-but-idle"}
        ]
        verification = wait_for_verified_workers(
            batch_analyses,
            worktree_results,
            timeout_seconds=timeout_seconds,
            codex_agent_name=codex_agent_name,
            runner=runner,
            idle_grace_seconds=(
                float(stall_timeout_seconds)
                if stall_timeout_seconds > 0
                else 120.0
            ),
        )
        verification_by_issue = {
            result.issue_number: result for result in verification
        }
        batch_wait = []
        for result in active:
            evidence = verification_by_issue.get(result.issue_number)
            if evidence is not None and evidence.status == "passed":
                batch_wait.append(
                    WorkerWaitResult(
                        result.issue_number,
                        result.worktree_id,
                        "completed",
                        "worker completed with committed passing evidence",
                    )
                )
            else:
                message = (
                    evidence.message
                    if evidence is not None
                    else "worker verification result is missing"
                )
                batch_wait.append(
                    WorkerWaitResult(
                        result.issue_number,
                        result.worktree_id,
                        "blocked",
                        message,
                    )
                )
        wait_results.extend(batch_wait)
        dispatch_passed = len(batch_dispatch) == len(issue_numbers) and all(
            result.status != "blocked" for result in batch_dispatch
        )
        verification_passed = len(verification) == len(issue_numbers) and all(
            result.status == "passed" for result in verification
        )
        if not dispatch_passed or not verification_passed:
            failures = "; ".join(
                f"#{result.issue_number}: {result.message}"
                for result in verification
                if result.status != "passed"
            )
            reason = "dispatch" if not dispatch_passed else "worker verification"
            blocked_by = f"scheduler batch {batch_index} failed {reason}"
            if failures:
                blocked_by = f"{blocked_by} ({failures})"
            batch_results.append(
                SchedulerBatchResult(
                    batch_index, tuple(issue_numbers), "blocked", blocked_by
                )
            )
            continue

        batch_results.append(
            SchedulerBatchResult(
                batch_index,
                tuple(issue_numbers),
                "completed",
                "all workers completed and passed verification",
            )
        )
        completed_issues.update(issue_numbers)

    return dispatch_results, wait_results, batch_results


def render_scheduler_report(
    batches: list[list[int]],
    max_parallel: int,
    results: list[SchedulerBatchResult],
) -> str:
    lines = [
        "# Scheduler Report",
        "",
        f"- Enforced max parallel: `{max_parallel}`",
        f"- Planned batches: `{len(batches)}`",
        "",
    ]
    by_index = {result.batch_index: result for result in results}
    for batch_index, issue_numbers in enumerate(batches, start=1):
        result = by_index.get(batch_index)
        issues = ", ".join(f"#{number}" for number in issue_numbers)
        lines.extend(
            [
                f"## Batch {batch_index}",
                "",
                f"- Issues: {issues}",
                f"- Width: `{len(issue_numbers)}`",
                f"- Status: `{result.status if result else 'not-started'}`",
                f"- Message: {result.message if result else 'not started'}",
                "",
            ]
        )
    return "\n".join(lines)


def render_worker_wait_report(results: list[WorkerWaitResult]) -> str:
    lines = ["# CommandMate Wait Report", ""]
    if not results:
        return "# CommandMate Wait Report\n\nNo worker wait requested.\n"
    for result in results:
        lines.extend(
            [
                f"## Issue #{result.issue_number}",
                "",
                f"- Worktree ID: `{result.worktree_id}`",
                f"- Status: `{result.status}`",
                f"- Message: {result.message}",
                "",
            ]
        )
    return "\n".join(lines)


def verify_worker_reports(
    analyses: list[IssueAnalysis],
    worktree_results: list[WorktreeResult],
    *,
    dry_run: bool,
    runner: Runner = run_command,
) -> list[WorkerVerificationResult]:
    by_issue = {result.issue_number: result for result in worktree_results}
    results: list[WorkerVerificationResult] = []
    status_re = re.compile(r"(?im)^\s*-\s*Status:\s*`?(passed|blocked|failed)`?\s*$")
    check_re = re.compile(r"(?im)^\s*-\s*`[^`]+`:\s*`?(passed|blocked|failed)`?\s*$")
    for analysis in analyses:
        worktree = by_issue.get(analysis.issue.number)
        root = (
            worktree.worktree_path
            if worktree is not None
            else resolve_worktree_path(analysis.worktree_path)
        )
        report_path = (
            root / "dev-reports" / f"issue-{analysis.issue.number}" / "verification.md"
        )
        if dry_run:
            results.append(
                WorkerVerificationResult(
                    analysis.issue.number,
                    "planned",
                    report_path,
                    "dry-run: worker verification report not inspected",
                )
            )
            continue
        if worktree is None or worktree.status == "blocked":
            results.append(
                WorkerVerificationResult(
                    analysis.issue.number,
                    "blocked",
                    report_path,
                    "usable worktree not available",
                )
            )
            continue
        if not report_path.is_file():
            results.append(
                WorkerVerificationResult(
                    analysis.issue.number,
                    "blocked",
                    report_path,
                    "verification report is missing",
                )
            )
            continue
        dirty = runner(["git", "status", "--porcelain"], cwd=root)
        if dirty.returncode != 0 or dirty.stdout.strip():
            results.append(
                WorkerVerificationResult(
                    analysis.issue.number,
                    "blocked",
                    report_path,
                    "worktree contains uncommitted changes after worker completion",
                )
            )
            continue
        relative_report = report_path.relative_to(root)
        tracked = runner(
            ["git", "ls-files", "--error-unmatch", str(relative_report)], cwd=root
        )
        if tracked.returncode != 0:
            results.append(
                WorkerVerificationResult(
                    analysis.issue.number,
                    "blocked",
                    report_path,
                    "verification report is not committed on the issue branch",
                )
            )
            continue
        report = report_path.read_text(encoding="utf-8")
        status_match = status_re.search(report)
        check_statuses = check_re.findall(report)
        if status_match is None:
            status = "blocked"
            message = "verification report has no machine-readable status"
        elif status_match.group(1).lower() != "passed":
            status = "blocked"
            message = f"worker verification status is {status_match.group(1).lower()}"
        elif not check_statuses:
            status = "blocked"
            message = "verification report contains no check results"
        elif any(item.lower() != "passed" for item in check_statuses):
            status = "blocked"
            message = "one or more worker verification checks did not pass"
        else:
            status = "passed"
            message = f"{len(check_statuses)} worker verification checks passed"
        results.append(
            WorkerVerificationResult(
                analysis.issue.number, status, report_path, message
            )
        )
    return results


def wait_for_verified_workers(
    analyses: list[IssueAnalysis],
    worktree_results: list[WorktreeResult],
    *,
    timeout_seconds: int,
    codex_agent_name: str,
    runner: Runner = run_command,
    sleep_fn=time.sleep,
    monotonic_fn=time.monotonic,
    idle_grace_seconds: float = 120.0,
) -> list[WorkerVerificationResult]:
    """Wait through premature CommandMate completion until workers stop processing.

    CommandMate's wait command can briefly report completion while a newly started
    Codex instance is still processing. Worker evidence remains authoritative: only
    retry verification while at least one unverified worker is observably processing.
    """

    verification = verify_worker_reports(
        analyses, worktree_results, dry_run=False, runner=runner
    )
    deadline = monotonic_fn() + max(timeout_seconds, 0)
    by_issue = {analysis.issue.number: analysis for analysis in analyses}
    idle_since: float | None = None
    while any(result.status != "passed" for result in verification):
        pending = [
            by_issue[result.issue_number]
            for result in verification
            if result.status != "passed"
        ]
        states = [
            get_commandmate_state(
                commandmate_worktree_id(analysis.branch_name),
                codex_agent_name=codex_agent_name,
                runner=runner,
            )
            for analysis in pending
        ]
        now = monotonic_fn()
        if any(state["processing"] is True for state in states):
            idle_since = None
        elif any(state["found"] is True and state["running"] is True for state in states):
            if idle_since is None:
                idle_since = now
            if now - idle_since >= max(idle_grace_seconds, 0.0):
                break
        else:
            break
        remaining = deadline - now
        if remaining <= 0:
            break
        sleep_fn(min(2.0, remaining))
        verification = verify_worker_reports(
            analyses, worktree_results, dry_run=False, runner=runner
        )
    return verification


def render_worker_verification_report(results: list[WorkerVerificationResult]) -> str:
    lines = ["# Worker Verification", ""]
    if not results:
        return "# Worker Verification\n\nNo verification reports inspected.\n"
    for result in results:
        lines.extend(
            [
                f"## Issue #{result.issue_number}",
                "",
                f"- Status: `{result.status}`",
                f"- Report: `{result.report_path}`",
                f"- Message: {result.message}",
                "",
            ]
        )
    return "\n".join(lines)


def render_pr_body(analysis: IssueAnalysis, run_id: str) -> str:
    files = "\n".join(f"- `{item}`" for item in analysis.suspected_files) or "- 未特定"
    tests = (
        "\n".join(f"- `{item}`" for item in analysis.test_expectations) or "- 未実行"
    )
    issue_links = "\n".join(
        f"Tracks #{number}" for number in included_issue_numbers(analysis)
    )
    return "\n".join(
        [
            issue_links,
            "",
            "## Summary",
            "",
            f"- {analysis.objective}",
            "",
            "## Changed Files",
            "",
            files,
            "",
            "## Tests Run",
            "",
            tests,
            "",
            "## Known Risks",
            "",
            "- None recorded by orchestration planner.",
            "",
            "## Orchestration",
            "",
            f"- Run ID: `{run_id}`",
            "",
        ]
    )


def find_existing_pr(
    branch_name: str, *, runner: Runner = run_command
) -> PullRequestResult | None:
    completed = runner(
        [
            "gh",
            "pr",
            "list",
            "--head",
            branch_name,
            "--base",
            "develop",
            "--json",
            "number,url,state,isDraft",
        ],
        cwd=REPO_ROOT,
    )
    if completed.returncode != 0 or not completed.stdout.strip():
        return None
    raw = json.loads(completed.stdout)
    if not raw:
        return None
    first = raw[0]
    return PullRequestResult(
        issue_number=0,
        branch_name=branch_name,
        status="existing",
        pr_number=int(first["number"]),
        url=str(first.get("url") or ""),
        message=(
            f"existing PR state: {first.get('state', 'unknown')}; "
            f"draft={bool(first.get('isDraft'))}"
        ),
    )


def create_pull_requests(
    analyses: list[IssueAnalysis],
    *,
    run_id: str,
    dry_run: bool,
    runner: Runner = run_command,
) -> list[PullRequestResult]:
    results: list[PullRequestResult] = []
    for analysis in analyses:
        if not dry_run and not branch_has_commits(analysis.branch_name, runner=runner):
            results.append(
                PullRequestResult(
                    issue_number=analysis.issue.number,
                    branch_name=analysis.branch_name,
                    status="blocked",
                    pr_number=None,
                    url=None,
                    message="branch has no commits ahead of origin/develop",
                )
            )
            continue
        if not dry_run:
            push = push_branch_to_origin(analysis.branch_name, runner=runner)
            if push.returncode != 0:
                results.append(
                    PullRequestResult(
                        issue_number=analysis.issue.number,
                        branch_name=analysis.branch_name,
                        status="blocked",
                        pr_number=None,
                        url=None,
                        message=push.stderr.strip() or "branch push failed",
                    )
                )
                continue
        existing = (
            None if dry_run else find_existing_pr(analysis.branch_name, runner=runner)
        )
        if existing is not None:
            results.append(
                PullRequestResult(
                    issue_number=analysis.issue.number,
                    branch_name=analysis.branch_name,
                    status=existing.status,
                    pr_number=existing.pr_number,
                    url=existing.url,
                    message=existing.message,
                )
            )
            continue
        included = included_issue_numbers(analysis)
        issue_prefix = "/".join(f"#{number}" for number in included)
        title = f"{issue_prefix} {analysis.issue.title}"
        body = render_pr_body(analysis, run_id)
        cmd = [
            "gh",
            "pr",
            "create",
            "--base",
            "develop",
            "--head",
            analysis.branch_name,
            "--title",
            title,
            "--body",
            body,
            "--draft",
        ]
        if dry_run:
            results.append(
                PullRequestResult(
                    issue_number=analysis.issue.number,
                    branch_name=analysis.branch_name,
                    status="planned",
                    pr_number=None,
                    url=None,
                    message="dry-run: PR creation skipped",
                )
            )
            continue
        completed = runner(cmd, cwd=REPO_ROOT, check=True)
        url = completed.stdout.strip() or None
        results.append(
            PullRequestResult(
                issue_number=analysis.issue.number,
                branch_name=analysis.branch_name,
                status="created",
                pr_number=parse_pr_number(url or ""),
                url=url,
                message="branch pushed and draft PR created",
            )
        )
    return results


def push_branch_to_origin(
    branch_name: str, *, runner: Runner = run_command
) -> subprocess.CompletedProcess[str]:
    return runner(["git", "push", "-u", "origin", branch_name], cwd=REPO_ROOT)


def parse_pr_number(value: str) -> int | None:
    match = re.search(r"/pull/(\d+)(?:\D*$|$)", value)
    if not match:
        return None
    return int(match.group(1))


def pr_numbers_for_merge(results: list[PullRequestResult]) -> list[int]:
    numbers: list[int] = []
    for result in results:
        if result.status not in {"created", "existing"}:
            continue
        if result.pr_number is not None:
            numbers.append(result.pr_number)
            continue
        parsed = parse_pr_number(result.url or "")
        if parsed is not None:
            numbers.append(parsed)
    return numbers


def order_pr_numbers_for_merge(
    results: list[PullRequestResult],
    issue_order: list[int],
    requested_pr_numbers: list[int],
) -> list[int]:
    by_issue: dict[int, int] = {}
    for result in results:
        if result.status not in {"created", "existing"}:
            continue
        pr_number = result.pr_number or parse_pr_number(result.url or "")
        if pr_number is not None:
            by_issue[result.issue_number] = pr_number

    if not by_issue:
        if len(requested_pr_numbers) != len(set(requested_pr_numbers)):
            raise ValueError("PR number list contains duplicates")
        return requested_pr_numbers

    missing_issues = [number for number in issue_order if number not in by_issue]
    if missing_issues:
        formatted = ", ".join(f"#{number}" for number in missing_issues)
        raise ValueError(f"missing PR mapping for issues {formatted}")
    ordered = [by_issue[number] for number in issue_order]
    if requested_pr_numbers:
        if len(requested_pr_numbers) != len(set(requested_pr_numbers)):
            raise ValueError("PR number list contains duplicates")
        if set(requested_pr_numbers) != set(ordered):
            raise ValueError("PR number list must match every requested issue")
    return ordered


def branch_has_commits(branch_name: str, *, runner: Runner = run_command) -> bool:
    completed = runner(
        ["git", "rev-list", "--count", f"{DEFAULT_BASE}..{branch_name}"],
        cwd=REPO_ROOT,
    )
    if completed.returncode != 0:
        return False
    try:
        return int(completed.stdout.strip() or "0") > 0
    except ValueError:
        return False


def render_pr_report(results: list[PullRequestResult]) -> str:
    lines = ["# PR Report", ""]
    for result in results:
        lines.extend(
            [
                f"## Issue #{result.issue_number}",
                "",
                f"- Branch: `{result.branch_name}`",
                f"- Status: `{result.status}`",
                f"- PR: `{result.pr_number or result.url or 'pending'}`",
                f"- Message: {result.message}",
                "",
            ]
        )
    return "\n".join(lines)


def merge_pull_requests(
    pr_numbers: list[int],
    *,
    dry_run: bool,
    merge_method: str | None,
    integration_checks: list[str],
    uat_gate: UatGateResult,
    expected_head_by_pr: dict[int, str] | None = None,
    runner: Runner = run_command,
) -> list[MergeResult]:
    if dry_run:
        return [
            MergeResult(pr_number, "planned", "dry-run: merge skipped")
            for pr_number in pr_numbers
        ]

    emit_progress(f"merge preflight started for {len(pr_numbers)} pull request(s)")
    mergeability: dict[int, MergeResult] = {}
    for pr_number in pr_numbers:
        emit_progress(f"PR #{pr_number}: checking initial mergeability")
        mergeable = check_pr_mergeability(pr_number, runner=runner)
        if mergeable.status not in {"mergeable", "draft"}:
            return [mergeable]
        mergeability[pr_number] = mergeable

    for pr_number in pr_numbers:
        checks = wait_for_pr_checks(pr_number, runner=runner)
        if checks.status != "passed":
            return [MergeResult(pr_number, "blocked", checks.message)]

    if uat_gate.status != "passed":
        return [
            MergeResult(
                pr_numbers[0],
                "blocked",
                f"UAT gate is {uat_gate.status}: {uat_gate.message}",
            )
        ]

    for pr_number in pr_numbers:
        if mergeability[pr_number].status != "draft":
            continue
        ready = mark_pr_ready(pr_number, runner=runner)
        if ready.status != "ready":
            return [ready]

    for pr_number in pr_numbers:
        checks = wait_for_pr_checks(pr_number, runner=runner)
        if checks.status != "passed":
            return [
                MergeResult(
                    pr_number,
                    "blocked",
                    f"post-UAT CI verification failed: {checks.message}",
                )
            ]
        mergeable = check_pr_mergeability(pr_number, runner=runner)
        if mergeable.status != "mergeable":
            return [mergeable]

    results: list[MergeResult] = []
    for pr_number in pr_numbers:
        emit_progress(f"PR #{pr_number}: final checks before merge")
        checks = wait_for_pr_checks(pr_number, runner=runner)
        if checks.status != "passed":
            results.append(
                MergeResult(
                    pr_number,
                    "blocked",
                    f"pre-merge CI verification failed: {checks.message}",
                )
            )
            break
        mergeable = check_pr_mergeability(pr_number, runner=runner)
        if mergeable.status != "mergeable":
            results.append(mergeable)
            break
        expected_head = (expected_head_by_pr or {}).get(pr_number, "").lower()
        if expected_head:
            try:
                current_head = read_pr_head_oid(pr_number, runner=runner)
            except (RuntimeError, ValueError, json.JSONDecodeError) as exc:
                results.append(MergeResult(pr_number, "blocked", str(exc)))
                break
            if current_head != expected_head:
                results.append(
                    MergeResult(
                        pr_number,
                        "blocked",
                        "PR head changed after UAT; rerun CI and UAT for the current head",
                    )
                )
                break
        resolved_merge_method = merge_method or DEFAULT_MERGE_METHOD
        emit_progress(f"PR #{pr_number}: merging with {resolved_merge_method}")
        cmd = [
            "gh",
            "pr",
            "merge",
            str(pr_number),
            f"--{resolved_merge_method}",
        ]
        merge = runner(cmd, cwd=REPO_ROOT)
        if merge.returncode != 0:
            results.append(
                MergeResult(
                    pr_number, "blocked", merge.stderr.strip() or "merge failed"
                )
            )
            break
        runner(
            ["git", "pull", "--ff-only", "origin", "develop"], cwd=REPO_ROOT, check=True
        )
        emit_progress(f"PR #{pr_number}: develop updated; running integration checks")
        verification = run_integration_checks(integration_checks, runner=runner)
        if verification.startswith("failed:"):
            results.append(
                MergeResult(
                    pr_number,
                    "blocked",
                    "merged, but integration verification failed",
                    verification,
                )
            )
            break
        results.append(
            MergeResult(pr_number, "merged", "merged and develop updated", verification)
        )
        emit_progress(f"PR #{pr_number}: merge completed")
    return results


def mark_pr_ready(pr_number: int, *, runner: Runner = run_command) -> MergeResult:
    completed = runner(["gh", "pr", "ready", str(pr_number)], cwd=REPO_ROOT)
    if completed.returncode != 0:
        return MergeResult(
            pr_number,
            "blocked",
            completed.stderr.strip() or "could not mark draft PR ready for review",
        )
    return MergeResult(
        pr_number, "ready", "draft PR marked ready after CI and UAT passed"
    )


def wait_for_pr_checks(pr_number: int, *, runner: Runner = run_command) -> MergeResult:
    emit_progress(f"PR #{pr_number}: waiting for GitHub checks")
    kwargs = {"capture_output": False} if runner is run_command else {}
    checks = runner(
        ["gh", "pr", "checks", str(pr_number), "--watch", "--interval", "10"],
        cwd=REPO_ROOT,
        **kwargs,
    )
    if checks.returncode != 0:
        return MergeResult(
            pr_number,
            "blocked",
            (checks.stderr or "").strip() or "CI checks failed or unavailable",
        )
    emit_progress(f"PR #{pr_number}: GitHub checks passed")
    return MergeResult(pr_number, "passed", "CI checks passed")


def wait_for_all_pr_checks(
    pr_numbers: list[int], *, dry_run: bool, runner: Runner = run_command
) -> list[MergeResult]:
    if dry_run:
        return [
            MergeResult(pr_number, "planned", "dry-run: CI checks not watched")
            for pr_number in pr_numbers
        ]
    return [wait_for_pr_checks(pr_number, runner=runner) for pr_number in pr_numbers]


def render_ci_report(results: list[MergeResult]) -> str:
    lines = ["# CI Report", ""]
    if not results:
        return "# CI Report\n\nNo PR checks evaluated.\n"
    for result in results:
        lines.extend(
            [
                f"## PR #{result.pr_number}",
                "",
                f"- Status: `{result.status}`",
                f"- Message: {result.message}",
                "",
            ]
        )
    return "\n".join(lines)


def check_pr_mergeability(
    pr_number: int, *, runner: Runner = run_command
) -> MergeResult:
    completed = runner(
        [
            "gh",
            "pr",
            "view",
            str(pr_number),
            "--json",
            "baseRefName,headRefName,headRefOid,isDraft,mergeStateStatus,number",
        ],
        cwd=REPO_ROOT,
    )
    if completed.returncode != 0:
        return MergeResult(pr_number, "blocked", "could not read PR mergeability")
    raw = json.loads(completed.stdout)
    if raw.get("isDraft"):
        return MergeResult(pr_number, "draft", "PR is draft")
    merge_state = str(raw.get("mergeStateStatus") or "UNKNOWN")
    if merge_state not in {"CLEAN", "HAS_HOOKS", "UNSTABLE", "UNKNOWN"}:
        head_ref = str(raw.get("headRefName") or f"PR #{pr_number} head branch")
        base_ref = str(raw.get("baseRefName") or "base branch")
        return MergeResult(
            pr_number,
            "blocked",
            (
                f"mergeStateStatus={merge_state}; synchronize `{head_ref}` with "
                f"`origin/{base_ref}`, rerun verification, push, and resume"
            ),
        )
    return MergeResult(pr_number, "mergeable", f"mergeStateStatus={merge_state}")


def read_pr_head_oid(pr_number: int, *, runner: Runner = run_command) -> str:
    completed = runner(
        ["gh", "pr", "view", str(pr_number), "--json", "headRefOid,number"],
        cwd=REPO_ROOT,
    )
    if completed.returncode != 0:
        raise RuntimeError(f"could not read head commit for PR #{pr_number}")
    head_oid = str(json.loads(completed.stdout).get("headRefOid") or "").lower()
    if not re.fullmatch(r"[0-9a-f]{40}", head_oid):
        raise ValueError(f"PR #{pr_number} returned an invalid head commit")
    return head_oid


def run_integration_checks(checks: list[str], *, runner: Runner = run_command) -> str:
    if not checks:
        return "not-configured"
    for index, check in enumerate(checks, start=1):
        emit_progress(f"integration check {index}/{len(checks)} started: {check}")
        kwargs = {"capture_output": False} if runner is run_command else {}
        completed = runner(["sh", "-c", check], cwd=REPO_ROOT, **kwargs)
        if completed.returncode != 0:
            emit_progress(f"integration check {index}/{len(checks)} failed")
            return f"failed: {check}"
        emit_progress(f"integration check {index}/{len(checks)} passed")
    return "passed"


def render_merge_report(results: list[MergeResult]) -> str:
    lines = ["# Merge Report", ""]
    if not results:
        return "# Merge Report\n\nNo PRs merged.\n"
    for result in results:
        lines.extend(
            [
                f"## PR #{result.pr_number}",
                "",
                f"- Status: `{result.status}`",
                f"- Message: {result.message}",
                f"- Verification: `{result.verification_status}`",
                "",
            ]
        )
    return "\n".join(lines)


def render_merge_recovery_report(results: list[MergeResult]) -> str:
    blocked = [result for result in results if result.status == "blocked"]
    lines = ["# Merge Recovery Report", ""]
    if not blocked:
        return "# Merge Recovery Report\n\nNo recovery action is required.\n"
    lines.extend(
        [
            "A merge gate blocked. No conflict was resolved automatically.",
            "Follow the per-PR blocker below. For a base-sync blocker, synchronize the named head branch, rerun verification, push, and resume the same run.",
            "",
        ]
    )
    for result in blocked:
        lines.extend(
            [
                f"- PR #{result.pr_number}: {result.message}",
            ]
        )
    lines.append("")
    return "\n".join(lines)


def close_merged_issues(
    issue_to_pr: dict[int, int],
    merge_results: list[MergeResult],
    *,
    authorized: bool,
    dry_run: bool,
    runner: Runner = run_command,
) -> list[IssueCloseResult]:
    merged_prs = {
        result.pr_number for result in merge_results if result.status == "merged"
    }
    results: list[IssueCloseResult] = []
    for issue_number, pr_number in issue_to_pr.items():
        if pr_number not in merged_prs:
            continue
        if dry_run:
            results.append(
                IssueCloseResult(
                    issue_number,
                    pr_number,
                    "planned" if authorized else "not-authorized",
                    "dry-run: Issue close skipped",
                )
            )
            continue
        if not authorized:
            results.append(
                IssueCloseResult(
                    issue_number,
                    pr_number,
                    "not-authorized",
                    "pass --close-issues to authorize closing this merged Issue",
                )
            )
            continue
        viewed = runner(
            ["gh", "issue", "view", str(issue_number), "--json", "number,state"],
            cwd=REPO_ROOT,
        )
        if viewed.returncode != 0:
            results.append(
                IssueCloseResult(
                    issue_number,
                    pr_number,
                    "blocked",
                    (viewed.stderr or "").strip() or "could not read Issue state",
                )
            )
            continue
        try:
            state = str(json.loads(viewed.stdout).get("state") or "").upper()
        except json.JSONDecodeError:
            results.append(
                IssueCloseResult(
                    issue_number,
                    pr_number,
                    "blocked",
                    "Issue state response was not valid JSON",
                )
            )
            continue
        if state == "CLOSED":
            results.append(
                IssueCloseResult(
                    issue_number,
                    pr_number,
                    "already-closed",
                    "Issue was already closed",
                )
            )
            continue
        if state != "OPEN":
            results.append(
                IssueCloseResult(
                    issue_number,
                    pr_number,
                    "blocked",
                    f"unexpected Issue state: {state or 'unset'}",
                )
            )
            continue
        emit_progress(f"Issue #{issue_number}: closing after PR #{pr_number} merged")
        closed = runner(
            [
                "gh",
                "issue",
                "close",
                str(issue_number),
                "--reason",
                "completed",
                "--comment",
                f"Implemented and merged into develop via PR #{pr_number}.",
            ],
            cwd=REPO_ROOT,
        )
        if closed.returncode != 0:
            results.append(
                IssueCloseResult(
                    issue_number,
                    pr_number,
                    "blocked",
                    (closed.stderr or "").strip() or "Issue close failed",
                )
            )
            continue
        verified = runner(
            ["gh", "issue", "view", str(issue_number), "--json", "number,state"],
            cwd=REPO_ROOT,
        )
        if verified.returncode != 0:
            results.append(
                IssueCloseResult(
                    issue_number,
                    pr_number,
                    "blocked",
                    (verified.stderr or "").strip()
                    or "Issue close succeeded but state readback failed",
                )
            )
            continue
        try:
            verified_state = str(
                json.loads(verified.stdout).get("state") or ""
            ).upper()
        except json.JSONDecodeError:
            results.append(
                IssueCloseResult(
                    issue_number,
                    pr_number,
                    "blocked",
                    "Issue close readback was not valid JSON",
                )
            )
            continue
        if verified_state != "CLOSED":
            results.append(
                IssueCloseResult(
                    issue_number,
                    pr_number,
                    "blocked",
                    f"Issue close was not confirmed; readback state={verified_state or 'unset'}",
                )
            )
            continue
        results.append(
            IssueCloseResult(
                issue_number,
                pr_number,
                "closed",
                "Issue closed as completed after mapped PR merge",
            )
        )
    return results


def render_issue_close_report(results: list[IssueCloseResult]) -> str:
    lines = ["# Issue Close Report", ""]
    if not results:
        return "# Issue Close Report\n\nNo merged Issues were eligible for closure.\n"
    for result in results:
        lines.extend(
            [
                f"## Issue #{result.issue_number}",
                "",
                f"- PR: `#{result.pr_number}`",
                f"- Status: `{result.status}`",
                f"- Message: {result.message}",
                "",
            ]
        )
    return "\n".join(lines)


def render_uat_report(
    analyses: list[IssueAnalysis],
    results: list[UatResult] | None = None,
    gate: UatGateResult | None = None,
    *,
    ignored_out_of_scope_results: int = 0,
) -> str:
    by_scenario = {
        (result.issue_number, result.scenario_index): result
        for result in (results or [])
    }
    lines = [
        "# UAT Report",
        "",
        "## Merge Gate",
        "",
        f"- Status: `{gate.status if gate else 'pending'}`",
        f"- Message: {gate.message if gate else 'UAT results have not been evaluated.'}",
        "",
        "## Automated Checks",
        "",
        "- Worker command evidence: see `worker-verification.md`.",
        "- Pull-request checks: see `ci-report.md`.",
        f"- Ignored out-of-scope UAT results: {ignored_out_of_scope_results}.",
        "",
        "## Manual CLI / TTY / GUI / Real-device Checks",
        "",
    ]
    for analysis in analyses:
        criteria = analysis.acceptance_criteria or ("Issue の期待動作を満たすこと",)
        lines.extend(
            [f"### Issue #{analysis.issue.number}: {analysis.issue.title}", ""]
        )
        for index, criterion in enumerate(criteria, start=1):
            result = by_scenario.get((analysis.issue.number, index))
            lines.extend(
                [
                    f"#### Scenario {index}",
                    "",
                    "- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。",
                    f"- 操作: `{criterion}` を確認できる画面または実機操作を行う。",
                    f"- 期待結果: {criterion}",
                    f"- Actual: {result.actual if result else 'unchecked'}",
                    "- Candidate head SHA: "
                    + (result.candidate_head_sha if result else "unchecked"),
                    "- Evidence: "
                    + (
                        result.evidence
                        if result
                        else "screenshot / relevant logs / 操作メモ / device or browser version"
                    ),
                    f"- Result: {result.status if result else 'unchecked'}",
                    "",
                ]
            )
    lines.extend(
        [
            "## Fix Loop",
            "",
            "UAT が fail した場合は、該当 Issue / PR / file に mapping する。",
            "そのうえで focused failure prompt から follow-up worktree を作成する。",
            "Retry limit: 3",
            "",
        ]
    )
    return "\n".join(lines)


def load_uat_results(path: Path | None) -> list[UatResult]:
    if path is None:
        return []
    raw = json.loads(path.read_text(encoding="utf-8"))
    items = raw["results"] if isinstance(raw, dict) and "results" in raw else raw
    if not isinstance(items, list):
        raise TypeError(
            "UAT results JSON must be a list or an object with a 'results' list"
        )
    results: list[UatResult] = []
    for item in items:
        if not isinstance(item, dict):
            raise TypeError("each UAT result must be an object")
        results.append(
            UatResult(
                issue_number=int(item["issue_number"]),
                scenario_index=int(item["scenario_index"]),
                status=str(item.get("status") or "").strip().lower(),
                actual=str(item.get("actual") or "").strip(),
                evidence=str(item.get("evidence") or "").strip(),
                candidate_head_sha=str(item.get("candidate_head_sha") or "")
                .strip()
                .lower(),
            )
        )
    return results


def scope_uat_results(
    results: list[UatResult],
    requested_issues: list[int],
    *,
    allow_superset: bool,
) -> tuple[list[UatResult], int]:
    if not allow_superset:
        return results, 0
    requested = set(requested_issues)
    scoped = [result for result in results if result.issue_number in requested]
    return scoped, len(results) - len(scoped)


def evaluate_uat_gate(
    analyses: list[IssueAnalysis],
    results: list[UatResult],
    *,
    require_complete: bool,
    dry_run: bool,
    expected_head_by_issue: dict[int, str] | None = None,
    require_head_binding: bool = False,
) -> UatGateResult:
    if dry_run:
        return UatGateResult("planned", "dry-run: UAT evidence not evaluated")
    expected = {
        (analysis.issue.number, index)
        for analysis in analyses
        for index, _criterion in enumerate(
            analysis.acceptance_criteria or ("Issue の期待動作を満たすこと",), start=1
        )
    }
    observed: dict[tuple[int, int], UatResult] = {}
    for result in results:
        key = (result.issue_number, result.scenario_index)
        if key in observed:
            return UatGateResult(
                "blocked", f"duplicate UAT result for Issue #{key[0]} scenario {key[1]}"
            )
        observed[key] = result
    unexpected = sorted(set(observed) - expected)
    if unexpected:
        issue_number, scenario_index = unexpected[0]
        return UatGateResult(
            "blocked",
            f"unexpected UAT result for Issue #{issue_number} scenario {scenario_index}",
        )
    missing = sorted(expected - set(observed))
    if missing:
        status = "blocked" if require_complete else "pending"
        issue_number, scenario_index = missing[0]
        return UatGateResult(
            status,
            f"missing UAT evidence for Issue #{issue_number} scenario {scenario_index}",
        )
    for key in sorted(expected):
        result = observed[key]
        if result.status != "passed":
            return UatGateResult(
                "blocked",
                f"UAT did not pass for Issue #{key[0]} scenario {key[1]}: {result.status or 'unset'}",
            )
        if not result.actual:
            return UatGateResult(
                "blocked",
                f"UAT actual result is empty for Issue #{key[0]} scenario {key[1]}",
            )
        if not result.evidence:
            return UatGateResult(
                "blocked",
                f"UAT evidence is empty for Issue #{key[0]} scenario {key[1]}",
            )
        expected_head = (expected_head_by_issue or {}).get(key[0], "").lower()
        if require_head_binding and not result.candidate_head_sha:
            return UatGateResult(
                "blocked",
                f"UAT candidate head SHA is empty for Issue #{key[0]} scenario {key[1]}",
            )
        if require_head_binding and not re.fullmatch(
            r"[0-9a-f]{40}", result.candidate_head_sha
        ):
            return UatGateResult(
                "blocked",
                f"UAT candidate head SHA is invalid for Issue #{key[0]} scenario {key[1]}",
            )
        if require_head_binding and not expected_head:
            return UatGateResult(
                "blocked",
                f"current PR head is unavailable for Issue #{key[0]}",
            )
        if require_head_binding and result.candidate_head_sha != expected_head:
            return UatGateResult(
                "blocked",
                f"UAT candidate head does not match current PR head for Issue #{key[0]} scenario {key[1]}",
            )
    return UatGateResult(
        "passed", f"all {len(expected)} UAT scenarios passed with evidence"
    )


def load_uat_failures(path: Path | None) -> list[UatFailure]:
    if path is None:
        return []
    raw = json.loads(path.read_text(encoding="utf-8"))
    items = raw["failures"] if isinstance(raw, dict) and "failures" in raw else raw
    if not isinstance(items, list):
        raise TypeError(
            "UAT failures JSON must be a list or an object with a 'failures' list"
        )
    failures: list[UatFailure] = []
    for item in items:
        if not isinstance(item, dict):
            continue
        failures.append(
            UatFailure(
                issue_number=int(item["issue_number"]),
                scenario=str(item.get("scenario", "")),
                expected=str(item.get("expected", "")),
                actual=str(item.get("actual", "")),
                evidence=str(item.get("evidence", "")),
            )
        )
    return failures


def render_uat_fix_prompts(
    failures: list[UatFailure], analyses: list[IssueAnalysis]
) -> str:
    by_issue = {analysis.issue.number: analysis for analysis in analyses}
    lines = ["# UAT Fix Prompts", ""]
    if not failures:
        lines.extend(["No UAT failures recorded.", ""])
        return "\n".join(lines)

    for failure in failures:
        analysis = by_issue.get(failure.issue_number)
        title = analysis.issue.title if analysis else "Unknown issue"
        branch = (
            analysis.branch_name
            if analysis
            else f"fix/issue-{failure.issue_number}-uat"
        )
        lines.extend(
            [
                f"## Issue #{failure.issue_number}: {title}",
                "",
                "```text",
                f"$codex-issue-worker UAT failure fix for Issue #{failure.issue_number}",
                "",
                "UAT で以下の scenario が fail しました。原因を特定し、最小修正を行ってください。",
                "",
                f"- Scenario: {failure.scenario or 'unspecified'}",
                f"- Expected: {failure.expected or 'unspecified'}",
                f"- Actual: {failure.actual or 'unspecified'}",
                f"- Evidence: {failure.evidence or 'not provided'}",
                f"- Suggested branch/worktree context: {branch}",
                "",
                "実施内容:",
                "1. 失敗を再現または evidence から原因を特定する。",
                "2. focused fix を行う。",
                "3. focused verification を実行する。",
                "4. UAT scenario の再確認手順を更新する。",
                "5. follow-up PR を作成できる状態にする。",
                "```",
                "",
            ]
        )
    return "\n".join(lines)


def create_uat_fix_worktrees(
    failures: list[UatFailure],
    *,
    dry_run: bool,
    runner: Runner = run_command,
) -> list[UatFixWorktreeResult]:
    results: list[UatFixWorktreeResult] = []
    seen: set[int] = set()
    for failure in failures:
        if failure.issue_number in seen:
            continue
        seen.add(failure.issue_number)
        slug = slugify(failure.scenario or "uat-failure", max_len=32)
        branch = f"fix/issue-{failure.issue_number}-uat-{slug}"
        path = (
            REPO_ROOT
            / f"../{REPO_ROOT.name}-fix-issue-{failure.issue_number}-uat-{slug}"
        ).resolve()
        if dry_run:
            results.append(
                UatFixWorktreeResult(
                    failure.issue_number,
                    branch,
                    path,
                    "planned",
                    "dry-run: UAT fix worktree creation skipped",
                )
            )
            continue
        if path.exists():
            if worktree_is_dirty(path, runner):
                results.append(
                    UatFixWorktreeResult(
                        failure.issue_number,
                        branch,
                        path,
                        "blocked",
                        "existing UAT fix worktree has uncommitted changes",
                    )
                )
            else:
                results.append(
                    UatFixWorktreeResult(
                        failure.issue_number,
                        branch,
                        path,
                        "reused",
                        "existing clean UAT fix worktree reused",
                    )
                )
            continue
        if branch_exists(branch, runner):
            cmd = ["git", "worktree", "add", str(path), branch]
        else:
            cmd = ["git", "worktree", "add", "-b", branch, str(path), DEFAULT_BASE]
        runner(cmd, cwd=REPO_ROOT, check=True)
        results.append(
            UatFixWorktreeResult(
                failure.issue_number,
                branch,
                path,
                "created",
                "UAT fix worktree created",
            )
        )
    return results


def render_uat_fix_worktree_report(results: list[UatFixWorktreeResult]) -> str:
    lines = ["# UAT Fix Worktrees", ""]
    if not results:
        return "# UAT Fix Worktrees\n\nNo UAT fix worktrees requested.\n"
    for result in results:
        lines.extend(
            [
                f"## Issue #{result.issue_number}",
                "",
                f"- Branch: `{result.branch_name}`",
                f"- Worktree: `{result.worktree_path}`",
                f"- Status: `{result.status}`",
                f"- Message: {result.message}",
                "",
            ]
        )
    return "\n".join(lines)


def write_final_report(run_dir: Path, analyses: list[IssueAnalysis]) -> None:
    lines = ["# Final Report", "", "## Issues", ""]
    for analysis in analyses:
        lines.append(f"- Issue #{analysis.issue.number}: {analysis.issue.title}")
    lines.extend(
        [
            "",
            "## Status",
            "",
            "Generated by current orchestration slice. Review phase-specific reports for details.",
            "",
        ]
    )
    (run_dir / "final-report.md").write_text("\n".join(lines), encoding="utf-8")


def emit_photon_event(
    base_url: str,
    *,
    event_kind: str,
    run_id: str,
    payload: dict[str, object],
) -> PhotonEventResult:
    if not base_url:
        return PhotonEventResult(event_kind, "skipped", "PHOTON URL not configured")
    body = json.dumps(
        {
            "schema_version": "codex-orchestrate.v1",
            "event_kind": event_kind,
            "run_id": run_id,
            "payload": sanitize_event_payload(payload),
        }
    ).encode("utf-8")
    request = urllib.request.Request(
        base_url.rstrip("/") + "/v1/events",
        data=body,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(request, timeout=2) as response:
            status = getattr(response, "status", 0)
        return PhotonEventResult(event_kind, "sent", f"HTTP {status}")
    except (OSError, urllib.error.URLError, TimeoutError) as exc:
        return PhotonEventResult(event_kind, "warning", f"PHOTON event failed: {exc}")


def sanitize_event_payload(value: object) -> object:
    if isinstance(value, dict):
        return {str(key): sanitize_event_payload(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [sanitize_event_payload(item) for item in value]
    if isinstance(value, str):
        return redact_paths(value)
    return value


def redact_paths(value: str) -> str:
    return re.sub(
        r"/(?:Users|home|tmp|private|var)/[^\s`'\"]+", "[REDACTED_PATH]", value
    )


def render_photon_events(results: list[PhotonEventResult]) -> str:
    lines = ["# PHOTON Events", ""]
    if not results:
        return "# PHOTON Events\n\nNot configured.\n"
    for result in results:
        lines.extend(
            [
                f"- `{result.event_kind}`: `{result.status}` - {result.message}",
            ]
        )
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    args = parse_args()
    repo_name = commandmate_repository_name()
    worktree_rows = parse_worktree_rows(args.worktree_row, args.issues)
    issues = load_issues(args.issues, args.issue_json, args.repo)
    analyses = [
        analyze_issue(issue, repo_name, skip_enhance=args.skip_enhance)
        for issue in issues
    ]
    analyses = apply_worktree_rows(analyses, worktree_rows, repo_name)
    work_item_issues = [analysis.issue.number for analysis in analyses]
    dependency_overrides = parse_dependency_overrides(
        args.dependency_override, work_item_issues
    )
    issue_decisions = parse_issue_decisions(args.issue_decision, work_item_issues)
    analyses = apply_planning_overrides(analyses, dependency_overrides, issue_decisions)
    if args.close_issues:
        included = [
            issue_number
            for analysis in analyses
            for issue_number in included_issue_numbers(analysis)
        ]
        duplicates = sorted(
            issue_number
            for issue_number in set(included)
            if included.count(issue_number) > 1
        )
        if duplicates:
            formatted = ", ".join(f"#{number}" for number in duplicates)
            raise ValueError(
                "--close-issues cannot be used when Issues span multiple worktree "
                f"rows; split Issues: {formatted}"
            )
    batches, issue_merge_order = classify_batches(
        analyses, args.merge_order, max_parallel=args.max_parallel
    )
    run_dir = write_artifacts(args, analyses)
    dry_run = bool(args.dry_run)
    photon_results: list[PhotonEventResult] = [
        emit_photon_event(
            args.photon_url,
            event_kind="orchestrate.started",
            run_id=run_dir.name,
            payload={"issues": args.issues, "dry_run": dry_run, "phase": args.phase},
        ),
        emit_photon_event(
            args.photon_url,
            event_kind="issue.analysis.completed",
            run_id=run_dir.name,
            payload={
                "issues": [
                    {
                        "number": analysis.issue.number,
                        "enhancement_needed": analysis.enhancement_needed,
                        "suspected_files": list(analysis.suspected_files),
                    }
                    for analysis in analyses
                ]
            },
        ),
    ]

    if args.apply_issue_enhancements:
        issue_results = apply_issue_enhancements(analyses, dry_run=dry_run)
        (run_dir / "issue-enhancement-report.md").write_text(
            render_issue_enhancement_report(issue_results),
            encoding="utf-8",
        )

    worktree_results: list[WorktreeResult] = []
    dispatch_results: list[WorkerSessionResult] = []
    wait_results: list[WorkerWaitResult] = []
    scheduler_results: list[SchedulerBatchResult] = []
    publish_requested = (
        args.create_prs
        or args.merge_prs
        or (not dry_run and phase_at_least(args.phase, "pr"))
    )
    merge_requested = args.merge_prs or (
        not dry_run and phase_at_least(args.phase, "merge")
    )
    if args.create_worktrees or (not dry_run and phase_at_least(args.phase, "dev")):
        worktree_results = create_or_reuse_worktrees(analyses, dry_run=dry_run)
        dispatch_results, wait_results, scheduler_results = (
            schedule_commandmate_batches(
                analyses,
                worktree_results,
                batches,
                max_parallel=args.max_parallel,
                dry_run=dry_run,
                dispatch_enabled=args.dispatch_commandmate,
                duration=args.commandmate_duration,
                codex_agent_name=args.codex_agent_name,
                poll=args.poll_commandmate,
                timeout_seconds=args.wait_commandmate_timeout,
                stall_timeout_seconds=args.wait_commandmate_stall_timeout,
                resume_completed=args.resume_completed_workers,
            )
        )
        (run_dir / "worker-sessions.md").write_text(
            render_worker_sessions(worktree_results, dispatch_results),
            encoding="utf-8",
        )
        (run_dir / "scheduler-report.md").write_text(
            render_scheduler_report(batches, args.max_parallel, scheduler_results),
            encoding="utf-8",
        )
        (run_dir / "commandmate-wait-report.md").write_text(
            render_worker_wait_report(wait_results),
            encoding="utf-8",
        )
        for result in dispatch_results:
            if result.status == "blocked":
                photon_results.append(
                    emit_photon_event(
                        args.photon_url,
                        event_kind="worker.blocked",
                        run_id=run_dir.name,
                        payload={
                            "issue_number": result.issue_number,
                            "worktree_id": result.worktree_id,
                            "message": result.message,
                        },
                    )
                )
            elif result.status in {"sent", "processing", "planned", "started-but-idle"}:
                photon_results.append(
                    emit_photon_event(
                        args.photon_url,
                        event_kind="worker.started",
                        run_id=run_dir.name,
                        payload={
                            "issue_number": result.issue_number,
                            "worktree_id": result.worktree_id,
                            "status": result.status,
                        },
                    )
                )

    can_publish = True
    if publish_requested and dispatch_results:
        can_publish = all(result.status != "blocked" for result in dispatch_results)
    if publish_requested and not dry_run and args.dispatch_commandmate:
        can_publish = (
            can_publish
            and len(scheduler_results) == len(batches)
            and all(result.status == "completed" for result in scheduler_results)
        )

    worker_verification_results: list[WorkerVerificationResult] = []
    if publish_requested:
        worker_verification_results = verify_worker_reports(
            analyses,
            worktree_results,
            dry_run=dry_run,
        )
        (run_dir / "worker-verification.md").write_text(
            render_worker_verification_report(worker_verification_results),
            encoding="utf-8",
        )
        allowed_verification_statuses = {"planned"} if dry_run else {"passed"}
        can_publish = (
            can_publish
            and bool(worker_verification_results)
            and all(
                result.status in allowed_verification_statuses
                for result in worker_verification_results
            )
        )

    pr_results: list[PullRequestResult] = []
    if publish_requested and can_publish:
        pr_results = create_pull_requests(
            analyses,
            run_id=run_dir.name,
            dry_run=dry_run,
        )
        (run_dir / "pr-report.md").write_text(
            render_pr_report(pr_results), encoding="utf-8"
        )
        allowed_pr_statuses = {"planned"} if dry_run else {"created", "existing"}
        can_publish = bool(pr_results) and all(
            result.status in allowed_pr_statuses for result in pr_results
        )
    elif publish_requested:
        (run_dir / "pr-report.md").write_text(
            "# PR Report\n\nSkipped because worker completion or verification did not pass.\n",
            encoding="utf-8",
        )

    requested_pr_numbers = [
        int(part.strip()) for part in args.pr_numbers.split(",") if part.strip()
    ]
    pr_numbers = order_pr_numbers_for_merge(
        pr_results, issue_merge_order, requested_pr_numbers
    )
    if dry_run and publish_requested and not pr_numbers:
        pr_numbers = issue_merge_order
    issue_to_pr = dict(zip(issue_merge_order, pr_numbers))

    uat_gate = UatGateResult("not-requested", "UAT phase was not requested")
    uat_requested = (
        args.write_uat
        or merge_requested
        or (not dry_run and phase_at_least(args.phase, "uat"))
    )
    expected_head_by_issue: dict[int, str] = {}
    if uat_requested:
        loaded_uat_results = load_uat_results(args.uat_results_json)
        uat_results, ignored_uat_results = scope_uat_results(
            loaded_uat_results,
            [analysis.issue.number for analysis in analyses],
            allow_superset=args.allow_uat_superset,
        )
        failures = load_uat_failures(args.uat_failures_json)
        ci_results = (
            wait_for_all_pr_checks(pr_numbers, dry_run=dry_run)
            if can_publish and pr_numbers
            else []
        )
        (run_dir / "ci-report.md").write_text(
            render_ci_report(ci_results), encoding="utf-8"
        )
        allowed_ci_statuses = {"planned"} if dry_run else {"passed"}
        ci_passed = bool(ci_results) and all(
            result.status in allowed_ci_statuses for result in ci_results
        )
        head_lookup_error = ""
        if ci_passed and merge_requested and not dry_run:
            try:
                expected_head_by_issue = {
                    issue_number: read_pr_head_oid(pr_number)
                    for issue_number, pr_number in issue_to_pr.items()
                }
            except (RuntimeError, ValueError, json.JSONDecodeError) as exc:
                head_lookup_error = str(exc)
        if ci_passed:
            if head_lookup_error:
                uat_gate = UatGateResult("blocked", head_lookup_error)
            else:
                uat_gate = evaluate_uat_gate(
                    analyses,
                    uat_results,
                    require_complete=merge_requested,
                    dry_run=dry_run,
                    expected_head_by_issue=expected_head_by_issue,
                    require_head_binding=merge_requested and not dry_run,
                )
        else:
            uat_gate = UatGateResult(
                "planned" if dry_run else "blocked",
                "UAT cannot proceed until every PR passes CI",
            )
        if failures and not dry_run:
            uat_gate = UatGateResult(
                "blocked", f"{len(failures)} explicit UAT failures recorded"
            )
        (run_dir / "uat-report.md").write_text(
            render_uat_report(
                analyses,
                uat_results,
                uat_gate,
                ignored_out_of_scope_results=ignored_uat_results,
            ),
            encoding="utf-8",
        )
        (run_dir / "uat-fix-prompts.md").write_text(
            render_uat_fix_prompts(failures, analyses),
            encoding="utf-8",
        )
        if args.create_uat_fix_worktrees:
            fix_results = create_uat_fix_worktrees(failures, dry_run=dry_run)
            (run_dir / "uat-fix-worktrees.md").write_text(
                render_uat_fix_worktree_report(fix_results),
                encoding="utf-8",
            )
        for failure in failures:
            photon_results.append(
                emit_photon_event(
                    args.photon_url,
                    event_kind="uat.failed",
                    run_id=run_dir.name,
                    payload={
                        "issue_number": failure.issue_number,
                        "scenario": failure.scenario,
                        "expected": failure.expected,
                        "actual": failure.actual,
                    },
                )
            )
        if uat_gate.status == "passed":
            photon_results.append(
                emit_photon_event(
                    args.photon_url,
                    event_kind="uat.passed",
                    run_id=run_dir.name,
                    payload={"issues": args.issues, "status": uat_gate.message},
                )
            )
        elif uat_gate.status == "blocked" and not failures:
            photon_results.append(
                emit_photon_event(
                    args.photon_url,
                    event_kind="uat.failed",
                    run_id=run_dir.name,
                    payload={"issues": args.issues, "reason": uat_gate.message},
                )
            )

    merge_results: list[MergeResult] = []
    if merge_requested and can_publish:
        if not pr_numbers:
            raise ValueError("merge phase requires created/existing PR numbers")
        merge_results = merge_pull_requests(
            pr_numbers,
            dry_run=dry_run,
            merge_method=args.merge_method,
            integration_checks=args.integration_check,
            uat_gate=uat_gate,
            expected_head_by_pr={
                pr_number: expected_head_by_issue[issue_number]
                for issue_number, pr_number in issue_to_pr.items()
                if issue_number in expected_head_by_issue
            },
        )
        (run_dir / "merge-report.md").write_text(
            render_merge_report(merge_results), encoding="utf-8"
        )
        (run_dir / "merge-recovery-report.md").write_text(
            render_merge_recovery_report(merge_results), encoding="utf-8"
        )
        issue_to_pr_for_close = {
            issue_number: issue_to_pr[analysis.issue.number]
            for analysis in analyses
            if analysis.issue.number in issue_to_pr
            for issue_number in included_issue_numbers(analysis)
        }
        issue_close_results = close_merged_issues(
            issue_to_pr_for_close,
            merge_results,
            authorized=args.close_issues,
            dry_run=dry_run,
        )
        (run_dir / "issue-close-report.md").write_text(
            render_issue_close_report(issue_close_results), encoding="utf-8"
        )
        for result in merge_results:
            if result.status not in {"merged", "blocked"}:
                continue
            photon_results.append(
                emit_photon_event(
                    args.photon_url,
                    event_kind="pr.merged"
                    if result.status == "merged"
                    else "verification.failed",
                    run_id=run_dir.name,
                    payload={
                        "pr_number": result.pr_number,
                        "status": result.status,
                        "message": result.message,
                    },
                )
            )
    elif merge_requested:
        (run_dir / "merge-report.md").write_text(
            "# Merge Report\n\nSkipped because worker completion or verification did not pass.\n",
            encoding="utf-8",
        )
        (run_dir / "merge-recovery-report.md").write_text(
            "# Merge Recovery Report\n\nMerge did not start because an earlier gate blocked publication.\n",
            encoding="utf-8",
        )
        (run_dir / "issue-close-report.md").write_text(
            "# Issue Close Report\n\nNo Issue was closed because no PR was merged.\n",
            encoding="utf-8",
        )

    write_final_report(run_dir, analyses)
    photon_results.append(
        emit_photon_event(
            args.photon_url,
            event_kind="orchestrate.completed",
            run_id=run_dir.name,
            payload={"issues": args.issues},
        )
    )
    (run_dir / "photon-events.md").write_text(
        render_photon_events(photon_results), encoding="utf-8"
    )
    print(run_dir)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
