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
