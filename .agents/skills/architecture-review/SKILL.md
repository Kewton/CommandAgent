---
name: architecture-review
description: Review a CommandAgent issue design for architectural fit, compatibility, security, risk, and testability. Use after a design policy exists or before high-risk implementation begins.
---

# CommandAgent Architecture Review

Review evidence and design decisions without implementing fixes.

## Review Passes

1. Read the issue, `dev-reports/issue-<number>/design-policy.md` when present, and the relevant current code/tests.
2. Check responsibility boundaries, coupling, cohesion, duplication, and whether the design is simpler than its alternatives.
3. Check CommandAgent-specific constraints:
   - runner and minimal-loop growth tripwires
   - profile capability registration and declarative knowledge ownership
   - event schema and consumer compatibility
   - bounded process execution, path guards, and secret handling
   - provider/tool/TUI behavior compatibility
4. Evaluate failure modes, recovery, concurrency/state transitions, and observability.
5. Verify that acceptance criteria map to focused and broader tests.
6. Classify every finding by severity (`blocking`, `major`, `minor`, `note`) and cite files or evidence.

Apply SOLID, KISS, YAGNI, and DRY as diagnostic lenses, not mechanical requirements. Avoid speculative findings unsupported by the repository.

## Decision

Choose:

- `APPROVED`: no material design blocker.
- `CONDITIONAL`: implementable after named changes or evidence.
- `REJECTED`: unsafe or fundamentally incompatible; redesign required.

Write `dev-reports/issue-<number>/architecture-review.md` with summary, findings, risk matrix, required actions, and decision. Do not modify the design or code; use `$apply-review` for approved design updates.
