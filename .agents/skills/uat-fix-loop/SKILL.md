---
name: uat-fix-loop
description: Repair failed CommandAgent UAT scenarios and reverify them through a bounded iteration loop. Use only when the user identifies a UAT run and authorizes the required fix, PR, merge, and rerun scope.
---

# CommandAgent UAT Fix Loop

Use a bounded loop. Default to at most three iterations unless the user sets another limit.

## Safety Rules

- Treat the selected UAT report as historical evidence; never rewrite it.
- Create a new run/report for each repair iteration.
- Confirm up front whether authorization includes worktrees, commits, pushes, PRs, and merges.
- Do not merge failing CI or continue after an integration regression.
- Do not start/stop CommandMate or infer server shutdown from sandbox localhost failure.

## Iteration

1. Read the identified UAT report and build a failure matrix: scenario, issue, expected, actual, evidence, severity, and reproduction status.
2. Group failures by root cause and affected issue/worktree. Do not create one fix per symptom when evidence shows a shared cause.
3. Reproduce and analyze each group. Use `$cause-analysis` when the causal chain is unclear.
4. Implement the smallest regression-tested repair using `$bug-fix` or `$tdd-impl`.
5. Run focused and broad quality checks in each worktree.
6. If authorized, commit/push, create or update PRs, wait for CI, and merge in dependency order.
7. Rerun the failed scenarios first, then any regression-sensitive scenarios using `$codex-uat`.
8. Record a new iteration report and decide `PASS`, `RETRY`, or `BLOCKED`.

Stop when all scenarios pass, the iteration limit is reached, the same unexplained failure repeats, or safe progress requires new authority/decisions.

## Output

Report failure groups, root causes, fixes, PR/merge status, commands/results, new UAT evidence, iteration count, remaining failures, and the exact blocker or next action.
