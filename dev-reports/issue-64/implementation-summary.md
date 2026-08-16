# Issue 64 implementation summary

## Outcome

GUI Trial no longer turns a child-not-started spawn failure into a permanent
workspace recovery lease. The server now reports the configured binary path and
the underlying OS error, removes only the new UUID-scoped run directory, and
returns the existing lease to `Idle`. If that rollback cannot complete, the
server deliberately retains `RecoveryRequired` and reports the rollback failure
instead of weakening the single-process gate.

An authenticated `GET /api/trial-workspace` endpoint now serializes the
existing lease mutex as `idle`, `running`, or `recovery_required`. Running and
recovery-required snapshots include the exact session ID. The Trial page exposes
this through a read-only card, refreshes it before Gate 1 and after HTTP 409, and
clears stale snapshots after a successful initial or continuation dispatch. No
reset, cancel, or alternate execution control was added.

## Files changed

- `src/bin/gui_server/workspace_policy.rs`: added a serializable snapshot of the
  existing lease state.
- `src/bin/gui_server/sessions.rs`: added the read-only handler, contextual spawn
  failure, and fail-closed rollback of an unstarted session.
- `src/bin/gui_server.rs`: routed authenticated lease inspection through GET.
- `gui/lib/types.ts`, `gui/app/try/page.tsx`, and `gui/app/globals.css`: added the
  typed read-only lease card and recovery-required session display.
- `tests/gui_server.rs`: covers missing binary -> 500 -> server restart on the
  same execution root -> corrected binary -> 202, plus authenticated recovery
  snapshot projection.
- `tests/gui_read_only_guard.rs`: pins the GET-only UI/API shape, authentication
  guard, rollback/error context, and absence of lease mutation controls.
- `docs/user/gui.md`: documents spawn retry and a conservative offline recovery
  procedure that preserves incomplete run evidence.

## Compatibility

Gate 1 confirmation and hashes, Origin and Bearer checks, CLI-only delegation,
event names and schemas, the `.anvil` layout, and single-process exclusivity are
unchanged. A child that actually started and exits without a current terminal
event still transitions to `RecoveryRequired`. No event/corpus contract changed,
so no corpus fixture was added.

Issue 63 (`4313d7ef`) and Issue 66 (`d6f0dec5`) were inspected as passed,
non-ancestor predecessors. Their reconnect and lifecycle-lock behavior was not
copied into this branch; the new lease snapshot is designed to compose with
both when those branches are integrated.
