---
name: commandagent-uat
description: Plan and record CommandAgent UAT checks for CLI, REPL, slash-command, release, provider, or manual terminal behavior. Use when changes need user-acceptance validation beyond unit tests.
---

# CommandAgent UAT

Use this skill after one or more changes are ready for acceptance validation.

## Output

Write the report under `workspace/management/runs/<run_id>/uat-report.md` unless the user requests another path.

Include:

- change or issue scope
- acceptance scenarios
- automated checks and results
- manual CLI, REPL, TTY, release, or provider steps when relevant
- expected results
- evidence to collect on failure
- pass/fail status
- follow-up fix prompt when UAT fails

## CommandAgent Checks

Use CommandAgent-specific surfaces:

- `commandagent --help`
- `commandagent --version`
- provider-free CLI paths
- interactive `commandagent>` REPL behavior
- `/plan-run` and `/ultra-plan-run` when planning behavior changed
- `.commandagent/` sessions, plans, repairs, and logs when state behavior changed

Do not run live provider checks unless the user explicitly asks.
