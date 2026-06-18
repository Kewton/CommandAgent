---
name: source-command-progress-report
description: Generate a CommandAgent repository progress report from git, GitHub, tests, worktrees, and code metrics. Use when the user asks for `/progress-report`, status, project progress, or management summary.
---

# Source Command: Progress Report

Use `.codex/prompts/progress-report-core.md` for the report structure.

## Rules

- Base the report on observed data.
- Use `N/A` for unavailable GitHub, CI, or metric data.
- Separate facts from recommendations.
- If `cloc` exists, split Rust source and test code when useful.
- Do not run expensive tests or live provider checks unless the user asks.

## Completion

Report the output path if a file was requested; otherwise provide the report in the final response.
