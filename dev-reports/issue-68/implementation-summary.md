# Issue 68 Implementation Summary

## Implemented

- Changed the Gate 2 phase projection to require an unsigned `phase_index`
  instead of synthesizing index zero for phase-adjacent events.
- Limited projection updates to the existing phase lifecycle vocabulary, so
  unknown events cannot create or revive a running row.
- Made explicit phase outcomes monotonic: failed stays failed, completed is not
  downgraded by later progress, and a later explicit failure may still replace
  completed to preserve the stricter outcome.
- Split the phase badges into neutral pending, accent running, success
  completed/passed, and danger failed treatments.

## Tests

- Added unit tests for unindexed lifecycle events, post-completion progress,
  post-failure progress and unknown events, and the recorded GUI smoke trace.
- The smoke test reads the immutable
  `workspace/management/runs/g1-gui-smoke/root-events.jsonl` evidence directly
  and asserts one `setup-project` row at index 1 with failed status.
- Added a read-only GUI source guard that pins distinct running, completed, and
  failed badge declarations; pending retains the neutral base declaration.

## Compatibility

`PhaseStatus` and `PolledSession` were not changed. The JSON field names and
response shape remain byte-compatible at the schema level; only the projected
phase row contents are corrected. No historical run evidence or `.anvil/`
runtime state was modified.
