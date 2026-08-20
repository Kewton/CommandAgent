# Issue #164 Implementation Summary

## Outcome

The measurement page now keeps the selected report when the report index is
revalidated after a tab visibility change. The root and proxy-base-path browser
smoke cases both confirmed that a non-first selection remains active after the
tab transitions from hidden to visible.

## Changes

- `gui/app/measurements/page.tsx` derives the selected report path and skips
  first-report initialization while that path remains in the refreshed index.
  If the selected report disappears, the existing first-report fallback still
  runs.
- `gui/scripts/smoke.mjs` selects the second measurement report, dispatches a
  hidden-to-visible transition, waits for the report-index response, and records
  whether the active path was retained.
- `tests/gui_read_only_guard.rs` pins both the selection-preservation condition
  and the visibility smoke contract.

## Compatibility

The shared `useResource` focus and visibility revalidation behavior is
unchanged. No API, event, persistence, runtime-state, or document schema changed.
No corpus fixture was required because this is a GUI-local selection-state
regression covered by the production browser smoke.
