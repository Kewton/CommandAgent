# Issue #75 Design

## Goal

Improve the read-only Run detail and Measurements surfaces without changing
the GUI server API or rewriting the generated historical score/time SVG.

## Predecessor state

- Issue #74 (`b7da2106`), Issue #76 (`23c6f2ab`), and Issue #77 (`e99547fa`)
  passed their recorded verification but are not ancestors of this worktree.
- #74 wraps the runs response in `RunIndex`; #76 localizes the GUI and already
  incorporates #77's accessibility fixes. This change will not merge those
  independent commits. It will keep its edits limited to presentation and the
  browser probe so the date, wrap, and mobile-map behavior can be retained
  when those branches are integrated.
- `workspace/management/runs/score_time_map.svg` is generated historical
  evidence and remains unchanged. Mobile readability belongs in its viewer.

## UI behavior

1. Format every non-placeholder run option as its existing modification date
   followed by the run ID. Continue using the bounded list returned by the
   current API; do not add status, search, or API fields.
2. Render the run-selection prompt with the neutral document-empty treatment
   instead of the repository-empty state, so an unselected run is not labeled
   `NO RECORDS`.
3. Add a read-only document wrap toggle. Preserve wrapping as the initial
   behavior, expose its state through `aria-pressed`, and switch explicit
   wrapped/unwrapped classes on the `<pre>` element.
4. At the mobile breakpoint, retain a readable minimum width for score/time
   SVG images inside an overflowable frame. Add a full-size SVG link and
   touch-oriented guidance on Measurements as a second enlargement path.

## Focused verification

- Extend the managed-Playwright smoke probe with a read-only mode. For both
  supported base paths it will assert that the unselected Run detail has no
  `NO RECORDS`, all run options include their API-derived dates, the wrap
  button changes the `<pre>` class, and the Measurements SVG frame overflows
  horizontally at a 390px viewport.
- Run GUI lint, typecheck, build, script syntax, the focused read-only browser
  probe, and the repository-required formatting, Clippy, and Rust test suite.
