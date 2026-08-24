---
name: progress-report
description: Summarize current CommandAgent delivery progress from repository, issue, pull-request, CI, and report evidence. Use when the user asks for status, completed work, blockers, risks, or next actions.
---

# CommandAgent Progress Report

Produce a read-only, timestamped snapshot. Do not change issues, pull requests, branches, or files unless the user separately requests a saved report.

## Evidence

Collect only what is relevant:

- current branch, commit, worktree state, and local divergence
- recent commits and active issue branches/worktrees
- requested GitHub issues and pull requests
- CI/check status
- issue design, implementation, verification, and UAT reports
- milestones, dependencies, and known blockers

Distinguish confirmed state from inference. Note when GitHub or CommandMate could not be reached and do not treat a sandbox localhost failure as proof that CommandMate is stopped.

## Output

Lead with overall status, then list completed work, in progress, blocked/at risk, verification state, and prioritized next actions. Include commit/issue/PR identifiers and evidence timestamps where useful. Avoid percentages unless they are derived from an explicit scope and explain the denominator.
