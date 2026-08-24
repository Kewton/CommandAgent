---
name: issue-split
description: Decompose a large CommandAgent issue into independently testable sub-issues with explicit dependencies. Use when one issue spans multiple layers, phases, pull requests, or independently releasable outcomes.
---

# CommandAgent Issue Split

Produce a dependency-aware split before creating any GitHub state.

## Workflow

1. Read the parent issue and inspect the relevant repository boundaries.
2. Confirm that splitting is warranted: multiple independent outcomes, risky cross-layer work, or a sequence that cannot be reviewed coherently in one pull request.
3. Define sub-issues around observable deliverables, not arbitrary file counts.
4. For every proposed sub-issue, provide:
   - title and objective
   - scope and non-goals
   - likely affected modules
   - acceptance criteria
   - verification
   - dependencies and what it unblocks
5. Draw the dependency order and identify work that may run in parallel.
6. Check that the combined sub-issues cover the parent without overlap or gaps.
7. Present the split plan. Create sub-issues and comment on the parent only when explicitly authorized.

When publishing, use repository `Kewton/CommandAgent`, preserve parent context, link every created issue, and avoid circular dependencies. Do not close the parent unless the user explicitly asks.

## Completion Report

List the parent, created or proposed child issues, dependency batches, critical path, parallel candidates, and any unresolved scope decision.
