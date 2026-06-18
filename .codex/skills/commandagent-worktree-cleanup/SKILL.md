---
name: commandagent-worktree-cleanup
description: Safely remove merged or obsolete CommandAgent git worktrees. Use when the user asks to clean up issue worktrees, feature worktrees, or all merged worktrees for this repository.
---

# CommandAgent Worktree Cleanup

Use this skill to remove only clean, merged worktrees.

## Safety Rules

- Never remove the current worktree.
- Never use `git worktree remove --force`.
- Never use `git branch -D`.
- Do not remove a worktree with uncommitted changes.
- Do not delete a branch unless its tip is reachable from `origin/develop`, unless the user explicitly chooses another merge base.
- Stop and report exact path, branch, and reason for dirty, detached, missing, or unmerged worktrees.

## Expected Names

Issue worktrees normally look like:

```text
../CommandAgent-issue-<number>-<slug>
../CommandAgent-feature-issue-<number>-<slug>
```

Branches normally look like:

```text
feature/issue-<number>-<slug>
```

Discover actual targets from `git worktree list --porcelain`; do not assume the slug.

## Procedure

1. Inspect:
   - `git branch --show-current`
   - `git worktree list --porcelain`
   - `git fetch origin develop --prune`
2. Resolve target worktrees by issue number, branch prefix, or path convention.
3. For each target, run:
   - `git -C <path> status --porcelain`
   - `git -C <path> branch --show-current`
   - `git merge-base --is-ancestor <branch> origin/develop`
4. Remove only safe targets:
   - `git worktree remove <path>`
   - `git branch -d <branch>`
5. Run `git worktree prune` and report removed/skipped items.
