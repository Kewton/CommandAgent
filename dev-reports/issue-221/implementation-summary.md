# Issue #221 implementation summary

## Implemented

- Fast-forwarded the worktree through verified predecessor Issue #227 commit
  `3a0b1f6e`, consuming its additive
  `commandagent.headless-summary/v1` fields and typed terminal overrides without
  changing their schema.
- Carried the outer `--summary-json` source through the CLI panic boundary into
  the existing direct-command SIGINT finalizer. Workflow child runs receive no
  source, so the outer command remains the sole owner of the final JSON line.
- Preserved the audited `DirectCommandCompletionGuard::start(&config)` path for
  ordinary commands and added a summary-aware constructor only when the flag is
  active.
- After SIGINT terminal evidence and notification state are closed, render one
  final JSON line with `status = interrupted` and `exit_code = 130`. The writer
  requires at least one valid persisted event, flushes the line, and keeps
  stdout locked until process exit so no later stdout can follow it.
- Mapped interruption errors found anywhere in the `anyhow` cause chain to
  process exit `130`; pack/CLI rejection remains `2` and other execution or
  validation failures remain `1`.
- Updated the headless guide with the SIGINT evidence boundary, aligned exit
  codes, and all seven additive Issue #227 fields.

## Coverage

- Added unit coverage for nested interruption exit mapping and for suppressing
  interruption JSON when the event path is absent or empty.
- Added a Unix process-level regression that starts a real headless child,
  waits for evidence, sends SIGINT, and verifies process exit `130`, persisted
  interrupted terminal evidence, and the v1 JSON as the final stdout line.
- Rechecked the headless corpus, documentation drift, and terminal protection
  audit in addition to formatting, warning-denied Clippy, and the full suite.

## Compatibility and failure semantics

- No event name, event field, runtime namespace, or schema version changed.
- No acceptance, assurance, verification, release-gate, or terminal failure
  condition was weakened. The interruption overrides affect only the terminal
  process/status projection after evidence already exists.
