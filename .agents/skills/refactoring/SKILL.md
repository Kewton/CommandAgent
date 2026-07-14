---
name: refactoring
description: Improve CommandAgent code structure without intentional behavior changes. Use when the user asks to simplify, extract, rename, reduce duplication, or shrink a guarded module while preserving contracts.
---

# CommandAgent Refactoring

Prove behavior preservation before and after the change.

## Workflow

1. Define the exact target, structural problem, and behaviors/contracts that must remain unchanged.
2. Read relevant tests, public interfaces, event output, and `docs/dev-guardrails.md`.
3. Run focused baseline tests before editing. Add characterization tests when behavior is not adequately pinned.
4. Refactor in small coherent slices. Avoid mixing feature work with structural changes.
5. After each slice, run the narrowest useful tests and inspect the diff for semantic drift.
6. Run formatting, Clippy, and broader tests proportional to the touched surface.
7. Compare protected-file line counts when the goal involves runner or minimal-loop extraction.

Do not raise growth baselines to make a refactor pass. Do not change event schemas, CLI/config contracts, or profile behavior unless the user separately authorizes that behavior change.

## Completion

Report the structural improvement, preserved contracts, before/after evidence, tests run, and any follow-up opportunity. Commit only when requested.
