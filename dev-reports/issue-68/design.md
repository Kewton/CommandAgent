# Issue 68 Design

## Problem

The Gate 2 phase projection currently defaults a missing `phase_index` to zero
and treats every event carrying a `phase_id` as running unless it is a phase
completion or failure. In the GUI smoke trace, the recovery event after the
failed phase therefore creates a second `00 setup-project RUNNING` row.

## Design

- Let indexed phase lifecycle events create or update a phase row. A recognized
  event without `phase_index` may update the latest existing row with the same
  `phase_id`, but it cannot create an index-zero ghost row.
- Separate status-changing lifecycle events from known auxiliary events.
  Context and recovery events update only the displayed stage; unknown events
  neither create nor mutate a phase row.
- Keep `failed` terminal once observed. Keep `completed` terminal unless a
  later explicit `ultra_phase_failed` records the stricter outcome. Later
  auxiliary events may update the displayed stage without changing that
  terminal status.
- When `tui_command_stop` or `run_stop` is observed, project every remaining
  pending/running row as `interrupted` and never revive it as running.
- Preserve `PhaseStatus` and `PolledSession` field names and shapes.
- Give pending, running, completed/passed, and failed badges separate neutral,
  accent, success, and danger treatments. Give the explicit interrupted state
  a separate warning treatment.

## Tests and verification

- Add Rust unit coverage for unindexed attachment without ghost rows, global
  terminal interruption, terminal-state stickiness, and failure stickiness.
- Parse `workspace/management/runs/g1-gui-smoke/root-events.jsonl` directly as
  a fixture and assert its one-row failed projection.
- Add a GUI source guard for the four explicit non-pending status styles.
- Run the focused GUI server/unit and read-only guard tests first, followed by
  formatting, Clippy, and the full Rust test suite because the shared GUI API
  projection is changing.

## Predecessor compatibility

Issues 63 and 66 change polling/lifecycle UI behavior around this phase list;
Issues 64 and 67 touch nearby server/type code. Their committed changes do not
change `phase_statuses` or the `PolledSession` phase fields, so this patch stays
localized and avoids depending on those branches being merged here.
