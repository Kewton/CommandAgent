---
name: work-plan
description: Turn a CommandAgent issue and approved design into an executable, issue-scoped implementation plan. Use before coding when tasks, file boundaries, tests, ordering, or completion gates need to be made explicit.
---

# CommandAgent Work Plan

Create a plan grounded in the current repository. Do not implement it as part of this skill.

## Workflow

1. Read the issue, acceptance criteria, design/review artifacts, and relevant code/tests.
2. Confirm the current behavior and constraints; label uncertain file impacts as hypotheses.
3. Split work into ordered, independently verifiable steps:
   - tests or fixtures that establish the behavior
   - smallest production changes
   - integration/compatibility changes
   - documentation or examples
   - focused and broader verification
4. For each step, list purpose, likely files, dependencies, proof command, and rollback concern.
5. Call out runner-growth, event-schema, profile/corpus, provider, CLI, TUI, and release risks when applicable.
6. Define a Definition of Done directly mapped to issue acceptance criteria.

Prefer new focused modules when `docs/dev-guardrails.md` identifies a chokepoint. Do not plan baseline increases merely to admit growth.

## Output

Write `dev-reports/issue-<number>/work-plan.md` with scope, assumptions, ordered tasks, affected files, test matrix, risk controls, and Definition of Done. End with blocking questions and the first executable step.
