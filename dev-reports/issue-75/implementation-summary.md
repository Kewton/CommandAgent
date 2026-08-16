# Issue #75 Implementation Summary

## Implemented

- Run detail now formats each run option as its modification date, normalized
  `status_text`, and unchanged run ID. An in-memory text filter matches ID,
  formatted date, human status, or enum state over the existing bounded
  `RunIndex.runs` window.
- `EmptyState` accepts an optional label. Run detail distinguishes
  `実行未選択` from a filter's `該当なし`, while loading and read errors continue
  to use their existing dedicated states.
- `DocumentViewer` now has an accessible Japanese wrap toggle and an optional
  base-path-aware source link. It starts in the prior wrapped state, reports
  that state with `aria-pressed`, switches explicit wrapped/unwrapped classes,
  and opens the same existing GET used to load the document in a new tab.
- Measurements gives the score/time SVG a touch-oriented explanation and a
  full-size link. At 390px, the image retains a readable minimum width inside
  a horizontally scrollable frame without widening the page itself.
- The GUI smoke harness now supports `--read-only` alongside the integrated
  Overview, feedback, polling, and full-Trial modes. It verifies all Issue #75
  behavior for both root and proxy base paths without dispatching a Trial run.
- A focused source guard pins the filter, contextual labels, source link,
  wrap classes, mobile overflow, and read-only browser assertions.

## Compatibility

- No Rust server, API route, response schema, event, or runtime-state behavior
  changed.
- The generated historical
  `workspace/management/runs/score_time_map.svg` remains byte-unchanged; only
  its GUI presentation changed.
- Current `develop`, including Issues #74, #76, and #77, is integrated. The
  changed behavior remains isolated to GUI presentation, shared state/viewer
  components, CSS, the browser probe, and a source guard.

## Focused coverage

Managed Playwright now checks that:

- an unselected Run detail page has no `NO RECORDS` label;
- all 100 displayed options exactly match their API-derived date, status text,
  and ID;
- the text filter narrows to the requested ID and exposes a contextual
  no-match state;
- the loaded document exposes the existing base-path-aware GET in a new tab;
- the wrap toggle changes and restores the `<pre>` class and `aria-pressed`;
- the Measurements map has inner horizontal overflow at 390px while the page
  still fits the viewport; and
- the full-size SVG link respects both supported base paths.
