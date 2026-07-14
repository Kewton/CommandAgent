---
name: orchestrate
description: Plan and run CommandAgent GitHub issue orchestration through CommandMate and dedicated git worktrees. Use when the user asks to orchestrate one or more issues in this repository.
---

# CommandAgent Orchestrate

Plan issue work first, then advance only through the phases the user has authorized.

## Operating Rules

- Start from the CommandAgent `develop` integration worktree.
- Use `origin/develop` as the base for planned worktrees.
- Keep issue enhancement lightweight and ask only blocking questions.
- Store run artifacts under `workspace/management/runs/<run_id>/`.
- Treat existing files under `workspace/management/runs/` as frozen historical evidence.
- Do not delete, reset, or overwrite existing worktrees without explicit approval.
- Do not create or merge pull requests unless the user has authorized those external actions.
- Do not merge pull requests with failing CI unless the user explicitly approves.
- Do not start or stop CommandMate, or kill port processes, unless the user explicitly asks.
- Treat `commandmatedev` localhost read failures as `unreachable` until verified outside the sandbox. A sandbox failure does not prove that the user's CommandMate server is stopped.
- Use the Codex CommandMate agent. The repository script defaults to `--agent codex` and waits on `--instance codex`.
- Include manual UAT steps when CLI/TTY behavior, release flow, GUI behavior, or a real device must be confirmed.

## First Action

Run the non-mutating planner before any other phase:

```bash
python3 scripts/codex_orchestrate.py <issue...> --dry-run
```

Review:

- `workspace/management/runs/<run_id>/manifest.md`
- `workspace/management/runs/<run_id>/issue-analysis.md`
- `workspace/management/runs/<run_id>/dependency-plan.md`

Confirm that the issue scope, dependency batches, suspected files, and blocking questions are coherent. The planner defaults to the `codex` CommandMate agent; override `--codex-agent-name` only when the user requests another registered instance.

## Advancing The Run

Use only the flags required for the authorized phase. Worktree creation, CommandMate dispatch, pull-request creation, merging, and UAT-fix worktrees are mutating operations. Report the generated run directory and stop before any unapproved phase.

The dispatched worker prompt invokes `$codex-issue-worker`. Keep that skill available in each issue worktree by committing this repository harness before dispatch.

## Verification

After changing the harness, run:

```bash
ruff check scripts/codex_orchestrate.py tests/test_codex_orchestrate.py
pytest -q tests/test_codex_orchestrate.py
```
