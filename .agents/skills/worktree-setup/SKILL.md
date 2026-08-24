---
name: worktree-setup
description: Create and verify a dedicated CommandAgent worktree for one GitHub issue. Use when the user explicitly asks to prepare an isolated issue branch and development directory.
---

# CommandAgent Worktree Setup

Create an isolated worktree without disturbing existing branches or directories.

## Workflow

1. Validate that the issue number is a positive integer and read its current title/body.
2. Inspect `git status`, the current branch, existing worktrees, and local/remote branches.
3. Derive:
   - branch: `feature/issue-<number>-<slug>`
   - directory: `../CommandAgent-issue-<number>-<slug>`
4. Refuse to overwrite an existing directory, branch, or worktree. If a matching worktree exists, report it and reuse it only when the user requests reuse.
5. Refresh `origin/develop` when available, then create from `origin/develop`:

```bash
git worktree add -b <branch> <directory> origin/develop
```

6. In the new worktree, confirm branch/status and run a proportional baseline such as `cargo build` and `cargo test --all-targets`.
7. If baseline verification fails, keep the worktree for inspection and report the failure; do not delete or reset it automatically.

Do not create an empty start commit. Do not modify the integration worktree. Worktree paths outside the current workspace may require explicit filesystem approval.

## Output

Report issue, directory, branch, base commit, baseline commands/results, and the CommandMate worktree ID when relevant.
