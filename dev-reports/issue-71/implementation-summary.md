# Issue 71 Implementation Summary

## Outcome

The GUI server now exposes an authenticated `GET /api/sessions` projection of
the configured execution root. It scans `.anvil/runs` on every request, admits
canonical UUID directories with a regular persisted confirmation record, and
returns at most 100 rows with the active lease session first. Each row includes
its ID, UUID-v7 start epoch (with file metadata fallback), observed modification
epoch, and a bounded file-backed gate/status. Malformed, non-UTF-8, or oversized
event streams use a null gate and `unreadable` status rather than a guess.

The same response carries a read-only snapshot of the existing workspace lease
as `idle`, `running { session_id }`, or
`recovery_required { session_id }`. Terminal status precedence matches the
existing detail API: a later process-level `run_stop` cannot overwrite a failed
`tui_command_stop`, and a confirmed continuation returns the summary to
`running` until a new terminal event exists.

## GUI

The Trial page loads the index after a complete-looking memory-only token is
entered and also provides an explicit refresh action. It updates the existing
lease card, shows session start/update/gate/status rows, and disables confirmed
launch with the exact owning/blocking session reason whenever the snapshot is
non-idle. The server-side lease acquisition
continues to be authoritative if the displayed snapshot becomes stale.

Each row uses a normal same-page `?session=<id>` link. The list action contains
no POST or alternate dispatch path and composes with Issue 63's GET-only
reconnect flow. No token is placed in the URL or browser storage.

## Files and compatibility

- `src/bin/gui_server/session_index.rs`: bounded execution-root scan, status
  projection, and authenticated index handler.
- `src/bin/gui_server.rs` and `src/bin/gui_server/sessions.rs`: minimal route and
  shared guard wiring against the integrated Issue 64 lease contract.
- `gui/components/trial-session-index.tsx`, `gui/app/try/page.tsx`,
  `gui/lib/types.ts`, and `gui/app/globals.css`: leaf history/lease
  presentation, reconnect links, and minimal launch-block wiring.
- `tests/gui_server.rs` and `tests/gui_read_only_guard.rs`: authentication,
  add/remove reflection, 100-row bound, status honesty, live lease, GET-only
  navigation, and read-only capability coverage.
- `docs/user/gui.md`: execution-root index, lease, and reconnect guidance.

Issues 63, 64, 66, 67, 68, 69, 70, and 80 were integrated from `develop`, and
Issue 71 was reconciled with their reconnect, lease, monitoring, and artifact
surfaces. Event names and schemas, `.anvil` layout, repository evidence routes,
CLI delegation, confirmation, recovery, and corpus contracts are unchanged, so
no corpus fixture or state migration was needed.
