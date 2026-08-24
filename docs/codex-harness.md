# Codex Harness

CommandAgent keeps its repository-local Codex instructions in `AGENTS.md` and
its reusable skills in `.agents/skills/`. Invoke a skill with `$skill-name`.
The legacy custom-command form `/command-name` is not installed in this
repository.

## Orchestration

Plan before dispatching any worker:

```bash
python3 scripts/codex_orchestrate.py <issue...> --dry-run
```

The planner writes a new directory under `workspace/management/runs/`. Existing
run directories are historical evidence and must not be overwritten. The
CommandMate integration explicitly selects the `codex` agent and waits on the
`codex` instance by default.

Mutating phases such as worktree creation, worker dispatch, pull-request
creation, merging, and UAT-fix worktrees require corresponding command flags
and user authorization. The `$orchestrate` skill documents those boundaries.

## Migrated Command Map

| Source command | Codex skill |
| --- | --- |
| `/acceptance-test` | `$acceptance-test` |
| `/apply-review` | `$apply-review` |
| `/architecture-review` | `$architecture-review` |
| `/bug-fix` | `$bug-fix` |
| `/cause-analysis` | `$cause-analysis` |
| `/create-pr` | `$codex-create-pr` |
| `/current-situation` | `$current-situation` |
| `/design-policy` | `$design-policy` |
| `/issue-create` | `$issue-create` |
| `/issue-enhance` | `$issue-enhance` |
| `/issue-split` | `$issue-split` |
| `/issues-exec-plan` | `$issues-exec-plan` |
| `/multi-stage-design-review` | `$multi-stage-design-review` |
| `/multi-stage-issue-review` | `$multi-stage-issue-review` |
| `/orchestrate` | `$orchestrate` |
| `/pm-auto-design2dev` | `$pm-auto-design2dev` |
| `/pm-auto-dev` | `$pm-auto-dev` |
| `/pm-auto-issue2dev` | `$pm-auto-issue2dev` |
| `/pr-merge-pipeline` | `$pr-merge-pipeline` |
| `/progress-report` | `$progress-report` |
| `/refactoring` | `$refactoring` |
| `/tdd-impl` | `$tdd-impl` |
| `/uat` | `$codex-uat` |
| `/uat-fix-loop` | `$uat-fix-loop` |
| `/work-plan` | `$work-plan` |
| `/worktree-cleanup` | `$worktree-cleanup` |
| `/worktree-setup` | `$worktree-setup` |

`$codex-issue-worker` is an additional internal worker skill used by the
orchestrator. The source worker and manual-UAT prompt templates were moved into
skill-local `references/` directories instead of installing deprecated custom
prompt files.

Release and post-release skills were intentionally not migrated in this task.
Release publication has separate credentials, tagging, artifact, and rollback
requirements and should be migrated and reviewed independently.

## Safety Model

Skills that write code or reports, alter worktrees, update GitHub, dispatch
CommandMate, or merge pull requests have implicit invocation disabled. Their
`agents/openai.yaml` metadata requires an explicit `$skill-name` invocation.
Read-only inspection inside a skill does not authorize later external actions;
the requested scope still determines whether create, push, PR, or merge steps
may run.

The composite skills execute sequentially in the current agent by default.
They do not dispatch subagents or CommandMate unless the user explicitly asks
for that coordination.

## Validation

Run the repository guardrails with:

```bash
python3 scripts/validate_codex_skills.py
ruff check scripts/codex_orchestrate.py scripts/validate_codex_skills.py tests/test_codex_orchestrate.py
python3 -m pytest tests/test_codex_orchestrate.py -q
```

CI runs the same checks before the Rust test suite. The skill validator checks
frontmatter, directory/name consistency, linked references, UI metadata,
explicit invocation policy types, and unresolved template markers.
