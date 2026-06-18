---
name: commandagent-orchestrate
description: Plan CommandAgent issue orchestration through a dry-run manifest before any worktree, CommandMate, PR, merge, or UAT action. Use when the user asks to orchestrate multiple issues or plan parallel issue work.
---

# CommandAgent Orchestrate

Use this skill for multi-issue planning. Start with dry-run artifacts.

## First Action

Run the dry-run planner:

```bash
python3 scripts/codex_orchestrate.py <issue...> --dry-run
```

If GitHub access is unavailable, use an issue fixture with `--issue-json`.

Review generated artifacts under:

```text
workspace/management/runs/<run_id>/
```

Expected files:

- `manifest.md`
- `issue-analysis.md`
- `dependency-plan.md`

## Operating Rules

- Use `origin/develop` as the default issue work base.
- Do not create worktrees unless `--create-worktrees` is explicitly requested.
- Do not dispatch CommandMate unless `--dispatch-commandmate` is explicitly requested.
- Do not create PRs unless `--create-prs` is explicitly requested.
- Do not merge PRs unless `--merge-prs` is explicitly requested and CI status is acceptable.
- Do not start or stop CommandMate unless the user explicitly asks.
- Treat sandboxed localhost failures as inconclusive until verified outside the sandbox.
- Keep all run artifacts in ignored workspace state unless the user asks to commit a summary.

## Mutating Phases

Mutating phases are disabled by default. Enable only one phase at a time and record the commands and outputs in the run directory.

Allowed explicit flags:

- `--create-worktrees`
- `--dispatch-commandmate`
- `--create-prs`
- `--merge-prs`
- `--write-uat`
- `--create-uat-fix-worktrees`
