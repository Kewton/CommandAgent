---
name: source-command-tdd-impl
description: Implement CommandAgent changes with a Red-Green-Refactor workflow. Use when the user asks for `/tdd-impl`, TDD implementation, or test-first development.
---

# Source Command: TDD Implementation

Use `.codex/prompts/tdd-impl-core.md` for the detailed workflow.

## Rules

- Use TDD for implementation tasks where a meaningful failing test can be written.
- Do not force TDD for review-only, planning-only, or docs-only tasks.
- Keep tests deterministic and offline.
- Use integration tests under `tests/` for public runtime behavior.
- Keep provider-backed live tests out of normal Cargo tests.

## Completion

Report Red-Green-Refactor evidence, tests run, changed files, and remaining risks.
