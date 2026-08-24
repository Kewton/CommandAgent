---
name: worktree-cleanup
description: Inspect and safely remove merged or obsolete CommandAgent issue worktrees, including squash- or rebase-merged branches proven equivalent to their merged pull request. Use when the user explicitly asks to clean one issue worktree or all eligible issue worktrees.
---

# CommandAgent Worktree Cleanup

Clean eligible issue worktrees without discarding work. The integration branch is `develop`; remote merge checks use `origin/develop`.

## Safety Rules

- Never remove the current worktree.
- Never remove the `develop` integration worktree.
- Never use `git worktree remove --force`.
- Never use `git branch -D`.
- Do not remove a worktree with uncommitted changes.
- Delete a branch only when its tip is reachable from `origin/develop`, or when the exact current tip and tree are proven to match a merged PR commit that is reachable from `origin/develop`.
- Never accept merged PR state alone, matching trees alone, or a stale remote-tracking branch as sufficient proof.
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

If fetch fails, continue with local inspection but do not remove any target or claim that remote merge state is current.

### 2. Resolve Targets

For one issue number, require a positive integer and match branches beginning `feature/issue-<number>-` or paths containing `CommandAgent-issue-<number>-` / `CommandAgent-feature-issue-<number>-`.

For all targets, select only issue worktrees, preferring branches beginning `feature/issue-`. Exclude the current and integration worktrees.

### 3. Check Every Candidate

Run:

```bash
git -C <worktree_path> status --porcelain
git -C <worktree_path> branch --show-current
git rev-parse <branch>
git merge-base --is-ancestor <branch_tip> origin/develop
```

Interpretation:

- Any status output means dirty: do not remove.
- An empty branch means detached: do not remove without explicit follow-up approval.
- Exit status 0 from `merge-base --is-ancestor` means the branch is merged.
- If direct ancestry fails, perform the exact merged-PR fallback below. Do not classify the branch as unmerged from ancestry alone because squash and rebase merges rewrite commit identity.

For the fallback, query merged PRs for the exact head branch and `develop` base:

```bash
gh pr list --state merged --head <branch> --base develop \
  --json number,state,baseRefName,headRefName,headRefOid,mergeCommit,mergedAt
```

Accept exactly one PR entry only when all of these checks pass:

1. `state` is `MERGED`, `baseRefName` is `develop`, and `headRefName` exactly equals the candidate branch.
2. `headRefOid` exactly equals `<branch_tip>`. This rejects branches advanced after merging.
3. `mergeCommit.oid` is present and `git merge-base --is-ancestor <merge_commit> origin/develop` exits 0.
4. `git rev-parse <branch_tip>^{tree}` and `git rev-parse <merge_commit>^{tree}` return the same tree ID.

Classify a candidate satisfying all four checks as `merged-equivalent`. Missing GitHub data, no exact PR, multiple exact PRs, a moved branch, an unreachable merge commit, or a tree mismatch is unverifiable: do not remove it.

### 4. Remove Only Safe Targets

For each clean, directly merged issue worktree:

```bash
git worktree remove <worktree_path>
git branch -d <branch>
```

For each clean `merged-equivalent` worktree, retain the previously recorded `<branch_tip>` and run:

```bash
git worktree remove <worktree_path>
git update-ref -d refs/heads/<branch> <branch_tip>
```

`git update-ref -d` is allowed only after every exact merged-PR fallback check passes. Its expected-old-OID argument makes deletion fail if the branch moves after verification. Never replace it with `git branch -D`.

If direct `git branch -d` says the branch is missing, record it as non-fatal. If it says the branch is not fully merged, do not retry with `-D`. If the guarded `git update-ref -d` fails, retain the branch and report that its tip changed or deletion could not be verified.

### 5. Verify

Run:

```bash
git worktree prune
git worktree list
```

Report removed paths, deleted branches, proof type (`direct` or `merged-equivalent`), fallback PR and merge commit when used, skipped targets with reasons, and the final worktree list.
