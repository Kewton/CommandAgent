---
name: source-command-work-plan
description: Create a concrete CommandAgent work plan for an issue, task, or requested change. Use when the user asks for `/work-plan`, work planning, task breakdown, or implementation planning.
---

# Source Command: Work Plan

Use this skill to plan one coherent task before implementation.

## Flow

1. Read the issue, user request, and relevant docs.
2. Inspect the smallest relevant code surface.
3. Identify responsible layers using `AGENTS.md` and `docs/architecture.md`.
4. Break the task into implementation, test, docs, and verification steps.
5. State assumptions, dependencies, risks, and stop conditions.
6. Write the plan to the requested path. If no path is given, prefer ignored workspace state.

## Plan Contents

- scope and non-goals
- files likely to change
- phase/task list
- test plan
- documentation impact
- verification commands
- rollback or replan triggers

Do not edit production code while producing a planning-only artifact.
