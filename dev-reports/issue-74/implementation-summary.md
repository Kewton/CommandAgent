# Issue #74 Implementation Summary

## Outcome

The Overview now reports the eight run rows an operator can see against the
complete run-directory total. Run badges display normalized evidence text and
take their presentation tone from an explicit machine state instead of status
substrings.

## Changes

- Replaced the bare `/api/runs` array with a `RunIndex` envelope containing the
  bounded `runs` window and a `total` counted before the existing 100-entry
  limit.
- Added `state` to each run summary, serialized as `pass`, `fail`, `pending`,
  or `unknown`. Status extraction removes Markdown emphasis/code markers while
  retaining useful text such as `FULL 3/3 (2026-08-03)`.
- Classified known vocabulary by exact normalized words, with failure taking
  precedence over other words. Unrecognized values plus `recorded` and
  `not recorded` remain unknown.
- Updated the Overview and run-detail consumers for the envelope. The Overview
  shows `recentRuns.length / total`, selects badge tone from `state`, and
  removes the three decorative summary cards while preserving the formal-band
  content panel.
- Added focused extraction coverage for all four states, precedence,
  normalization, fallbacks, and substring rejection, plus a GUI server
  integration fixture with 101 synthetic run directories.
- Added an Overview-only managed-Playwright mode that validates API-backed
  counts and marker-free visible badges for root and proxied base paths without
  dispatching a Trial run.

## Contract impact

`/api/runs` now returns `{ runs, total }` rather than a bare array; both
repository consumers were migrated together. Existing `RunSummary` fields are
preserved and `state` is additive. The existing report-selection order,
100-entry bound, and run-detail evidence behavior remain intact. No event
schema, recovery/corpus contract, historical evidence, or `.anvil/` runtime
path changed. Independent predecessor commits for Issues #71 (`c312eb75`),
#77 (`e99547fa`), and #80 (`b84034b6`) were inspected but not merged.
