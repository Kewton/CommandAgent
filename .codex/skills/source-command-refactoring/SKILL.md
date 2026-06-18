---
name: source-command-refactoring
description: Perform behavior-preserving CommandAgent refactoring. Use when the user asks for `/refactoring`, refactoring, code structure cleanup, module splitting, or reducing complexity without changing behavior.
---

# Source Command: Refactoring

Use `.codex/prompts/refactoring-core.md` for the detailed workflow.

## CommandAgent Rules

- Preserve behavior by default.
- Add or identify tests before risky moves.
- Keep public imports compatible or update all call sites in the same patch.
- Keep docs consistent when module ownership or runtime behavior changes.
- Do not reintroduce removed legacy systems.

## Completion

Report:

- what changed
- why behavior is preserved
- tests run
- docs checked or updated
- follow-up cleanup, if any

Commit only when the user asks.
