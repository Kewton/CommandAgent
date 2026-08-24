---
name: codex-issue-worker
description: Implement one assigned CommandAgent issue in a dedicated git worktree. Use after the repository orchestrator dispatches an issue worker task or when the user explicitly assigns one issue to the current worktree.
---

# CommandAgent Issue Worker

Implement one issue inside its dedicated worktree. Read [worker-prompt.md](references/worker-prompt.md) when the dispatch message is incomplete or a reusable worker checklist is needed.

## Required Flow

1. Read the issue summary, acceptance criteria, orchestration notes, suspected files, and references.
2. Inspect the smallest relevant code surface before editing.
3. Write `dev-reports/issue-<number>/design.md` before implementation edits.
4. Implement the smallest coherent change that satisfies the issue. Avoid broad architecture churn unless required.
5. Add or update focused tests when behavior changes.
6. Run focused verification first.
7. Run broader verification when shared behavior, CI-sensitive code, release code, or harness code is touched.
8. Write `dev-reports/issue-<number>/implementation-summary.md`.
9. Write `dev-reports/issue-<number>/verification.md` using the verification contract below.
10. Commit with a clear issue-scoped message when the task explicitly includes a commit.

## Verification Guidance

- Rust: run the narrowest relevant `cargo test` command, then broaden to `cargo test` when shared behavior is affected.
- Shared Rust or CLI behavior: run `cargo clippy --all-targets -- -D warnings`.
- Formatting-sensitive Rust changes: run `cargo fmt --all -- --check`.
- CLI/help surface: update and run the relevant snapshot or check.
- Harness Python: run the focused pytest module and Ruff checks.
- Event output: preserve schema and compatibility unless the issue explicitly authorizes a schema change.

## Verification Contract

Use this shape only when every required check succeeded:

```markdown
- Status: `passed`

## Checks

- `<command>`: `passed`
```

If a required check fails or cannot run, use `blocked` for the overall status and record the failed or blocked check honestly. Never mark the report passed with missing, failed, or ambiguous checks.

Keep review lightweight and ask only blocking questions. Do not create a pull request, merge, or modify external issue state unless explicitly authorized.
