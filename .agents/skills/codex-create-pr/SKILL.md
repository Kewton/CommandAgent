---
name: codex-create-pr
description: Create a verified pull request from a CommandAgent issue worktree to develop. Use when the user explicitly asks to publish completed issue work or an authorized orchestration run reaches its pull-request phase.
---

# CommandAgent Create PR

Create a pull request only after the issue worker has committed verified changes and the user has authorized publication.

## Preflight

1. Confirm the current branch and worktree are the intended issue worktree.
2. Confirm the worktree is clean and the expected commits are present.
3. Review the issue reports and test results.
4. Check whether an open pull request already exists for the branch. Do not create a duplicate.

## Required PR Body

Include:

- linked issue
- concise summary
- changed files
- tests run and results
- known risks or `None`
- orchestration run ID when available

Target `develop` unless the user explicitly asks for a release or `main` pull request. Do not merge as part of this skill unless merging is separately authorized.
