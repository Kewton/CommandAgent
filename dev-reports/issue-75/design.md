# Issue #75 Design

## Goal

Improve the read-only Run detail and Measurements surfaces without changing
the GUI server API or rewriting the generated historical score/time SVG. Cover
the complete Issue behavior: contextual empty states, date/status selection,
text filtering, document actions, and mobile map access.

## Predecessor state

- The branch is integrated with current `develop` (`5797a3cc`), including
  Issues #74, #76, and #77. The implementation consumes #74's `RunIndex` and
  `status_text`, retains the Japanese GUI copy, and preserves the run-ledger
  and Trial accessibility/lifecycle contracts.
- `workspace/management/runs/score_time_map.svg` is generated historical
  evidence and remains unchanged. Mobile readability belongs in its viewer.

## UI behavior

1. Format every non-placeholder run option as its modification date,
   normalized status text, and run ID. Filter the bounded `RunIndex.runs`
   window in memory by ID, formatted date, human status, or enum state; do not
   add an API field or route.
2. Extend `EmptyState` with an optional label. Distinguish the unselected run
   (`実行未選択`) and an empty filter result (`該当なし`) from a genuinely empty
   repository, while retaining the shared loading and coded-error states.
3. Add a read-only document wrap toggle. Preserve wrapping as the initial
   behavior, expose its state through `aria-pressed`, and switch explicit
   wrapped/unwrapped classes on the `<pre>` element.
4. Give loaded documents an optional source link that opens the same existing
   GET endpoint in a new tab. Run acceptance/evidence and measurement reports
   construct only their already-used, base-path-aware GET URLs.
5. At the mobile breakpoint, retain a readable minimum width for score/time
   SVG images inside an overflowable frame. Add a full-size SVG link and
   touch-oriented guidance on Measurements as a second enlargement path.

## Focused verification

- Extend the managed-Playwright smoke probe with a read-only mode. For both
  supported base paths it will assert contextual empty labels, exact
  API-derived date/status options, ID filtering, the source GET link, the wrap
  button class/ARIA transition, and horizontal SVG overflow at 390px.
- Add a focused source guard for these read-only contracts while preserving
  all previously integrated smoke modes.
- Run GUI lint, typecheck, build, script syntax, the focused read-only browser
  probe, and the repository-required formatting, Clippy, and Rust test suite.
