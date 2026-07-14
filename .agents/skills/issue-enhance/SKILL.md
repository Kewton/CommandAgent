---
name: issue-enhance
description: Clarify and complete an existing CommandAgent GitHub issue using repository evidence. Use when an issue lacks acceptance criteria, scope, affected areas, reproduction details, or implementation-ready context.
---

# CommandAgent Issue Enhance

Improve an issue without silently changing its intent.

## Workflow

1. Read the issue with `gh issue view <number> --json number,title,body,labels,assignees` when available.
2. Classify the issue and identify missing sections, ambiguity, hidden assumptions, and unverifiable acceptance criteria.
3. Inspect the smallest relevant code, tests, documentation, and guardrails. Mark inferred file impacts as hypotheses.
4. Ask only questions whose answers materially change scope, behavior, compatibility, or safety. Continue with clearly labeled assumptions when safe.
5. Prepare a revised body that preserves useful original context and adds:
   - objective and background
   - requirements and non-goals
   - reproduction/expected/actual behavior for bugs
   - affected modules and compatibility constraints
   - implementation tasks at a useful, non-prescriptive level
   - testable acceptance criteria and verification
   - dependencies and risks
6. Show the meaningful changes before publication.
7. Run `gh issue edit` only when the user explicitly authorizes updating GitHub.

Do not overwrite the issue merely to normalize prose. Do not claim that code investigation proved a cause unless evidence supports it.

## Completion Report

State what was clarified, remaining assumptions, repository evidence consulted, and whether the result is a local draft or a published update.
