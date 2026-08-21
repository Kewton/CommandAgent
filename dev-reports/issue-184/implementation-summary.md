# Issue #184 Implementation Summary

## Outcome

The repository-run page now makes the bounded index explicit and still lets an
operator open any known run ID:

- it renders `表示件数 100 / 総数 196` in the current repository instead of
  presenting the first 100 entries as the complete ledger;
- the existing search field continues to filter those indexed summaries by ID,
  date, status text, and state;
- submitting a complete ID opens `GET /api/runs/{id}` directly, including a run
  omitted from the bounded index;
- a directly selected run remains represented in the controlled select even
  when it is absent from the local option set; and
- the selected ID is repeated in an `output` that wraps anywhere, so a native
  mobile select cannot make the current identity unreadable.

The measurements report hierarchy is now `h1 → h2 → h2 → h3`. `DocumentViewer`
accepts a narrowly typed `headingLevel` prop with a backward-compatible `h2`
default. Measurements promotes the report-list label to `h2` and requests `h3`
for the selected report; Runs and Trial retain `h2` document headings.

## Files changed

- `gui/app/runs/page.tsx`: displayed/total count, exact-ID form submission,
  direct-selection option, and wrapping selected-ID output.
- `gui/components/document-viewer.tsx`: caller-selectable `h2`/`h3` document
  heading with unchanged default behavior.
- `gui/app/measurements/page.tsx`: heading-only report-list and viewer-level
  changes.
- `gui/scripts/smoke.mjs`: real omitted-run lookup, count, mobile fit, and
  heading-order browser assertions for root and proxy base paths.
- `tests/gui_read_only_guard.rs`: focused source contracts for the new behavior.
- `dev-reports/issue-184/`: design, implementation, and verification records.

## Evidence

The final managed Playwright report at
`/private/tmp/commandagent-issue184-smoke-20260822-final/browser-smoke.json`
records `ok: true` for both base paths. Each case displayed 100 of 196 runs,
opened `20260719-015733-orchestrate` (absent from the returned 100 summaries),
kept the selected ID visible at 390 px, and found no page-width overflow.

A separate axe-core 4.13.0 scan ran the `heading-order` rule on the rendered
Measurements and Runs pages. Measurements reported levels `1, 2, 2, 3`, Runs
reported `1, 2`, and both returned zero violations.

## Scope audit

No backend route, response schema, event contract, historical run evidence,
`.anvil/` runtime state, corpus fixture, or runner growth-tripwire file changed.
The implementation reuses the existing honest `RunIndex.total` and exact-ID
detail endpoint rather than widening or weakening their contracts.
