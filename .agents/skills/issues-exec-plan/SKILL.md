---
name: issues-exec-plan
description: Build a dependency-aware execution plan across open CommandAgent issues. Use when the user asks for ordering, batching, prioritization, a critical path, or parallel work recommendations.
---

# CommandAgent Issues Execution Plan

Plan from current issue and repository evidence; do not mutate issues or branches.

## Workflow

1. Fetch the requested open issues, optionally filtered by labels or milestone.
2. Extract each issue's objective, priority, dependencies, likely code surface, acceptance criteria, and readiness gaps.
3. Inspect shared files and contracts where overlap is uncertain.
4. Build a directed dependency graph. Distinguish hard dependencies from sequencing preferences.
5. Group issues into executable batches up to the user's parallelism limit.
6. Identify the critical path, merge order, integration risks, and issues that need enhancement or splitting first.
7. Define gates for each batch and after integration.

Default integration checks are proportional to risk and may include:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

Respect `docs/dev-guardrails.md`, especially runner growth budgets and declarative knowledge contracts. Preserve event schemas unless an issue explicitly changes them.

## Output

Write `dev-reports/issues-exec-plan.md` when a repository artifact is requested; otherwise return the plan directly. Include issue readiness, dependency graph, batches, merge order, risks, quality gates, and blocking questions. Use `$orchestrate` when the user wants the plan dispatched through CommandMate.
