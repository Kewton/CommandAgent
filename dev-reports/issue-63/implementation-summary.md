# Issue 63 implementation summary

## Implemented

- Added a bounded Trial monitoring policy with a 750 ms healthy cadence,
  exponential retry delay capped at 12 seconds, and a four-attempt stop limit
  for oversized or invalid event streams.
- Kept execution state separate from `connected`, `degraded`, and `lost`
  monitoring state. Gate 2 now shows the last successful update and lights the
  green execution dot only while monitoring is connected.
- Added error-specific guidance for 401/403, manual upstream Access redirects,
  and thrown browser fetch failures.
- Added memory-only reconnect through the existing session-status GET. The
  session ID is restored from `?session=<id>` after reload, while the runtime
  token must be re-entered and is never placed in the URL or browser storage.
- Added a 409 recovery path that extracts an already-running or
  recovery-required session ID and exposes the same GET-only reconnect action.
- Extended the two-base-path Playwright smoke with first-poll failure recovery,
  proxy Access recovery, 401 token guidance, reload reconnect, GET-only call
  auditing, 409 recovery guidance, token persistence checks, and mobile probes.
- Updated the GUI guide and focused protection test. No server session store,
  event schema, CLI delegation path, cancellation control, or intervention
  surface was added.

## Files

- `gui/lib/trial-monitor.ts`: retry and failure classification policy.
- `gui/app/try/page.tsx`: monitor state, polling recovery, and reconnect UI.
- `gui/app/globals.css`: monitor/reconnect presentation and connected-dot rule.
- `gui/scripts/smoke.mjs`: deterministic desktop/mobile recovery coverage.
- `tests/gui_read_only_guard.rs`: focused source/security contract coverage.
- `docs/user/gui.md`: operator behavior and recovery guidance.
