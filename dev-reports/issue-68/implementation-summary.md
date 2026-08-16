# Issue 68 Implementation Summary

## Implemented

- Stopped synthesizing index zero. Recognized unindexed context/recovery events
  now attach to the latest existing row with the same `phase_id`; without an
  existing row they are ignored.
- Split phase events into status-changing lifecycle events, stage-only
  auxiliary events, and ignored unknown events.
- Made explicit phase outcomes monotonic: failed stays failed, completed is not
  downgraded by later progress, and a later explicit failure may still replace
  completed to preserve the stricter outcome.
- Converted pending/running rows to explicit `interrupted` status at
  `tui_command_stop` or `run_stop`, and prevented later progress from reviving
  them.
- Split the phase badges into neutral pending, accent running, success
  completed/passed, danger failed, and warning interrupted treatments.

## Tests

- Added unit tests for unindexed attachment without ghost rows, terminal
  interruption, post-completion progress, post-failure auxiliary/unknown
  events, and the recorded GUI smoke trace.
- The smoke test reads the immutable
  `workspace/management/runs/g1-gui-smoke/root-events.jsonl` evidence directly
  and asserts one `setup-project` row at index 1 with failed status.
- Added a read-only GUI source guard that pins distinct pending, running,
  completed, failed, and interrupted badge declarations.

## Compatibility

`PhaseStatus` and `PolledSession` were not changed. The JSON field names and
response shape remain byte-compatible at the schema level; only the projected
phase row contents are corrected. No historical run evidence or `.anvil/`
runtime state was modified.
