# Issue 118 Implementation Summary

## Implemented

- Reduced `gui/app/try/page.tsx` from the full Trial implementation to a
  wiring-only route that renders `Shell` and `TrialRun`.
- Moved the unchanged Trial markup and presentation helpers into
  `gui/components/trial-run.tsx`.
- Moved Trial state, effects, polling lifecycle, token handling, and actions into
  `gui/hooks/use-trial-run.ts`.
- Added `gui/lib/trial-api.ts` as the single owner of Trial API paths,
  authorization headers, typed JSON requests, session polling, and session file
  reads. The session-index component now reuses this module.
- Added `gui/lib/format.ts` and replaced duplicated byte/date helpers in the
  dashboard, run-detail page, Trial screen, and session index.
- Extended `MonitorFailure` so every failure retains `status` and `code`, using
  `0`/`null` for failures without an HTTP response and explicit metadata for
  invalid monitor responses.
- Renamed and used the shared `isTrialTokenRejected` predicate so ordinary API
  failures and monitor failures clear rejected Trial tokens through the same
  existing path.
- Updated `tests/gui_read_only_guard.rs` to follow the new module ownership and
  added a focused architecture/helper-ownership guard without weakening the
  existing read-only, authentication, polling, copy, or UI contract checks.

## Compatibility Result

- All 53 `data-testid` occurrences in the Trial screen/session index match the
  `origin/develop` baseline exactly.
- All 72 Japanese string/template literals in the Trial route/session index
  match the baseline exactly.
- All 11 Trial `apiPath` call shapes match the baseline after normalizing local
  variable names.
- Feedback smoke output matched the baseline after excluding generated/freshness
  timestamps. Polling retained the same accepted 10-minute virtual-clock
  contract: 50–65 calls, conditional requests after the first response, and at
  least 90% reduction from fixed 750 ms polling.
- No Rust event schema, GUI server route, `.anvil/` state, corpus fixture, or
  historical evidence was changed.
