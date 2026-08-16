# Issue #80 Implementation Summary

## Implemented

- Added weak ETags to authenticated Trial status responses. The validator is
  derived from the confirmed Gate 1 card hash and current `events.jsonl`
  metadata. Matching `If-None-Match` requests return 304 before event-file
  reading, JSONL parsing, phase projection, or acceptance-sheet generation.
- Kept the `PolledSession` payload unchanged at its original ten fields. A
  focused integration assertion compares the exact JSON key set, and
  `tests/gui_read_only_guard.rs` pins the TypeScript shape.
- Extended `gui/lib/trial-monitor.ts` with a one-second changed-response
  interval and exponential unchanged-response backoff capped at ten seconds.
  The Trial page retains the last representation on 304, sends its ETag on the
  next GET, resets idle backoff on changed 200 responses, and preserves Issue
  #63's independently bounded transport-failure recovery in the same state
  machine.
- Changed static response caching so only `_next/static/**` receives
  `public, max-age=31536000, immutable`. Exported HTML and all other paths keep
  `Cache-Control: no-store`.
- Extended the browser smoke with a `--polling-only` mode and a real exported
  Trial-page virtual-clock probe. It records `observed_calls`, verifies every
  request after the initial 200 carries `If-None-Match`, and rejects results
  outside 50–65 calls or below a 90% reduction from the 801-call fixed-750 ms
  baseline.
- Recorded the mechanism in `docs/dev/mechanism-ledger.md` and added focused
  raw-HTTP response-header support to `tests/gui_server.rs`.

## Measured result

The final passing ten-minute smoke observed 60 status calls for both the root
deployment and `/proxy/commandagent/`, versus 801 at the former fixed interval.
Both cases sent the expected ETag on every conditional request and reduced
request count by approximately 92.51%.

## Compatibility and scope

- Event names, event bytes, `PolledSession`, and the live `.anvil/` namespace
  are unchanged. No corpus fixture update was required because no event,
  recovery, or corpus contract changed.
- Required predecessor commits are integrated. The new unchanged-response
  policy shares the existing `trial-monitor.ts` leaf with Issue #63 without
  weakening failure/reconnect behavior.
- No CommandMate process, release, historical evidence, or live `.anvil/`
  runtime state was changed.
