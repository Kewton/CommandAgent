---
name: pr-merge-pipeline
description: Verify and merge an authorized ordered set of CommandAgent pull requests into develop. Use only when the user explicitly requests pull-request creation and/or merging as a coordinated pipeline.
---

# CommandAgent PR Merge Pipeline

Establish the authorized PR set, target, merge method, and stopping conditions before mutating GitHub.

## Safety Rules

- Target `develop` unless the user explicitly requests another branch.
- Never merge failing, pending, draft, conflicted, or unreviewed PRs contrary to repository policy.
- Never force-push, force-delete, bypass required checks, or merge to `main` implicitly.
- Do not auto-resolve semantic conflicts. Report the affected branch and required decision.
- Preserve an explicit merge order when provided; otherwise derive it from dependencies and overlapping files.

## Pipeline

1. Inventory authorized branches/PRs, issue links, worktrees, commits, and existing open PRs.
2. Create missing PRs only when included in the user's authorization, following `$codex-create-pr` requirements.
3. Wait for checks and review state; record failures per PR. Do not merge until all required gates pass.
4. Determine dependency-aware merge order and present it when not already approved.
5. For each PR, recheck base, mergeability, checks, and reviews immediately before merging.
6. Merge using the authorized method. After each merge, update the integration worktree with fast-forward-only operations and run proportional integration checks.
7. Stop the pipeline on conflict, CI regression, integration-test failure, or unexpected branch state. Do not continue with dependent PRs.
8. Run final integration verification on `develop` and capture its commit.

## Output

Write a new orchestration report rather than editing historical run evidence. List every PR's status, merge commit/order, skipped or blocked items, checks, integration results, final `develop` commit, and follow-up actions.
