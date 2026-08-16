# Issue 68 Design

## Problem

The Gate 2 phase projection currently defaults a missing `phase_index` to zero
and treats every event carrying a `phase_id` as running unless it is a phase
completion or failure. In the GUI smoke trace, the recovery event after the
failed phase therefore creates a second `00 setup-project RUNNING` row.

## Design

- Admit an event to the phase projection only when it has both a string
  `phase_id` and an unsigned `phase_index`.
- Project only the known phase lifecycle events. Unknown events do not create
  or mutate a phase row.
- Keep `failed` terminal once observed. Keep `completed` terminal unless a
  later explicit `ultra_phase_failed` records the stricter outcome. Progress
  events may update the stage only while the phase is non-terminal.
- Preserve `PhaseStatus` and `PolledSession` field names and shapes.
- Give pending, running, completed/passed, and failed badges separate neutral,
  accent, success, and danger treatments.

## Tests and verification

- Add Rust unit coverage for unindexed events, terminal-state stickiness, and
  failure stickiness.
- Parse `workspace/management/runs/g1-gui-smoke/root-events.jsonl` directly as
  a fixture and assert its one-row failed projection.
- Add a GUI source guard for the three explicit non-pending status styles.
- Run the focused GUI server/unit and read-only guard tests first, followed by
  formatting, Clippy, and the full Rust test suite because the shared GUI API
  projection is changing.

## Predecessor compatibility

Issues 63 and 66 change polling/lifecycle UI behavior around this phase list;
Issues 64 and 67 touch nearby server/type code. Their committed changes do not
change `phase_statuses` or the `PolledSession` phase fields, so this patch stays
localized and avoids depending on those branches being merged here.
