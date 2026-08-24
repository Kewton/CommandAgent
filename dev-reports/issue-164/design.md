# Issue #164 Design

## Scope

Keep the measurement report selected when the shared report index revalidates
after the browser tab becomes visible again. Preserve the existing focus and
visibility revalidation behavior in `useResource`; this issue changes only the
measurement page's default-selection rule and its browser smoke coverage.

## Design

- Derive the current report path from the loaded `DocumentRecord`. When the
  report index changes, retain that selection if the path is still present.
- Load the first report only when there is no selection or the selected report
  disappeared from the refreshed index. This preserves the existing initial
  default and provides an honest fallback for a deleted report.
- Extend the read-only browser smoke to select a non-first report, dispatch a
  hidden-to-visible transition, wait for the `/api/reports` revalidation, and
  assert that the same report button remains active.
- Pin the new source and smoke contracts in `tests/gui_read_only_guard.rs`.

## Verification

Run the focused GUI read-only guard, GUI lint and typecheck, then build the
release binary and run the read-only browser smoke against it. Because the
production change is isolated to the Next.js page and does not alter Rust or a
shared schema, the repository-wide Rust suite is not required beyond the
focused guard and the Rust build exercised by the smoke.
