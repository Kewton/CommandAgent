# Issue 71 Design

## Problem and boundaries

Repository evidence views read `workspace/management/runs`, but confirmed GUI
Trials live below `<execution-root>/.anvil/runs/<session-id>`. The Trial page
therefore has neither a bounded index of prior sessions nor a direct view of the
in-process workspace lease. Starting another Trial is currently the only way to
discover an active lease, through an HTTP 409 response.

This issue adds discovery and read-only lease projection. It does not change
event schemas, the live `.anvil` layout, CLI delegation, confirmation gates,
lease recovery, cancellation, or repository evidence routes.

## Predecessor review

The verified Issue 63, 64, 66, 67, 68, 69, 70, and 80 commits were inspected and
then integrated through the current `develop` branch before final verification.
They cover reconnect/poll recovery, lease recovery, lifecycle locking, Trial
options, phase projection, Gate 2 feedback, artifact viewing, and conditional
polling.

Issue 71 remains additive after that integration. Its session links use
`?session=<id>`, the memory-only GET reconnect boundary owned by Issue 63. Its
lease JSON reuses Issue 64's tagged `idle`, `running`, and `recovery_required`
shape without introducing a second lease representation.

## API design

- Add authenticated `GET /api/sessions` alongside the existing POST route.
- Read the execution root on every request; do not retain a server-side session
  index. Admit only real directories whose names are canonical UUIDs and which
  contain a regular JSON record in the existing boundary-confirmation
  directory. Ignore symlinks and unrelated runtime directories.
- Return at most 100 sessions. Put the current running/recovery lease session
  first, then order by newest observed modification time and ID. Each row
  contains ID, start epoch (UUID v7, falling back to events/run metadata), last
  modification epoch, and a conservative file-backed gate/status. The list
  omits acceptance-sheet generation; malformed or oversized events yield a
  null gate and `unreadable` status rather than a guessed lifecycle state.
- Include a snapshot of the existing lease mutex in the same response. The
  snapshot is read-only and identifies the session for `running` and
  `recovery_required`.
- Apply the existing Trial workspace and Bearer guard. GET does not require an
  Origin header, matching the existing session-status route.

## GUI design

- Load the session index after a complete-looking runtime token is entered and
  expose an explicit refresh action. Feed its lease snapshot into the existing
  Issue 64 lease card and show bounded session rows with start, update, gate,
  and status.
- Disable confirmed launch whenever the latest lease snapshot is non-idle and
  display the exact reason and owning session ID. Server-side acquisition
  remains authoritative if the snapshot becomes stale.
- Render each session action as a normal same-page link to `?session=<id>`.
  The action itself performs no fetch and no POST; after integration, Issue 63
  restores that ID and reconnects only through `GET /api/sessions/{id}`.

## Tests and verification

- Extend `tests/gui_server.rs` to cover unauthenticated 401, directory additions
  and removal between requests, status projection, the 100-row limit, and a
  live `running` lease with its session ID.
- Extend `tests/gui_read_only_guard.rs` to pin GET-only routing, authentication,
  read-only lease projection, reconnect-link shape, launch blocking, and the
  absence of session-list mutation controls.
- Run focused GUI server and read-only guard tests, GUI lint/typecheck/build,
  formatting and Clippy, then the full Rust suite because binary routing and a
  shared GUI page change. No corpus fixture changes are needed because no event,
  recovery, or corpus contract changes.
