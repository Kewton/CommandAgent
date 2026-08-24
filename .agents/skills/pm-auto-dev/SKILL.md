---
name: pm-auto-dev
description: Run a prepared CommandAgent issue through implementation, review, acceptance verification, and reporting. Use when the issue and design are ready and the user asks for an end-to-end local development flow.
---

# CommandAgent Auto Development

Execute the development stages sequentially in the current agent. Do not dispatch subagents or CommandMate unless the user explicitly requests it.

## Preconditions

- The issue has testable acceptance criteria.
- The current worktree is the intended issue worktree and is safe to edit.
- Design/work-plan decisions needed for implementation are resolved.

## Stages

1. **Context**: read the issue, design, work plan, current code/tests, and guardrails.
2. **TDD implementation**: follow `$tdd-impl` red-green-refactor cycles for each behavior.
3. **Focused review**: inspect the diff for logic errors, unsafe paths/processes, secret leakage, state-machine mistakes, event compatibility, and missing tests. Fix verified findings.
4. **Acceptance**: map every criterion to executable evidence using the `$acceptance-test` workflow.
5. **Refactor**: make only behavior-preserving cleanup justified by the change; rerun tests.
6. **Documentation**: update living docs/examples only where behavior or usage changed.
7. **Broad verification**: run formatting, Clippy, full tests, and any affected corpus/conformance/scenario checks.
8. **Reports**: write design, implementation, verification, and acceptance summaries under `dev-reports/issue-<number>/`.

Stop on a blocking design decision, unsafe scope expansion, or a baseline failure unrelated to the issue. Do not push, create a pull request, or merge unless separately authorized. Commit only when requested.

## Completion

Report stage status, changed files, acceptance mapping, commands/results, known risks, and readiness for `$codex-create-pr`.
