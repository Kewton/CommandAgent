# Issue 69 Design

## Problem

Gate 2 currently exposes only the latest file-backed status. A long run can
therefore remain a green `running` label for many minutes without showing how
long the browser has observed it, how that duration compares with the measured
mean from Gate 1, or which projected phase is active. Reaching Gate 3/4 also
does not create a notification outside the page body.

## Predecessor review

Issues 63 (`4313d7ef`), 64 (`7fcb0dbe`), 66 (`d6f0dec5`), 67
(`f51c20b5`), 68 (`73f57e8d`), and 80 (`b84034b6`) are complete sibling
commits of this worktree's base rather than ancestors. Their committed changes
cover monitoring recovery, workspace-lease recovery, lifecycle locking,
server-derived Trial options, phase projection correctness, and conditional
adaptive polling. This patch will not duplicate or merge those histories.

The new feedback is deliberately derived from existing client state. It does
not change `PolledSession`, phase/event schemas, polling cadence, recovery
policy, cache validation, server routes, or the live `.anvil/` namespace. The
phase display consumes the existing `PhaseStatus.total`, so it composes with
Issue 68's corrected projection. The elapsed timer is independent of both the
healthy/failure schedule from Issue 63 and unchanged-response schedule from
Issue 80.

## Design

- Record the browser time when a confirmed session is accepted and update a
  displayed elapsed duration once per second while Gate 2 is active. Stop the
  interval at terminal so the final Gate 2 duration remains stable.
- Place elapsed duration beside the already-computed measured mean duration in
  the Gate 2 header. Keep `not recorded` honest when Gate 1 has no duration
  sample.
- Select the running projected phase when present, otherwise the latest phase,
  and render `Phase x / N` from its `index` and payload `total`. Show a neutral
  placeholder before the first phase event.
- On Gate 3/4, change `document.title` to a completion title containing the
  terminal gate and outcome. Restore the previous title when leaving the
  terminal state.
- Add stable test IDs for elapsed, measured mean, and phase progress so mobile
  and browser checks do not depend on visual layout.

## Tests and verification

- Extend `gui/scripts/smoke.mjs` with a feedback-only mode and a deterministic
  Playwright probe. It mocks proposal, creation, and `PolledSession` responses,
  waits a little over two real seconds, asserts the elapsed display changes,
  checks `Phase 2 / 5` and `10.2 min mean`, then switches the mock to Gate 4 and
  asserts the tab title changed. Run the same probe as part of the normal smoke
  for both supported base paths.
- Extend the focused GUI source guard for the timer, total-based phase display,
  terminal title update, and mocked browser assertions.
- Run GUI syntax/lint/typecheck/build and the focused feedback smoke first.
  Then run the GUI guard and the repository formatting, Clippy, and full test
  checks because the required worker workflow calls for broad handoff checks.
