# Issue #80 Design

## Scope and predecessor compatibility

The required Issue #63, #64, #66, and #67 commits were inspected before this
design. They are verified sibling commits of this worktree's base rather than
ancestors, so this patch does not copy their unrelated monitoring recovery,
workspace-lease, lifecycle, or option-guidance UI. The polling policy will live
in a new `gui/lib/trial-polling.ts` leaf so Issue #63's failure/reconnect policy
can remain independent: transport failures use its retry backoff, while valid
unchanged responses use this Issue's idle backoff. The status handler and tests
are additive to the server changes in Issues #64 and #67, and no Issue #66
lifecycle state is changed.

## Status validation

- Keep the `PolledSession` Rust and TypeScript fields unchanged.
- After authorization, session-id validation, and confirmation loading, derive
  a weak ETag from the immutable confirmation hash plus the event file's length
  and nanosecond modification time. Missing event files use an explicit initial
  revision.
- Match `If-None-Match` before reading or parsing `events.jsonl`. An unchanged
  request returns 304 with no body, avoiding both full JSONL parsing and
  terminal acceptance-sheet regeneration. A changed request follows the
  existing projection path and returns the same JSON schema with the new ETag.
- Mark the authenticated status response `private, no-cache`: clients may
  retain the representation but must revalidate it, and authorization is still
  checked on every request.

## Adaptive polling

- Send the most recently observed ETag on later status GETs and retain the
  current `PolledSession` on 304.
- Poll one second after a changed response. Consecutive 304 responses double
  the interval up to ten seconds; any changed 200 response resets the interval
  to one second. A ten-minute unchanged run therefore needs about 64 status
  calls instead of about 800 at the old fixed 750 ms interval.
- Extend the browser smoke evidence with observed status-call counts and a
  ten-minute virtual-clock probe against the real exported Trial page. The
  probe returns one JSON representation followed by 304s, so it exercises the
  bundled scheduling code without waiting ten wall-clock minutes.

## Static response caching

- Return `public, max-age=31536000, immutable` only for files below
  `_next/static/**`, whose exported Next.js names are content-addressed.
- Keep `index.html` and all other static paths at `no-store`, preserving prompt
  discovery of new deployments and avoiding accidental long-lived caching of
  unhashed files.

## Verification

Add focused `gui_server` coverage for cache headers, status ETag/304 behavior,
and the exact status JSON key set. Extend the read-only guard for the adaptive
polling/conditional-request contract and smoke budget. Run the focused Rust
tests and GUI lint/type/build checks first, then formatting, Clippy, and the full
Rust suite because shared GUI server behavior is touched.
