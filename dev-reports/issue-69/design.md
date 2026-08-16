# Issue 69 Design

## Problem

Gate 2 needs visible execution feedback beyond the latest file-backed status.
A long run should show how long this browser has observed it, how that duration
compares with the measured mean from Gate 1, and which projected phase is
active. Reaching Gate 3/4 should also be visible from the browser tab.

## Predecessor review

The current `develop` base includes the merged work for Issues 63, 64, 66, 67,
68, 76, 77, and 80. Those changes provide monitoring recovery, workspace-lease
recovery, lifecycle locking, server-derived Trial options, Japanese UI,
accessibility fixes, corrected phase projection, and conditional adaptive
polling. Issue 69 composes with those implementations rather than duplicating
or weakening them.

The new feedback is derived only from existing client state. It does not change
`PolledSession`, phase/event schemas, polling cadence, recovery policy, cache
validation, server routes, or the live `.anvil/` namespace. Monitoring state
remains separate from execution progress.

## Design

- Record browser time when a confirmed session response is accepted, or when
  an existing session is reconnected, and update elapsed time once per second
  while Gate 2 is active. Stop the interval at terminal.
- Place elapsed time beside the existing measured mean and explicitly label the
  mean as a comparison, not an ETA guarantee.
- Select the running projected phase when present, otherwise the latest phase.
  Render `Phase x / N` from `index` and payload `total` only when `total > 0`.
- On Gate 3/4, change `document.title` to `✔ <outcome> — CommandAgent` and
  restore the prior route title when leaving terminal.
- Keep stable test IDs for elapsed, measured mean, and phase progress.

## Tests and verification

- Add a feedback-only Playwright mode for both supported base paths. The probe
  uses a virtual clock and mocked Trial responses to verify one-second elapsed
  updates, hidden zero totals, `Phase 2 / 5`, the measured mean and non-ETA
  label, separation from monitor health, and the terminal title.
- Run the same probe from the normal smoke path without changing its existing
  lifecycle, reconnect, failure-recovery, or conditional-polling coverage.
- Extend the focused GUI source guard and run GUI syntax, lint, typecheck,
  export builds, repository formatting, Clippy, and the full Rust test suite.
