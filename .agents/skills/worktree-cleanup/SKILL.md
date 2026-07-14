---
name: worktree-cleanup
description: Inspect and safely remove merged or obsolete CommandAgent issue worktrees. Use when the user explicitly asks to clean one issue worktree or all eligible issue worktrees.
---

# CommandAgent Worktree Cleanup

Clean eligible issue worktrees without discarding work. The integration branch is `develop`; remote merge checks use `origin/develop`.

## Safety Rules

- Never remove the current worktree.
- Never remove the `develop` integration worktree.
- Never use `git worktree remove --force`.
- Never use `git branch -D`.
- Do not remove a worktree with uncommitted changes.
- Do not delete a branch unless its tip is reachable from `origin/develop`.
- If a worktree is dirty, detached, or unmerged, stop for that target and report its exact path, branch, and reason.

## Expected Shapes

Harness worktrees normally look like:

```text
../CommandAgent-issue-<number>-<slug>
../CommandAgent-feature-issue-<number>-<slug>
```

Branches normally look like:

```text
feature/issue-<number>-<slug>
```

Discover candidates from `git worktree list --porcelain`; never assume the slug.

## Procedure

### 1. Inspect

Run:

```bash
git branch --show-current
git worktree list --porcelain
git fetch origin develop --prune
```

If fetch fails, continue with local inspection but do not claim that remote merge state is current.

### 2. Resolve Targets

For one issue number, require a positive integer and match branches beginning `feature/issue-<number>-` or paths containing `CommandAgent-issue-<number>-` / `CommandAgent-feature-issue-<number>-`.

For all targets, select only issue worktrees, preferring branches beginning `feature/issue-`. Exclude the current and integration worktrees.

### 3. Check Every Candidate

Run:

```bash
git -C <worktree_path> status --porcelain
git -C <worktree_path> branch --show-current
git merge-base --is-ancestor <branch> origin/develop
```

Interpretation:

- Any status output means dirty: do not remove.
- An empty branch means detached: do not remove without explicit follow-up approval.
- Exit status 0 from `merge-base --is-ancestor` means the branch is merged.
- Any other result means unmerged or unverifiable: do not delete the branch.

### 4. Remove Only Safe Targets

For each clean, merged issue worktree:

```bash
git worktree remove <worktree_path>
git branch -d <branch>
```

If `git branch -d` says the branch is missing, record it as non-fatal. If it says the branch is not fully merged, do not retry with `-D`.

### 5. Verify

Run:

```bash
git worktree prune
git worktree list
```

Report removed paths, deleted branches, skipped targets with reasons, and the final worktree list.
