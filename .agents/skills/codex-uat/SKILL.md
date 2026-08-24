---
name: codex-uat
description: Prepare and record CommandAgent user-acceptance checks, including CLI, TTY, release, GUI, and real-device evidence. Use after selected changes have reached develop or when the user explicitly requests UAT planning or execution.
---

# CommandAgent UAT

Derive reproducible acceptance scenarios from the selected issues. Read [manual-check.md](references/manual-check.md) whenever a scenario needs human CLI/TTY, GUI, release, or real-device confirmation.

## Required Output

Write `workspace/management/runs/<run_id>/uat-report.md` with:

- selected issues and tested revision
- acceptance scenarios derived from the issues
- automated checks, commands, and results
- manual CLI/TTY/GUI/release or real-device steps when relevant
- expected results
- evidence to collect on failure
- pass, fail, or blocked status per scenario
- a `$codex-issue-worker` follow-up prompt when UAT fails

Do not overwrite an existing historical run report. Use a new run ID. Treat unclear, unsafe, or non-reproducible UAT as blocked and state the missing evidence.
