# Issue 162 design: durable Gate 2 timing across reconnect

## Context

- A newly launched Trial keeps the Gate 1 proposal in React state and starts the
  Gate 2 clock from the browser's current time.
- Reload discards both values. The reconnect path currently substitutes a new
  browser start time and has no proposal, so elapsed time restarts at zero and
  the measured mean renders as `未記録`.
- Every confirmed session already has durable inputs for both values: its UUIDv7
  records the launch epoch, and its confirmation identity pins the same band
  source, task family, and arm used for the Gate 1 measured mean.

## Design

1. Add `started_epoch_seconds` to the accepted-create response and add both
   `started_epoch_seconds` and `average_duration_seconds` to the polled-session
   response. These are additive API fields; existing event names, event data,
   confirmation records, and `.anvil/` paths remain unchanged.
2. Reuse the session-index start-time calculation and the Gate 1 band-price
   calculation rather than introducing a second clock or pricing rule. The
   status endpoint derives the values from the confirmed session on every
   non-304 response.
3. Initialize the browser clock from the accepted-create epoch, refresh it from
   each polled response, and restore it from the reconnect response. Render the
   measured mean from the polled session when present, falling back to the
   in-memory Gate 1 proposal before the first status response.
4. Extend the focused Gate 2 browser smoke to launch with a fixed server start
   epoch, advance time, reload, reconnect by the URL session ID, and assert that
   elapsed time does not reset and the measured mean stays identical.

## Verification strategy

- Update the GUI server integration test to pin the new create/status fields and
  prove the status mean equals the original Gate 1 proposal.
- Run GUI syntax, typecheck, lint, build, the two-base-path feedback smoke, the
  focused Rust GUI guard/server tests, then repository formatting, Clippy, and
  the default and GUI-feature test suites because the shared session response
  contract changes.

## Non-goals

- No browser-storage cache for timing or pricing, automatic reconnect, new
  mutation endpoint, event/schema migration, historical evidence rewrite, or
  live `.anvil/` state migration.
