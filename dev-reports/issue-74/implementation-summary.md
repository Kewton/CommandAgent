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
- Added `status_text` and `state` to each run summary. `state` is serialized as
  `pass`, `fail`, `pending`, or `unknown`; status extraction removes Markdown
  emphasis/code markers while retaining useful text such as
  `FULL 3/3 (2026-08-03)`. The existing `status` field remains as a normalized
  compatibility alias.
- Classified known vocabulary by exact normalized words, with failure taking
  precedence over other words. Unrecognized values plus `recorded` and
  `not recorded` remain unknown.
- Updated the Overview and run-detail consumers for the envelope while
  retaining the integrated Japanese navigation, accessibility, base-path,
  shared-error, and Trial-session behavior. The Overview shows
  `recentRuns.length / total`, renders `status_text`, selects badge tone from
  `state`, and removes the three decorative summary cards while preserving the
  formal-band content panel.
- Added focused extraction coverage for all four states, precedence,
  normalization, fallbacks, and substring rejection, plus a GUI server
  integration fixture with 101 synthetic run directories.
- Added an Overview-only managed-Playwright mode that validates API-backed
  counts and marker-free visible badges for root and proxied base paths without
  dispatching a Trial run.

## Contract impact

`/api/runs` now returns `{ runs, total }` rather than a bare array; both
repository consumers were migrated together. Existing `RunSummary` fields are
preserved, while `status_text` and `state` are additive. The existing
report-selection order, 100-entry bound, and run-detail evidence behavior
remain intact. No event schema, recovery/corpus contract, historical evidence,
or `.anvil/` runtime path changed. The current `develop` predecessor set was
integrated without weakening its GUI lifecycle or accessibility contracts.
