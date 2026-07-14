---
name: design-policy
description: Create an issue-scoped CommandAgent design policy grounded in the current architecture and guardrails. Use before implementation when behavior, module boundaries, data contracts, or tradeoffs need an explicit design.
---

# CommandAgent Design Policy

Write a decision-oriented design document, not a generic architecture essay.

## Workflow

1. Read the issue, acceptance criteria, related discussions, and existing reports.
2. Inspect the current implementation and tests. Use the actual CommandAgent layout: planner, minimal loop, tools, providers, profiles, TUI, runs, and event output as relevant.
3. Read `docs/dev-guardrails.md` for planner or minimal-loop work.
4. Define:
   - problem, goals, and non-goals
   - current behavior and constraints
   - proposed responsibility and module boundaries
   - data flow, state transitions, and failure handling
   - public CLI/config/event compatibility
   - security, path, secret, and process-execution boundaries
   - alternatives and tradeoffs
   - migration/rollback strategy when needed
   - test strategy and observable acceptance mapping
5. Identify uncertain assumptions and decisions that require user input.
6. Keep the design proportional to the issue; avoid speculative abstraction.

For runner growth, prefer new focused modules over expanding chokepoints. For profile/evidence knowledge, preserve declarative TOML ownership and required scenario matrices. For event output, preserve schemas and consumers unless explicitly authorized otherwise.

## Output

Write `dev-reports/issue-<number>/design-policy.md`. Include a decision log, affected files, compatibility assessment, risks, and verification plan. Do not edit production code as part of this skill unless the user also asks for implementation.
