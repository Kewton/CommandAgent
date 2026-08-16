# Issue #75 Implementation Summary

## Implemented

- Run detail now formats each run option as its modification date followed by
  the unchanged run ID. The selector still uses the existing bounded runs API
  and adds no status, search, or server fields.
- The unselected Run detail surface now uses the neutral document-selection
  prompt, so it no longer presents `NO RECORDS` before the user has selected a
  run.
- `DocumentViewer` now has an accessible wrap toggle. It starts in the prior
  wrapped state, reports that state with `aria-pressed`, and switches the
  document `<pre>` between `document-content--wrapped` and
  `document-content--unwrapped`.
- Measurements gives the score/time SVG a touch-oriented explanation and a
  full-size link. At 390px, the image retains a readable minimum width inside
  a horizontally scrollable frame without widening the page itself.
- The GUI smoke harness now supports `--read-only` and verifies all Issue #75
  behavior for both root and proxy base paths without dispatching a Trial run.
  Its run-payload normalization also allows the same probe to survive Issue
  #74's pending `RunIndex` integration.

## Compatibility

- No Rust server, API route, response schema, event, or runtime-state behavior
  changed.
- The generated historical
  `workspace/management/runs/score_time_map.svg` remains byte-unchanged; only
  its GUI presentation changed.
- Issues #74, #76, and #77 remain unmerged in this worktree. The changed
  behavior is isolated to option rendering, the shared viewer, Measurements
  presentation, CSS, and the browser probe so it can be retained during later
  integration with #74's response envelope and #76's Japanese copy.

## Focused coverage

Managed Playwright now checks that:

- an unselected Run detail page has no `NO RECORDS` label;
- all 100 displayed options exactly match their API-derived date and ID;
- the wrap toggle changes and restores the `<pre>` class and `aria-pressed`;
- the Measurements map has inner horizontal overflow at 390px while the page
  still fits the viewport; and
- the full-size SVG link respects both supported base paths.
