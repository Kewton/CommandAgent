# Issue #355 implementation summary

## Baseline

The committed predecessor chain for Issues #352, #353, and #354 was integrated before
editing so the fix is based on the session-isolated, role-aware GUI Trial implementation.

## Changes

- Added one committed GUI contract manifest shared by the static export and
  `gui_server`. Runtime status exposes the server marker, the shell renders a Japanese
  mismatch banner, and `gui_server --check` rejects a missing or different export marker.
- Made the client-side session `identity` optional. An old server response now keeps the
  terminal rendered and shows Japanese rebuild/restart guidance instead of reaching the
  Next.js error page.
- Added a bounded, additive failure-diagnostics projection for `stop_reason`, release-gate
  reasons, and probe status/reasons/evidence paths. The session list and terminal both link
  FAILED state to those recorded causes without changing existing event names or schemas.
- Unified read-only session lookup across canonical `.commandagent/runs` and legacy
  `.anvil/runs`, while preserving canonical-first selection, real-directory checks,
  symlink rejection, and canonical-only write paths.
- Documented rebuilding the static GUI and `gui_server` from the same checkout with
  `cargo build --features gui --bin gui_server`, plus `--check` and banner remediation.

## Tests

- Extended GUI-server integration coverage for contract mismatch detection, legacy-run
  detail/events/index access, and projected FAILED diagnostics.
- Extended the browser smoke at both `/` and `/proxy/commandagent/` for a missing
  `identity`, mismatch warning, terminal diagnostics, and session-row diagnostics.
- Updated the read-only guard to pin the new additive/optional API fields.
