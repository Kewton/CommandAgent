---
name: source-command-current-situation
description: Turn a reported CommandAgent problem into a structured bug report or GitHub issue draft. Use when the user asks for `/current-situation`, current situation, bug report drafting, or issue creation from observed behavior.
---

# Source Command: Current Situation

Use this skill to organize a bug or confusing behavior into a reproducible report.

## Flow

1. Extract observed behavior, expected behavior, reproduction steps, frequency, environment, and impact.
2. Gather relevant local evidence when available:
   - `.commandagent/logs`
   - `.commandagent/sessions`
   - command output in the conversation
   - relevant source files found with `rg`
3. Check for duplicate issues when GitHub access is available.
4. Draft the issue body.
5. Ask for user confirmation before creating a GitHub issue.

## Issue Template

```markdown
## Behavior

## Reproduction

## Expected

## Actual

## Frequency

## Environment

## Evidence

## Impact

## Related Code
```

Prefer the GitHub connector when available; use `gh` only when authenticated and appropriate.
