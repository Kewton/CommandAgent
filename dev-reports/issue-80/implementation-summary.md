# Issue #80 Implementation Summary

## Implemented

- Added weak ETags to authenticated Trial status responses. The validator is
  derived from the confirmed Gate 1 card hash and current `events.jsonl`
  metadata. Matching `If-None-Match` requests return 304 before event-file
  reading, JSONL parsing, phase projection, or acceptance-sheet generation.
- Kept the `PolledSession` payload unchanged at its original ten fields. A
  focused integration assertion compares the exact JSON key set, and
  `tests/gui_read_only_guard.rs` pins the TypeScript shape.
- Added `gui/lib/trial-polling.ts` with a one-second changed-response interval
  and exponential unchanged-response backoff capped at ten seconds. The Trial
  page retains the last representation on 304, sends its ETag on the next GET,
  and resets the backoff on every changed 200 response.
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

The passing ten-minute smoke observed 58 status calls for the root deployment
and 57 for `/proxy/commandagent/`, versus 801 at the former fixed interval.
Both cases sent the expected ETag on every conditional request and reduced
request count by approximately 92.8%.

## Compatibility and scope

- Event names, event bytes, `PolledSession`, and the live `.anvil/` namespace
  are unchanged. No corpus fixture update was required because no event,
  recovery, or corpus contract changed.
- Required predecessor commits #63, #64, #66, and #67 were inspected as
  non-ancestor sibling commits. Their unrelated UI behavior was not copied.
  The new unchanged-response policy remains in a separate leaf module from
  Issue #63's transport-failure/reconnect policy so both can be combined when
  the dependency branches are integrated.
- No pull request, merge, release, CommandMate action, or external issue state
  was changed.
