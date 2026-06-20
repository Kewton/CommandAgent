---
name: source-command-acceptance-test
description: Create or run CommandAgent acceptance tests from issue criteria. Use when the user asks for `/acceptance-test`, acceptance testing, or validation against acceptance criteria.
---

# Source Command: Acceptance Test

Use `.codex/prompts/acceptance-test-core.md` for the detailed workflow.

## Rules

- Map every acceptance criterion to a test, check, or explicit manual UAT item.
- Prefer focused unit or integration tests over broad test churn.
- Do not introduce a separate `tests/acceptance` layout unless the repo adopts it.
- Keep live providers and network out of normal checks unless explicitly requested.

## Completion

Report the acceptance matrix, commands run, results, untested criteria, and follow-up.
