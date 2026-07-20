from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path
from types import ModuleType

import pytest

SCRIPT_PATH = Path(__file__).resolve().parents[1] / "scripts" / "codex_orchestrate.py"


def load_script() -> ModuleType:
    spec = importlib.util.spec_from_file_location("codex_orchestrate", SCRIPT_PATH)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def test_slugify_keeps_safe_branch_text() -> None:
    module = load_script()

    assert module.slugify("Add Codex Harness!") == "add-codex-harness"
    assert module.slugify("!!!") == "task"


def test_analyze_issue_extracts_acceptance_files_and_tests() -> None:
    module = load_script()
    issue = module.Issue(
        number=12,
        title="Add dry-run planner",
        body=(
            "Implement planner for `scripts/codex_orchestrate.py`.\n\n"
            "## Acceptance Criteria\n"
            "- `pytest -q` passes\n"
            "- writes `workspace/management/runs/example/manifest.md`\n"
        ),
        labels=("feature",),
    )

    analysis = module.analyze_issue(issue, "CommandAgent", skip_enhance=False)

    assert analysis.enhancement_needed is False
    assert analysis.branch_name == "feature/issue-12-add-dry-run-planner"
    assert "scripts/codex_orchestrate.py" in analysis.suspected_files
    assert "pytest" in analysis.test_expectations
    assert analysis.acceptance_criteria


def test_analyze_issue_extracts_nested_japanese_acceptance_checklists() -> None:
    module = load_script()
    issue = module.Issue(
        number=10,
        title="Modernize REPL input",
        body=(
            "# 背景 / 目的\n"
            "`src/tui/repl.rs` の入力体験を改善する。\n\n"
            "# 要求仕様(受け入れ基準)\n\n"
            "## 1. Tab補完\n"
            "- [ ] スラッシュコマンドを前方一致で補完する。\n\n"
            "## 2. Ctrl+C 作法\n"
            "- [x] 空行で連続2回押すと正常終了する。\n"
            "### 状態リセット\n"
            "1. 間に他のキー入力があればカウントをリセットする。\n\n"
            "# 実装ガイド\n"
            "- 新規モジュールに実装する。\n"
        ),
    )

    analysis = module.analyze_issue(issue, "CommandAgent", skip_enhance=False)

    assert analysis.acceptance_criteria == (
        "スラッシュコマンドを前方一致で補完する。",
        "空行で連続2回押すと正常終了する。",
        "間に他のキー入力があればカウントをリセットする。",
    )
    assert analysis.enhancement_needed is False
    assert analysis.questions == ()


def test_analyze_issue_skips_markdown_heading_for_objective() -> None:
    module = load_script()
    issue = module.Issue(
        number=2,
        title="[P0][M1] Define v1 sidecar schema",
        body=(
            "## 概要\n"
            "`workspace/v0.1.0/05_development_preparation_plan.md` の M1 Schema First に従い、"
            "v1 sidecar schema を実装する。\n\n"
            "## 完了条件\n"
            "- `schema_version` が request / response / event に必須である\n"
        ),
    )

    analysis = module.analyze_issue(issue, "CommandAgent", skip_enhance=False)

    assert analysis.objective.startswith(
        "`workspace/v0.1.0/05_development_preparation_plan.md`"
    )
    assert analysis.objective != "概要"


def test_extract_file_candidates_ignores_absolute_path_fragments() -> None:
    module = load_script()
    candidates = module.extract_file_candidates(
        "参照: `/Users/me/repo/external/scripts/export_agent_training_data.py`\n"
        "対象: `src/memory/sanitizer.py`\n"
    )

    assert (
        "Users/me/repo/external/scripts/export_agent_training_data.py" not in candidates
    )
    assert "src/memory/sanitizer.py" in candidates


def test_classify_file_candidates_splits_external_references() -> None:
    module = load_script()

    suspected, references = module.classify_file_candidates(
        [
            "photon-mlx-develop/scripts/export_agent_training_data.py",
            "src/memory/sanitizer.py",
        ]
    )

    assert suspected == ["src/memory/sanitizer.py"]
    assert references == ["photon-mlx-develop/scripts/export_agent_training_data.py"]


def test_enrich_file_candidates_uses_deterministic_filtered_order(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    module = load_script()
    paths = [
        "src/zeta.rs",
        "workspace/management/runs/noise.md",
        "src/echo.rs",
        "src/alpha.rs",
        "src/delta.rs",
        "src/charlie.rs",
        "src/bravo.rs",
    ]

    def fake_runner(args, **kwargs):  # type: ignore[no-untyped-def]
        return module.subprocess.CompletedProcess(args, 0, "\n".join(paths), "")

    monkeypatch.setattr(module, "run_command", fake_runner)

    enriched = module.enrich_file_candidates_with_rg("UniqueSearchTerm", [])

    assert enriched == [
        "src/alpha.rs",
        "src/bravo.rs",
        "src/charlie.rs",
        "src/delta.rs",
        "src/echo.rs",
    ]


def test_issue_2_to_5_dependency_batches_are_not_fully_serial() -> None:
    module = load_script()
    issues = [
        module.Issue(2, "[P0][M1] Define v1 sidecar schema", "schema EventRecord"),
        module.Issue(
            3, "[P0][M3] Implement sanitizer module", "sanitizer redact secret token"
        ),
        module.Issue(
            4,
            "[P0][M2] Implement local SQLite event store",
            "SQLite local-first storage",
        ),
        module.Issue(
            5,
            "[P0][M2] Implement sidecar health/events/suggest",
            "FastAPI sidecar endpoint client",
        ),
    ]
    analyses = [
        module.analyze_issue(issue, "CommandAgent", skip_enhance=True)
        for issue in issues
    ]

    batches, merge_order = module.classify_batches(analyses, "")

    assert batches[0] == [2, 3]
    assert batches[1:] == [[4], [5]]
    assert merge_order == [2, 3, 4, 5]


def test_dependency_batches_enforce_configured_max_parallel() -> None:
    module = load_script()
    analyses = [
        module.analyze_issue(
            module.Issue(
                number,
                f"Independent {number}",
                f"Implement `src/independent_{number}.rs`",
            ),
            "CommandAgent",
            skip_enhance=True,
        )
        for number in range(1, 6)
    ]

    batches, merge_order = module.classify_batches(analyses, "", max_parallel=2)

    assert batches == [[1, 2], [3, 4], [5]]
    assert merge_order == [1, 2, 3, 4, 5]
    assert all(len(batch) <= 2 for batch in batches)


def test_explicit_merge_order_cannot_precede_dependency() -> None:
    module = load_script()
    analyses = [
        module.analyze_issue(
            module.Issue(2, "Define contract", "schema contract"),
            "CommandAgent",
            skip_enhance=True,
        ),
        module.analyze_issue(
            module.Issue(4, "Add storage", "SQLite storage"),
            "CommandAgent",
            skip_enhance=True,
        ),
    ]

    with pytest.raises(ValueError, match="before dependencies #2"):
        module.classify_batches(analyses, "4,2", max_parallel=2)


def test_parser_rejects_non_positive_max_parallel() -> None:
    module = load_script()

    with pytest.raises(SystemExit):
        module.parse_args_from_list_for_test(["1", "--max-parallel", "0"])


def test_dependency_overrides_replace_inference_with_complete_explicit_graph() -> None:
    module = load_script()
    issues = [
        module.Issue(number, f"Issue {number}", f"Update `src/issue_{number}.rs`")
        for number in range(11, 18)
    ]
    analyses = [
        module.replace(
            module.analyze_issue(issue, "CommandAgent", skip_enhance=True),
            suspected_files=(f"src/issue_{issue.number}.rs",),
        )
        for issue in issues
    ]
    overrides = module.parse_dependency_overrides(
        [
            "11:",
            "12:11,13,14",
            "13:11",
            "14:15",
            "15:",
            "16:15",
            "17:16",
        ],
        list(range(11, 18)),
    )

    analyses = module.apply_planning_overrides(analyses, overrides, {})
    batches, merge_order = module.classify_batches(analyses, "", max_parallel=2)

    assert batches == [[11, 15], [13, 14], [12, 16], [17]]
    assert merge_order == [11, 15, 13, 14, 12, 16, 17]
    issue_15 = next(item for item in analyses if item.issue.number == 15)
    assert module.direct_dependencies(issue_15, analyses) == []
    assert module.dependency_reason(issue_15, analyses) == (
        "explicitly has no dependencies"
    )


def test_dependency_overrides_require_every_requested_issue() -> None:
    module = load_script()

    with pytest.raises(ValueError, match="missing #12"):
        module.parse_dependency_overrides(["11:"], [11, 12])


def test_dependency_overrides_reject_cycles() -> None:
    module = load_script()
    analyses = [
        module.analyze_issue(
            module.Issue(number, f"Issue {number}", f"Update `src/{number}.rs`"),
            "CommandAgent",
            skip_enhance=True,
        )
        for number in (11, 12)
    ]
    overrides = module.parse_dependency_overrides(["11:12", "12:11"], [11, 12])
    analyses = module.apply_planning_overrides(analyses, overrides, {})

    with pytest.raises(ValueError, match="dependency cycle"):
        module.classify_batches(analyses, "", max_parallel=2)


def test_issue_decision_becomes_acceptance_and_worker_instruction() -> None:
    module = load_script()

    def no_rg(*args, **kwargs):  # type: ignore[no-untyped-def]
        raise FileNotFoundError

    module.run_command = no_rg
    analysis = module.analyze_issue(
        module.Issue(17, "Choose protocol naming", "Record the selected option."),
        "CommandAgent",
        skip_enhance=False,
    )
    decisions = module.parse_issue_decisions(
        ["17:Adopt Option A and update only docs/mechanism-ledger.md."], [17]
    )

    updated = module.apply_planning_overrides([analysis], {17: ()}, decisions)[0]
    prompt = module.build_worker_prompt(updated)

    assert updated.approved_decision.startswith("Adopt Option A")
    assert updated.enhancement_needed is False
    assert updated.questions == ()
    assert updated.suspected_files == ("docs/mechanism-ledger.md",)
    assert updated.acceptance_criteria == (
        "Apply approved decision: Adopt Option A and update only "
        "docs/mechanism-ledger.md.",
    )
    assert "## Approved Decision" in prompt
    assert "Adopt Option A and update only docs/mechanism-ledger.md." in prompt


def test_issue_decision_without_scope_keeps_scope_question() -> None:
    module = load_script()

    def no_rg(*args, **kwargs):  # type: ignore[no-untyped-def]
        raise FileNotFoundError

    module.run_command = no_rg
    analysis = module.analyze_issue(
        module.Issue(17, "Choose protocol naming", "Record the selected option."),
        "CommandAgent",
        skip_enhance=False,
    )
    decisions = module.parse_issue_decisions(["17:Adopt Option A."], [17])

    updated = module.apply_planning_overrides([analysis], {17: ()}, decisions)[0]

    assert updated.enhancement_needed is True
    assert updated.questions == (
        "影響範囲を特定できません。想定している機能領域やファイルがあれば教えてください。",
    )
    assert updated.suspected_files == ()


def test_build_issue_body_with_orchestration_notes_is_idempotent() -> None:
    module = load_script()
    issue = module.Issue(
        number=9,
        title="Clarify issue",
        body="Original body\n\n## 完了条件\n- Works\n",
    )
    analysis = module.analyze_issue(issue, "CommandAgent", skip_enhance=False)

    first = module.build_issue_body_with_orchestration_notes(analysis)
    second = module.build_issue_body_with_orchestration_notes(
        module.IssueAnalysis(
            issue=module.Issue(issue.number, issue.title, first, issue.labels),
            objective=analysis.objective,
            acceptance_criteria=analysis.acceptance_criteria,
            suspected_files=analysis.suspected_files,
            reference_files=analysis.reference_files,
            test_expectations=analysis.test_expectations,
            enhancement_needed=analysis.enhancement_needed,
            questions=analysis.questions,
            branch_name=analysis.branch_name,
            worktree_path=analysis.worktree_path,
            dependency_hints=analysis.dependency_hints,
        )
    )

    assert first == second
    assert first.count("<!-- codex-orchestrate-notes -->") == 1


def test_write_artifacts_from_fixture(tmp_path: Path) -> None:
    module = load_script()
    fixture = tmp_path / "issues.json"
    fixture.write_text(
        json.dumps(
            {
                "issues": [
                    {
                        "number": 1,
                        "title": "Update schema docs",
                        "body": "Touch `workspace/management/codex_harness_spec.md`.\n",
                        "labels": ["docs"],
                    },
                    {
                        "number": 2,
                        "title": "Add script tests",
                        "body": (
                            "Update `tests/test_codex_orchestrate.py`.\n\n"
                            "## Acceptance Criteria\n- pytest passes\n"
                        ),
                        "labels": ["test"],
                    },
                ]
            }
        ),
        encoding="utf-8",
    )

    args = module.parse_args_from_list_for_test(
        [
            "1",
            "2",
            "--dry-run",
            "--issue-json",
            str(fixture),
            "--run-id",
            "test-run",
            "--runs-dir",
            str(tmp_path / "runs"),
            "--max-parallel",
            "1",
        ]
    )
    issues = module.load_issues(args.issues, args.issue_json)
    analyses = [
        module.analyze_issue(issue, "CommandAgent", skip_enhance=args.skip_enhance)
        for issue in issues
    ]
    run_dir = module.write_artifacts(args, analyses)

    assert (run_dir / "manifest.md").exists()
    assert (run_dir / "issue-analysis.md").exists()
    assert (run_dir / "dependency-plan.md").exists()
    assert (run_dir / "scheduler-report.md").exists()
    assert "Issue #1" in (run_dir / "issue-analysis.md").read_text(encoding="utf-8")
    assert "CommandMate Codex agent: `codex`" in (run_dir / "manifest.md").read_text(
        encoding="utf-8"
    )
    dependency_plan = (run_dir / "dependency-plan.md").read_text(encoding="utf-8")
    assert "Batch 1: #1" in dependency_plan
    assert "Batch 2: #2" in dependency_plan


def test_worktree_planning_does_not_mutate_in_dry_run() -> None:
    module = load_script()
    issue = module.Issue(
        number=3,
        title="Add worktree manager",
        body="Update `scripts/codex_orchestrate.py`.\n\n## Acceptance Criteria\n- dry-run only\n",
    )
    analysis = module.analyze_issue(issue, "CommandAgent", skip_enhance=False)

    results = module.create_or_reuse_worktrees([analysis], dry_run=True)

    assert results[0].status == "planned"
    assert results[0].branch_name == "feature/issue-3-add-worktree-manager"


def test_parser_defaults_to_codex_agent() -> None:
    module = load_script()

    args = module.parse_args_from_list_for_test(["1", "--dry-run"])

    assert args.codex_agent_name == "codex"
    assert args.merge_method == "merge"
    assert args.phase == "plan"
    assert module.PHASE_ORDER["pr"] < module.PHASE_ORDER["uat"]
    assert module.PHASE_ORDER["uat"] < module.PHASE_ORDER["merge"]


def test_commandmate_send_command_selects_codex_agent() -> None:
    module = load_script()

    cmd = module.build_commandmate_send_command(
        "repo-issue-1",
        "hello",
        duration="3h",
        codex_agent_name="codex",
    )

    assert cmd == [
        "commandmatedev",
        "send",
        "repo-issue-1",
        "hello",
        "--agent",
        "codex",
        "--auto-yes",
        "--duration",
        "3h",
    ]


def test_dispatch_commandmate_sends_only_worker_task() -> None:
    module = load_script()
    issue = module.Issue(
        number=1,
        title="Add worker task",
        body="Implement the issue.\n\n## Acceptance Criteria\n- Done\n",
    )
    analysis = module.analyze_issue(issue, "CommandAgent", skip_enhance=True)
    worktree = module.WorktreeResult(
        issue_number=1,
        branch_name="feature/issue-1-add-worker-task",
        worktree_path=Path("/tmp/CommandAgent-issue-1-add-worker-task"),
        status="created",
        message="worktree created",
    )
    calls: list[list[str]] = []

    def fake_runner(args, **kwargs):  # type: ignore[no-untyped-def]
        calls.append(args)
        return module.subprocess.CompletedProcess(args, 0, "", "")

    results = module.dispatch_commandmate(
        [analysis],
        [worktree],
        dry_run=False,
        duration="3h",
        codex_agent_name="codex",
        poll=False,
        runner=fake_runner,
    )

    assert len(calls) == 1
    assert calls[0][:3] == [
        "commandmatedev",
        "send",
        "commandagent-feature-issue-1-add-worker-task",
    ]
    assert calls[0][-5:] == ["--agent", "codex", "--auto-yes", "--duration", "3h"]
    assert "$codex-issue-worker" in calls[0][3]
    assert "- Status: `passed`" in calls[0][3]
    assert calls[0][3] != "hello"
    assert results[0].commands == (" ".join(calls[0]),)


def test_dispatch_commandmate_reports_send_failure_as_blocked() -> None:
    module = load_script()
    issue = module.Issue(
        number=1,
        title="Add worker task",
        body="Implement the issue.\n\n## Acceptance Criteria\n- Done\n",
    )
    analysis = module.analyze_issue(issue, "CommandAgent", skip_enhance=True)
    worktree = module.WorktreeResult(
        issue_number=1,
        branch_name="feature/issue-1-add-worker-task",
        worktree_path=Path("/tmp/CommandAgent-issue-1-add-worker-task"),
        status="created",
        message="worktree created",
    )

    def fake_runner(args, **kwargs):  # type: ignore[no-untyped-def]
        raise module.subprocess.CalledProcessError(
            99,
            args,
            output="",
            stderr="Error: Resource not found. Check the worktree ID.",
        )

    results = module.dispatch_commandmate(
        [analysis],
        [worktree],
        dry_run=False,
        duration="3h",
        codex_agent_name="codex",
        poll=False,
        runner=fake_runner,
    )

    assert results[0].status == "blocked"
    assert results[0].message == "Error: Resource not found. Check the worktree ID."


def test_commandmate_ls_command_omits_empty_branch_prefix() -> None:
    module = load_script()

    assert module.build_commandmate_ls_command(branch_prefix="feature/issue-") == [
        "commandmatedev",
        "ls",
        "--branch",
        "feature/issue-",
        "--json",
    ]
    assert module.build_commandmate_ls_command(branch_prefix=None) == [
        "commandmatedev",
        "ls",
        "--json",
    ]
    assert module.build_commandmate_ls_command(branch_prefix="") == [
        "commandmatedev",
        "ls",
        "--json",
    ]


def test_commandmate_worktree_id_uses_commandmate_branch_format() -> None:
    module = load_script()

    assert (
        module.commandmate_worktree_id("feature/issue-2-p0-m1-define-v1-sidecar-schema")
        == "commandagent-feature-issue-2-p0-m1-define-v1-sidecar-schema"
    )


def test_commandmate_repository_name_strips_issue_worktree_suffix(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    module = load_script()
    monkeypatch.setattr(
        module,
        "REPO_ROOT",
        Path("/tmp/CommandAgent-issue-2-p0-m1-define-v1-sidecar-schema"),
    )

    assert module.commandmate_repository_name() == "CommandAgent"


def test_commandmate_repository_name_strips_develop_worktree_suffix(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    module = load_script()
    monkeypatch.setattr(module, "REPO_ROOT", Path("/tmp/CommandAgent-develop"))

    assert module.commandmate_repository_name() == "CommandAgent"


def test_poll_worker_startup_reports_started_idle_without_prompting() -> None:
    module = load_script()
    calls: list[list[str]] = []

    def fake_runner(args, **kwargs):  # type: ignore[no-untyped-def]
        calls.append(args)
        if args == ["commandmatedev", "ls", "--json"]:
            return module.subprocess.CompletedProcess(
                args,
                0,
                json.dumps(
                    {
                        "worktrees": [
                            {
                                "id": "repo-issue-1",
                                "status": "running",
                                "isProcessing": False,
                            }
                        ]
                    }
                ),
                "",
            )
        return module.subprocess.CompletedProcess(args, 0, "", "")

    result = module.poll_worker_startup(
        1,
        "repo-issue-1",
        codex_agent_name="",
        commands=("hello", "task"),
        runner=fake_runner,
    )

    assert result.status == "started-but-idle"
    assert result.message == "worker session is running but not processing"
    assert result.running is True
    assert result.processing is False
    assert calls == [["commandmatedev", "ls", "--json"]]


def test_poll_worker_startup_reports_commandmate_unreachable() -> None:
    module = load_script()
    calls: list[list[str]] = []

    def fake_runner(args, **kwargs):  # type: ignore[no-untyped-def]
        calls.append(args)
        return module.subprocess.CompletedProcess(
            args,
            1,
            "",
            "Error: Server is not running. Start it with: commandmate start",
        )

    result = module.poll_worker_startup(
        1,
        "repo-issue-1",
        codex_agent_name="",
        commands=("hello", "task"),
        runner=fake_runner,
    )

    assert result.status == "blocked"
    assert result.message.startswith("commandmate-unreachable:")
    assert result.running is None
    assert result.processing is None
    assert calls == [["commandmatedev", "ls", "--json"]]


def test_wait_for_commandmate_workers_uses_wait_without_starting_server() -> None:
    module = load_script()
    calls: list[list[str]] = []
    sessions = [
        module.WorkerSessionResult(
            11,
            "repo-issue-11",
            "started-but-idle",
            False,
            True,
            "running",
            (),
        )
    ]

    def fake_runner(args, **kwargs):  # type: ignore[no-untyped-def]
        calls.append(args)
        return module.subprocess.CompletedProcess(args, 0, "completed\n", "")

    results = module.wait_for_commandmate_workers(
        sessions,
        timeout_seconds=600,
        stall_timeout_seconds=120,
        codex_agent_name="codex",
        runner=fake_runner,
    )

    assert results[0].status == "completed"
    assert calls == [
        [
            "commandmatedev",
            "wait",
            "repo-issue-11",
            "--timeout",
            "600",
            "--instance",
            "codex",
            "--stall-timeout",
            "120",
        ]
    ]


def test_wait_for_commandmate_workers_classifies_unreachable() -> None:
    module = load_script()
    sessions = [
        module.WorkerSessionResult(
            11,
            "repo-issue-11",
            "sent",
            None,
            None,
            "sent",
            (),
        )
    ]

    def fake_runner(args, **kwargs):  # type: ignore[no-untyped-def]
        return module.subprocess.CompletedProcess(
            args,
            1,
            "",
            "Error: Server is not running. Start it with: commandmate start",
        )

    results = module.wait_for_commandmate_workers(
        sessions,
        timeout_seconds=600,
        stall_timeout_seconds=0,
        codex_agent_name="codex",
        runner=fake_runner,
    )

    assert results[0].status == "blocked"
    assert results[0].message.startswith("commandmate-unreachable:")


def test_scheduler_dispatches_dependency_batches_only_after_verification(
    tmp_path: Path,
) -> None:
    module = load_script()
    issues = [
        module.Issue(2, "Define contract", "schema contract"),
        module.Issue(3, "Add sanitizer", "sanitizer redact secret"),
        module.Issue(4, "Add storage", "SQLite storage"),
    ]
    analyses = [
        module.analyze_issue(issue, "CommandAgent", skip_enhance=True)
        for issue in issues
    ]
    batches, _ = module.classify_batches(analyses, "", max_parallel=2)
    worktrees = []
    for analysis in analyses:
        root = tmp_path / f"issue-{analysis.issue.number}"
        report = (
            root / "dev-reports" / f"issue-{analysis.issue.number}" / "verification.md"
        )
        report.parent.mkdir(parents=True)
        report.write_text(
            "# Verification\n\n- Status: `passed`\n\n- `cargo test`: `passed`\n",
            encoding="utf-8",
        )
        worktrees.append(
            module.WorktreeResult(
                analysis.issue.number,
                analysis.branch_name,
                root,
                "created",
                "created",
            )
        )
    calls: list[list[str]] = []

    def fake_runner(args, **kwargs):  # type: ignore[no-untyped-def]
        calls.append(args)
        if args[:2] in (["commandmatedev", "send"], ["commandmatedev", "wait"]):
            return module.subprocess.CompletedProcess(args, 0, "completed\n", "")
        if args[:3] == ["git", "status", "--porcelain"]:
            return module.subprocess.CompletedProcess(args, 0, "", "")
        if args[:3] == ["git", "ls-files", "--error-unmatch"]:
            return module.subprocess.CompletedProcess(args, 0, "verification.md\n", "")
        return module.subprocess.CompletedProcess(args, 1, "", "unexpected")

    dispatch, waits, schedule = module.schedule_commandmate_batches(
        analyses,
        worktrees,
        batches,
        max_parallel=2,
        dry_run=False,
        dispatch_enabled=True,
        duration="3h",
        codex_agent_name="codex",
        poll=False,
        timeout_seconds=600,
        stall_timeout_seconds=0,
        runner=fake_runner,
    )

    assert batches == [[2, 3], [4]]
    assert [result.status for result in schedule] == ["completed", "completed"]
    assert len(dispatch) == 3
    assert len(waits) == 3
    send_4 = next(
        index
        for index, call in enumerate(calls)
        if call[:2] == ["commandmatedev", "send"] and "issue-4" in call[2]
    )
    waits_for_first_batch = [
        index
        for index, call in enumerate(calls)
        if call[:2] == ["commandmatedev", "wait"]
        and ("issue-2" in call[2] or "issue-3" in call[2])
    ]
    assert waits_for_first_batch
    assert max(waits_for_first_batch) < send_4
    send_4_prompt = next(
        call[3]
        for call in calls
        if call[:2] == ["commandmatedev", "send"] and "issue-4" in call[2]
    )
    assert "Issue #2" in send_4_prompt
    assert "Issue #3" in send_4_prompt
    assert "passed verification" in send_4_prompt


def test_scheduler_stops_later_batches_after_wait_failure(tmp_path: Path) -> None:
    module = load_script()
    analyses = [
        module.analyze_issue(
            module.Issue(number, f"Issue {number}", "independent"),
            "CommandAgent",
            skip_enhance=True,
        )
        for number in (1, 2)
    ]
    worktrees = [
        module.WorktreeResult(
            analysis.issue.number,
            analysis.branch_name,
            tmp_path / f"issue-{analysis.issue.number}",
            "created",
            "created",
        )
        for analysis in analyses
    ]
    calls: list[list[str]] = []

    def fake_runner(args, **kwargs):  # type: ignore[no-untyped-def]
        calls.append(args)
        if args[:2] == ["commandmatedev", "send"]:
            return module.subprocess.CompletedProcess(args, 0, "", "")
        if args[:2] == ["commandmatedev", "wait"]:
            return module.subprocess.CompletedProcess(args, 1, "", "worker failed")
        return module.subprocess.CompletedProcess(args, 1, "", "unexpected")

    dispatch, waits, schedule = module.schedule_commandmate_batches(
        analyses,
        worktrees,
        [[1], [2]],
        max_parallel=1,
        dry_run=False,
        dispatch_enabled=True,
        duration="3h",
        codex_agent_name="codex",
        poll=False,
        timeout_seconds=600,
        stall_timeout_seconds=0,
        runner=fake_runner,
    )

    sends = [call for call in calls if call[:2] == ["commandmatedev", "send"]]
    assert len(sends) == 1
    assert "issue-1" in sends[0][2]
    assert len(waits) == 1
    assert [result.status for result in schedule] == ["blocked", "blocked"]
    issue_2 = next(result for result in dispatch if result.issue_number == 2)
    assert issue_2.status == "blocked"
    assert "not dispatched because scheduler batch 1 failed" in issue_2.message


def test_scheduler_resume_skips_only_committed_passing_workers(tmp_path: Path) -> None:
    module = load_script()
    analyses = [
        module.analyze_issue(
            module.Issue(number, f"Issue {number}", "independent"),
            "CommandAgent",
            skip_enhance=True,
        )
        for number in (1, 2)
    ]
    worktrees = []
    for analysis in analyses:
        root = tmp_path / f"issue-{analysis.issue.number}"
        root.mkdir()
        worktrees.append(
            module.WorktreeResult(
                analysis.issue.number,
                analysis.branch_name,
                root,
                "created",
                "created",
            )
        )
    report = tmp_path / "issue-1/dev-reports/issue-1/verification.md"
    report.parent.mkdir(parents=True)
    report.write_text(
        "# Verification\n\n- Status: `passed`\n\n- `cargo test`: `passed`\n",
        encoding="utf-8",
    )
    calls: list[list[str]] = []

    def fake_runner(args, **kwargs):  # type: ignore[no-untyped-def]
        calls.append(args)
        cwd = kwargs.get("cwd")
        if args[:3] == ["git", "status", "--porcelain"]:
            return module.subprocess.CompletedProcess(args, 0, "", "")
        if args[:3] == ["git", "ls-files", "--error-unmatch"]:
            return module.subprocess.CompletedProcess(
                args,
                0 if cwd == tmp_path / "issue-1" else 1,
                "verification.md\n" if cwd == tmp_path / "issue-1" else "",
                "",
            )
        if args[:2] in (["commandmatedev", "send"], ["commandmatedev", "wait"]):
            return module.subprocess.CompletedProcess(args, 0, "completed\n", "")
        return module.subprocess.CompletedProcess(args, 1, "", "unexpected")

    dispatch, waits, schedule = module.schedule_commandmate_batches(
        analyses,
        worktrees,
        [[1], [2]],
        max_parallel=1,
        dry_run=False,
        dispatch_enabled=True,
        duration="3h",
        codex_agent_name="codex",
        poll=False,
        timeout_seconds=600,
        stall_timeout_seconds=0,
        resume_completed=True,
        runner=fake_runner,
    )

    sends = [call for call in calls if call[:2] == ["commandmatedev", "send"]]
    assert len(sends) == 1
    assert "issue-2" in sends[0][2]
    issue_1 = next(result for result in dispatch if result.issue_number == 1)
    assert issue_1.status == "verified-complete"
    assert len(waits) == 1
    assert [result.status for result in schedule] == ["completed", "blocked"]


def test_scheduler_serializes_file_overlap_and_reports_predecessor(
    tmp_path: Path,
) -> None:
    module = load_script()
    analyses = [
        module.analyze_issue(
            module.Issue(number, f"Issue {number}", "Update `src/shared.rs`"),
            "CommandAgent",
            skip_enhance=True,
        )
        for number in (1, 2)
    ]
    batches, _ = module.classify_batches(analyses, "", max_parallel=2)
    worktrees = [
        module.WorktreeResult(
            analysis.issue.number,
            analysis.branch_name,
            tmp_path / f"issue-{analysis.issue.number}",
            "planned",
            "planned",
        )
        for analysis in analyses
    ]

    dispatch, waits, schedule = module.schedule_commandmate_batches(
        analyses,
        worktrees,
        batches,
        max_parallel=2,
        dry_run=True,
        dispatch_enabled=True,
        duration="3h",
        codex_agent_name="codex",
        poll=False,
        timeout_seconds=600,
        stall_timeout_seconds=0,
    )

    assert batches == [[1], [2]]
    assert waits == []
    assert [result.status for result in schedule] == ["planned", "planned"]
    issue_2 = next(result for result in dispatch if result.issue_number == 2)
    assert "Issue #1" in issue_2.commands[0]
    assert "file-conflict predecessor" in issue_2.commands[0]


def test_verify_worker_reports_requires_passing_status_and_checks(
    tmp_path: Path,
) -> None:
    module = load_script()
    issue = module.Issue(
        number=11,
        title="Verify worker output",
        body="## Acceptance Criteria\n- verification passes\n",
    )
    analysis = module.analyze_issue(issue, "CommandAgent", skip_enhance=True)
    report = tmp_path / "dev-reports" / "issue-11" / "verification.md"
    report.parent.mkdir(parents=True)
    report.write_text(
        "# Verification\n\n- Status: `passed`\n\n"
        "## Checks\n\n- `cargo test`: `passed`\n",
        encoding="utf-8",
    )
    worktree = module.WorktreeResult(
        11, analysis.branch_name, tmp_path, "created", "created"
    )

    def clean_git_runner(args, **kwargs):  # type: ignore[no-untyped-def]
        return module.subprocess.CompletedProcess(args, 0, "", "")

    results = module.verify_worker_reports(
        [analysis], [worktree], dry_run=False, runner=clean_git_runner
    )

    assert results[0].status == "passed"
    assert "1 worker verification checks passed" in results[0].message


def test_verify_worker_reports_blocks_missing_check_evidence(tmp_path: Path) -> None:
    module = load_script()
    issue = module.Issue(11, "Verify worker output", "## Acceptance Criteria\n- done\n")
    analysis = module.analyze_issue(issue, "CommandAgent", skip_enhance=True)
    report = tmp_path / "dev-reports" / "issue-11" / "verification.md"
    report.parent.mkdir(parents=True)
    report.write_text("# Verification\n\n- Status: `passed`\n", encoding="utf-8")
    worktree = module.WorktreeResult(
        11, analysis.branch_name, tmp_path, "created", "created"
    )

    def clean_git_runner(args, **kwargs):  # type: ignore[no-untyped-def]
        return module.subprocess.CompletedProcess(args, 0, "", "")

    results = module.verify_worker_reports(
        [analysis], [worktree], dry_run=False, runner=clean_git_runner
    )

    assert results[0].status == "blocked"
    assert results[0].message == "verification report contains no check results"


def test_verify_worker_reports_blocks_dirty_worktree(tmp_path: Path) -> None:
    module = load_script()
    issue = module.Issue(11, "Verify worker output", "## Acceptance Criteria\n- done\n")
    analysis = module.analyze_issue(issue, "CommandAgent", skip_enhance=True)
    report = tmp_path / "dev-reports" / "issue-11" / "verification.md"
    report.parent.mkdir(parents=True)
    report.write_text(
        "# Verification\n\n- Status: `passed`\n\n"
        "## Checks\n\n- `cargo test`: `passed`\n",
        encoding="utf-8",
    )
    worktree = module.WorktreeResult(
        11, analysis.branch_name, tmp_path, "created", "created"
    )

    def dirty_git_runner(args, **kwargs):  # type: ignore[no-untyped-def]
        return module.subprocess.CompletedProcess(args, 0, " M src/lib.rs\n", "")

    results = module.verify_worker_reports(
        [analysis], [worktree], dry_run=False, runner=dirty_git_runner
    )

    assert results[0].status == "blocked"
    assert "uncommitted changes" in results[0].message


def test_render_uat_report_includes_manual_evidence() -> None:
    module = load_script()
    issue = module.Issue(
        number=4,
        title="GUI check",
        body="## Acceptance Criteria\n- Button is visible on device\n",
    )
    analysis = module.analyze_issue(issue, "CommandAgent", skip_enhance=False)

    report = module.render_uat_report([analysis])

    assert "Manual CLI / TTY / GUI / Real-device Checks" in report
    assert "screenshot" in report


def test_evaluate_uat_gate_requires_complete_passing_evidence() -> None:
    module = load_script()
    issue = module.Issue(
        number=4,
        title="GUI check",
        body="## Acceptance Criteria\n- Button is visible\n- Button opens settings\n",
    )
    analysis = module.analyze_issue(issue, "CommandAgent", skip_enhance=True)
    partial = [module.UatResult(4, 1, "passed", "Visible", "screenshot-1.png")]

    incomplete = module.evaluate_uat_gate(
        [analysis], partial, require_complete=True, dry_run=False
    )
    passed = module.evaluate_uat_gate(
        [analysis],
        [
            *partial,
            module.UatResult(4, 2, "passed", "Settings opened", "tty-session.txt"),
        ],
        require_complete=True,
        dry_run=False,
    )

    assert incomplete.status == "blocked"
    assert "scenario 2" in incomplete.message
    assert passed.status == "passed"
    assert passed.message == "all 2 UAT scenarios passed with evidence"


def test_evaluate_uat_gate_blocks_failed_or_evidence_free_results() -> None:
    module = load_script()
    issue = module.Issue(
        4, "GUI check", "## Acceptance Criteria\n- Button is visible\n"
    )
    analysis = module.analyze_issue(issue, "CommandAgent", skip_enhance=True)

    failed = module.evaluate_uat_gate(
        [analysis],
        [module.UatResult(4, 1, "failed", "Missing", "screenshot.png")],
        require_complete=True,
        dry_run=False,
    )
    no_evidence = module.evaluate_uat_gate(
        [analysis],
        [module.UatResult(4, 1, "passed", "Visible", "")],
        require_complete=True,
        dry_run=False,
    )

    assert failed.status == "blocked"
    assert no_evidence.status == "blocked"
    assert "evidence is empty" in no_evidence.message


def test_load_uat_results_reads_evidence_contract(tmp_path: Path) -> None:
    module = load_script()
    fixture = tmp_path / "uat-results.json"
    fixture.write_text(
        json.dumps(
            {
                "results": [
                    {
                        "issue_number": 4,
                        "scenario_index": 1,
                        "status": "PASSED",
                        "actual": "Button visible",
                        "evidence": "screenshot.png",
                    }
                ]
            }
        ),
        encoding="utf-8",
    )

    results = module.load_uat_results(fixture)

    assert results == [
        module.UatResult(4, 1, "passed", "Button visible", "screenshot.png")
    ]


def test_render_uat_fix_prompts_maps_failure_to_issue() -> None:
    module = load_script()
    issue = module.Issue(
        number=5,
        title="Fix GUI regression",
        body="## Acceptance Criteria\n- GUI works\n",
    )
    analysis = module.analyze_issue(issue, "CommandAgent", skip_enhance=False)
    failure = module.UatFailure(
        issue_number=5,
        scenario="Open settings",
        expected="Settings opens",
        actual="Blank screen",
        evidence="screenshot.png",
    )

    prompt = module.render_uat_fix_prompts([failure], [analysis])

    assert "UAT failure fix for Issue #5" in prompt
    assert "Blank screen" in prompt
    assert "feature/issue-5-fix-gui-regression" in prompt


def test_create_uat_fix_worktrees_dry_run() -> None:
    module = load_script()
    failure = module.UatFailure(
        5, "Open settings", "Settings opens", "Blank", "shot.png"
    )

    results = module.create_uat_fix_worktrees([failure], dry_run=True)

    assert results[0].status == "planned"
    assert results[0].branch_name.startswith("fix/issue-5-uat-open-settings")


def test_photon_event_payload_redacts_paths_without_failing() -> None:
    module = load_script()

    result = module.emit_photon_event(
        "",
        event_kind="worker.blocked",
        run_id="run-1",
        payload={"path": "/Users/example/project/secret.txt"},
    )

    assert result.status == "skipped"
    assert module.sanitize_event_payload(
        {"path": "/Users/example/project/secret.txt"}
    ) == {"path": "[REDACTED_PATH]"}


def test_render_pr_body_contains_required_sections() -> None:
    module = load_script()
    issue = module.Issue(
        number=6,
        title="Add PR support",
        body=(
            "Update `scripts/codex_orchestrate.py`.\n\n"
            "## Acceptance Criteria\n- PR body lists tests\n"
        ),
    )
    analysis = module.analyze_issue(issue, "CommandAgent", skip_enhance=False)

    body = module.render_pr_body(analysis, "run-1")

    assert "Closes #6" in body
    assert "## Tests Run" in body
    assert "run-1" in body


def test_create_pull_requests_pushes_branch_before_develop_pr() -> None:
    module = load_script()
    issue = module.Issue(
        number=11,
        title="[P2] Add adapter",
        body="Update `src/models/photon_adapter.py`.",
    )
    analysis = module.analyze_issue(issue, "CommandAgent", skip_enhance=True)
    calls: list[list[str]] = []

    def fake_runner(args, **kwargs):  # type: ignore[no-untyped-def]
        calls.append(args)
        if args[:3] == ["git", "rev-list", "--count"]:
            return module.subprocess.CompletedProcess(args, 0, "1\n", "")
        if args[:3] == ["git", "push", "-u"]:
            return module.subprocess.CompletedProcess(args, 0, "", "")
        if args[:3] == ["gh", "pr", "list"]:
            return module.subprocess.CompletedProcess(args, 0, "[]\n", "")
        if args[:3] == ["gh", "pr", "create"]:
            return module.subprocess.CompletedProcess(
                args,
                0,
                "https://github.com/Kewton/CommandAgent/pull/42\n",
                "",
            )
        return module.subprocess.CompletedProcess(args, 1, "", "unexpected")

    results = module.create_pull_requests(
        [analysis],
        run_id="run-1",
        dry_run=False,
        runner=fake_runner,
    )

    assert results[0].status == "created"
    assert results[0].pr_number == 42
    assert ["git", "push", "-u", "origin", analysis.branch_name] in calls
    create_call_index = next(
        index for index, call in enumerate(calls) if call[:3] == ["gh", "pr", "create"]
    )
    assert (
        calls.index(["git", "push", "-u", "origin", analysis.branch_name])
        < create_call_index
    )
    assert "--draft" in calls[create_call_index]


def test_pr_numbers_for_merge_uses_created_and_existing_prs() -> None:
    module = load_script()
    results = [
        module.PullRequestResult(
            11, "feature/issue-11", "created", None, "https://x/pull/42", ""
        ),
        module.PullRequestResult(
            12, "feature/issue-12", "existing", 43, "https://x/pull/43", ""
        ),
        module.PullRequestResult(
            13, "feature/issue-13", "blocked", 44, "https://x/pull/44", ""
        ),
    ]

    assert module.pr_numbers_for_merge(results) == [42, 43]


def test_order_pr_numbers_for_merge_enforces_issue_dependency_order() -> None:
    module = load_script()
    results = [
        module.PullRequestResult(2, "feature/issue-2", "existing", 42, None, ""),
        module.PullRequestResult(3, "feature/issue-3", "existing", 43, None, ""),
        module.PullRequestResult(4, "feature/issue-4", "existing", 44, None, ""),
    ]

    ordered = module.order_pr_numbers_for_merge(results, [2, 3, 4], [44, 42, 43])

    assert ordered == [42, 43, 44]


def test_order_pr_numbers_for_merge_rejects_incomplete_pr_set() -> None:
    module = load_script()
    results = [
        module.PullRequestResult(2, "feature/issue-2", "existing", 42, None, ""),
        module.PullRequestResult(3, "feature/issue-3", "existing", 43, None, ""),
    ]

    with pytest.raises(ValueError, match="must match every requested issue"):
        module.order_pr_numbers_for_merge(results, [2, 3], [42])


def test_merge_pull_requests_waits_for_ci_before_merge() -> None:
    module = load_script()
    calls: list[list[str]] = []

    def fake_runner(args, **kwargs):  # type: ignore[no-untyped-def]
        calls.append(args)
        if args[:3] == ["gh", "pr", "view"]:
            return module.subprocess.CompletedProcess(
                args,
                0,
                json.dumps(
                    {"isDraft": False, "mergeStateStatus": "CLEAN", "number": 42}
                ),
                "",
            )
        if args[:3] == ["gh", "pr", "checks"]:
            return module.subprocess.CompletedProcess(args, 0, "checks passed\n", "")
        if args[:3] == ["gh", "pr", "merge"]:
            return module.subprocess.CompletedProcess(args, 0, "", "")
        if args[:3] == ["git", "pull", "--ff-only"]:
            return module.subprocess.CompletedProcess(args, 0, "", "")
        return module.subprocess.CompletedProcess(args, 1, "", "unexpected")

    results = module.merge_pull_requests(
        [42, 43],
        dry_run=False,
        merge_method="squash",
        integration_checks=[],
        uat_gate=module.UatGateResult("passed", "all scenarios passed"),
        runner=fake_runner,
    )

    assert [result.status for result in results] == ["merged", "merged"]
    assert ["gh", "pr", "checks", "42", "--watch", "--interval", "10"] in calls
    check_indices = [
        index for index, call in enumerate(calls) if call[:3] == ["gh", "pr", "checks"]
    ]
    merge_indices = [
        index for index, call in enumerate(calls) if call[:3] == ["gh", "pr", "merge"]
    ]
    assert len(check_indices) == 6
    assert max(check_indices[:4]) < merge_indices[0]
    assert check_indices[4] < merge_indices[0]
    assert check_indices[5] < merge_indices[1]


def test_merge_pull_requests_resolves_missing_strategy_to_merge() -> None:
    module = load_script()
    calls: list[list[str]] = []

    def fake_runner(args, **kwargs):  # type: ignore[no-untyped-def]
        calls.append(args)
        if args[:3] == ["gh", "pr", "view"]:
            return module.subprocess.CompletedProcess(
                args,
                0,
                json.dumps(
                    {"isDraft": False, "mergeStateStatus": "CLEAN", "number": 42}
                ),
                "",
            )
        if args[:3] == ["gh", "pr", "checks"]:
            return module.subprocess.CompletedProcess(args, 0, "checks passed\n", "")
        if args[:3] == ["gh", "pr", "merge"]:
            return module.subprocess.CompletedProcess(args, 0, "", "")
        if args[:3] == ["git", "pull", "--ff-only"]:
            return module.subprocess.CompletedProcess(args, 0, "", "")
        return module.subprocess.CompletedProcess(args, 1, "", "unexpected")

    results = module.merge_pull_requests(
        [42],
        dry_run=False,
        merge_method=None,
        integration_checks=[],
        uat_gate=module.UatGateResult("passed", "all scenarios passed"),
        runner=fake_runner,
    )

    assert results[0].status == "merged"
    assert ["gh", "pr", "merge", "42", "--merge"] in calls


def test_merge_pull_requests_marks_draft_ready_only_after_ci_and_uat() -> None:
    module = load_script()
    calls: list[list[str]] = []
    ready = False

    def fake_runner(args, **kwargs):  # type: ignore[no-untyped-def]
        nonlocal ready
        calls.append(args)
        if args[:3] == ["gh", "pr", "view"]:
            return module.subprocess.CompletedProcess(
                args,
                0,
                json.dumps(
                    {"isDraft": not ready, "mergeStateStatus": "CLEAN", "number": 42}
                ),
                "",
            )
        if args[:3] == ["gh", "pr", "checks"]:
            return module.subprocess.CompletedProcess(args, 0, "checks passed\n", "")
        if args[:3] == ["gh", "pr", "ready"]:
            ready = True
            return module.subprocess.CompletedProcess(args, 0, "", "")
        if args[:3] == ["gh", "pr", "merge"]:
            return module.subprocess.CompletedProcess(args, 0, "", "")
        if args[:3] == ["git", "pull", "--ff-only"]:
            return module.subprocess.CompletedProcess(args, 0, "", "")
        return module.subprocess.CompletedProcess(args, 1, "", "unexpected")

    results = module.merge_pull_requests(
        [42],
        dry_run=False,
        merge_method="squash",
        integration_checks=[],
        uat_gate=module.UatGateResult("passed", "all scenarios passed"),
        runner=fake_runner,
    )

    ready_index = calls.index(["gh", "pr", "ready", "42"])
    check_indices = [
        index for index, call in enumerate(calls) if call[:3] == ["gh", "pr", "checks"]
    ]
    merge_index = calls.index(["gh", "pr", "merge", "42", "--squash"])
    assert results[0].status == "merged"
    assert (
        check_indices[0]
        < ready_index
        < check_indices[1]
        < check_indices[2]
        < merge_index
    )


def test_merge_pull_requests_blocks_after_ci_when_uat_did_not_pass() -> None:
    module = load_script()
    calls: list[list[str]] = []

    def fake_runner(args, **kwargs):  # type: ignore[no-untyped-def]
        calls.append(args)
        if args[:3] == ["gh", "pr", "view"]:
            return module.subprocess.CompletedProcess(
                args,
                0,
                json.dumps(
                    {"isDraft": True, "mergeStateStatus": "CLEAN", "number": 42}
                ),
                "",
            )
        if args[:3] == ["gh", "pr", "checks"]:
            return module.subprocess.CompletedProcess(args, 0, "checks passed\n", "")
        return module.subprocess.CompletedProcess(args, 1, "", "unexpected")

    results = module.merge_pull_requests(
        [42],
        dry_run=False,
        merge_method="squash",
        integration_checks=[],
        uat_gate=module.UatGateResult("blocked", "scenario 1 failed"),
        runner=fake_runner,
    )

    assert results[0].status == "blocked"
    assert "UAT gate is blocked" in results[0].message
    assert not any(call[:3] == ["gh", "pr", "ready"] for call in calls)
    assert not any(call[:3] == ["gh", "pr", "merge"] for call in calls)
