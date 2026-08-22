# Issue #185 Implementation Summary

## Implemented

- Added a controlled Measurements report search that filters the complete
  in-memory index by report ID or path, case-insensitively. The index reports
  `visible / total`, shows an explicit `該当なし` state, and restores all 256
  rows when cleared.
- Kept document selection independent from the filter. A selected report stays
  loaded when its row is hidden and becomes active again when the full list is
  restored; the existing visibility-revalidation selection behavior is also
  unchanged.
- Reused the existing picker form presentation without adding filter-specific
  global CSS. The only `globals.css` changes are the two existing mobile
  measurement map-frame rules.
- Replaced the mobile map's forced `70rem` minimum image width and scrollable
  frame with a frame-width SVG fit and hidden frame overflow. The original,
  base-path-aware SVG link remains available for detailed zooming.
- Updated the map explanation and accessible region label so they no longer
  advertise horizontal scrolling.

## Focused checks

- Added `measurement_filter_and_mobile_map_fit_are_pinned` to lock the report
  filtering, no-match, map-copy, and mobile CSS contracts.
- Updated the existing Measurements/Run read-only guard for the new no-axis
  overflow smoke assertion.
- Extended the managed Playwright read-only smoke to exercise exact-path
  filtering, zero matches, count restoration, filter-independent selection,
  390px page fit, both map overflow axes, rendered image fit, and computed
  overflow styles.

## Browser evidence

The passing evidence is in
`/private/tmp/commandagent-issue185-smoke.7aAqYV/`. Its
`browser-smoke.json` reports top-level `ok: true` for both `/` and
`/proxy/commandagent/`, with no unexpected console errors.

For both base paths, the browser observed:

- 256 initial report rows, `1 / 256` for the selected full-path query,
  `0 / 256` plus visible `該当なし` for a missing query, and restored
  `256 / 256` after clearing;
- the same selected report before and after filtering;
- a 390px viewport with a 322×207 map frame and image;
- no horizontal or vertical frame overflow, `overflow-x/y: hidden`, and no
  document-level horizontal overflow;
- a working original-SVG link under both base paths.

The root mobile Measurements screenshot was inspected at original resolution.
It shows the complete map inside one frame, the search control and count above
the report list, and no clipped or two-axis-scrolling map content.

## Compatibility and scope

No API, event, report, route, persisted-state, or runtime schema changed. The
production diff is confined to `gui/app/measurements/page.tsx` and the approved
measurement map-frame selectors in `gui/app/globals.css`; supporting changes
are limited to the focused GUI guard, read-only smoke, and Issue #185 reports.
