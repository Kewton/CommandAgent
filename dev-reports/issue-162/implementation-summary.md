# Issue 162 implementation summary

## Outcome

GUI Trial Gate 2 now restores its original elapsed-time origin and measured
average duration after a same-tab reload or explicit reconnect. The values are
reconstructed from confirmed server-side session data instead of transient
React state or browser storage.

## Changes

- Added `started_epoch_seconds` to accepted session creation responses and
  added `started_epoch_seconds` plus `average_duration_seconds` to session
  status responses.
- Moved the existing UUIDv7/filesystem start-time calculation into the shared
  session module so the create response, status response, and session index use
  one implementation.
- Reused the Gate 1 band-price calculation against the persisted confirmation
  identity, preserving the same task-family/band-arm measurement semantics on
  reconnect.
- Updated the Trial hook to initialize and restore elapsed time from the server
  epoch, clamp clock skew at zero, and prefer the polled measured mean while
  retaining the Gate 1 proposal as the pre-poll fallback.
- Extended the focused Gate 2 Playwright smoke to reload an active run,
  reconnect by its URL session ID, and assert elapsed time and mean preservation
  for both root and proxied base paths.
- Updated the Rust API/source guards and the English/Japanese Trial guidance for
  the reconnect behavior.

## Compatibility

- Session response changes are additive. Event names and payloads,
  confirmation-record schemas, filesystem paths, and the live `.anvil/`
  namespace are unchanged.
- Reconnect remains GET-only and cannot dispatch or mutate a Trial session.
- No historical evidence was rewritten.

## Follow-up: authentication retry

- Reproduced the Issue 158 blocker with a freshly rebuilt release candidate.
  Stack locations showed that the background session-index 401 cleared the
  token before the first explicit reconnect click could dispatch; the failure
  was a production interaction race rather than a Playwright fill race.
- Deferred only automatic session-index revalidation while the compose screen
  has a concrete reconnect target. The explicit reconnect now owns rejection,
  clears the bad token, and leaves the form ready for a valid retry. Manual
  history refresh and automatic refresh outside direct reconnect remain intact.
- Extended the session-index smoke for both base paths with the exact wrong
  token rejection, storage removal, valid-token retry-enabled, and GET-only
  successful reconnect assertions.
- Kept every full-smoke gate. Updated its stale editable-control cardinality
  from six to seven so the existing assertion checks the added planner-model
  control instead of reporting a false failure.

The rebuilt candidate completed the full root and proxy Trial flows. Both
recorded an explicit reconnect sequence of HTTP 401 followed by HTTP 200, and
both preserved elapsed time and the measured mean across reconnect.

## CI follow-up: Rust 1.97/1.98 session-file errors

- Cherry-picked only Issue 160 code commit `714017ca` as `b4221a12`; the Issue
  160 report commit and report files were not applied.
- Boxed the session-file handler error response behind `SessionFileError` so
  Rust 1.97.1 Clippy accepts the GUI server without a lint allowance.
- The wrapper returns the original `Response` unchanged. Status, headers, JSON
  bytes, path-confinement checks, bounded reads, and symlink rejection retain
  their existing behavior.
- No Issue 162 GUI timing or reconnect production code changed in this
  follow-up.
