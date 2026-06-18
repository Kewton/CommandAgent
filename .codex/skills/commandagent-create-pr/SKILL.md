---
name: commandagent-create-pr
description: Create a CommandAgent pull request from the current committed branch. Use when the user asks Codex to publish local work, open a PR, or prepare a branch for review in this repository.
---

# CommandAgent Create PR

Use this skill after the intended work is committed and verified.

## Required Flow

1. Run `git status --short --branch` and inspect the branch.
2. Confirm the intended diff and commits belong in the PR.
3. Check for an existing open PR for the branch before creating a new one.
4. Push the current branch.
5. Create the PR with a body that includes summary, changed files, tests run, docs checked, risks, and follow-up.
6. Report the PR URL, target branch, commit range, and validation.

## Target Branch Rules

- Default feature/task PRs to `develop`.
- Target `main` only when the user explicitly asks, or when release workflow requires it.
- Do not directly push to protected or release branches when a PR is expected.
- Prefer the GitHub connector for PR creation. Use `gh` only when authenticated and needed.

## PR Body Checklist

- What changed.
- Why it aligns with CommandAgent's minimal design.
- Public API or workflow impact.
- Tests run.
- Docs checked or updated.
- Focused eval, if behavior changed.
- Follow-up work.
