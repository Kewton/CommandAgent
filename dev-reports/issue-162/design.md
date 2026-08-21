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

## Follow-up: authentication retry race

### Reproduction context

- Issue 158's rebuilt full-smoke candidate completed a Trial, navigated back
  from its session-index row, submitted `<valid-token>-wrong`, observed the
  definitive rejection and cleared field, then entered the valid token again.
- The second reconnect button stayed disabled until Playwright's unchanged
  30-second click timeout. The captured page is at the editable compose stage
  with the session ID present and the controlled token field empty.
- The Trial page can have more than one authenticated request in flight: the
  explicit reconnect and the session-index revalidation each report definitive
  rejection through the shared token callback. The current callback protects
  against clearing a different current string, but the ordering and request
  ownership need to be measured under the exact full-smoke sequence.

### Follow-up design boundary

1. Rebuild the release candidate and run the original full smoke unchanged to
   reproduce the two-request failure without weakening its 30-second action,
   GET-only reconnect, token-removal, root/proxy, or terminal assertions.
2. Add only focused diagnostics/assertions needed to classify request ordering.
   Treat a delayed response as authoritative only for the exact token version
   that originated that request; a newer user edit must remain authoritative.
3. If production can clear a newer edit, fix the shared token owner with the
   smallest request-safe state rule and cover overlapping rejection/user-edit
   order deterministically. If production state is already protected, fix only
   a demonstrated harness sequencing race and retain every original gate.
4. Rebuild after the fix and require the unchanged full smoke to complete both
   `/` and `/proxy/commandagent/`, followed by the Issue 162 feedback smoke and
   proportional GUI/Rust verification.

The first complete post-fix run also exposed a stale cardinality inside the
existing lifecycle assertion: the locator covers seven editable identity
controls after the planner-model field was added, while `allEnabled` still
required exactly six. Correct the cardinality to seven so the same assertion
checks every current control; do not remove, bypass, or relax the assertion.

### Diagnosis

- Both the Issue 158 evidence and the clean rebuilt reproduction point to the
  first reconnect click (`smoke.mjs` line 833 there and line 791 here), not the
  later valid-token retry. No direct session GET can be dispatched before that
  timed-out click.
- Filling the complete wrong token starts the session index's automatic GET.
  Its 401 clears the shared controlled token between Playwright's `fill` and
  `click`, so the reconnect button becomes disabled. This is a production
  interaction race exposed by the harness, not a Playwright input race.
- A compose screen with a concrete reconnect target should make the explicit
  reconnect request authoritative. Automatic index revalidation is deferred in
  that state; manual index refresh remains available, ordinary history screens
  retain automatic revalidation, and successful reconnect lifts the deferral.

### Follow-up non-goals

- No retry auto-submit, button-force click, longer timeout, ignored 401, relaxed
  token-removal assertion, skipped full Trial case, or proxy-only substitution.
