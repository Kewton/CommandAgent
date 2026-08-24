# Issue #185 Design

## Goal and scope

Make the 256-entry measurements report index practical to browse and make the
score/time SVG fit a 390px mobile viewport without horizontal and vertical map
scrolling. Production changes are limited to
`gui/app/measurements/page.tsx` and the existing measurement map-frame rules in
`gui/app/globals.css`, as required by the approved decision.

There are no required predecessors. The two commits currently present on
`origin/develop` but not this worktree affect only CommandMate orchestration
code and tests, so they do not change the inspected GUI contracts.

## Existing contracts

- The reports endpoint already returns the complete `DocumentSummary[]`; no API
  or schema change is needed.
- The page defaults to the first report and retains the selected path when the
  shared report resource revalidates. Filtering must not replace that selection
  or trigger another document request.
- The map exposes a base-path-aware original-SVG link. That remains available
  for detailed inspection after the embedded map is scaled to the viewport.
- The mobile stylesheet currently forces the map image to a `70rem` minimum
  width and makes its frame scrollable, which creates the reported overflow.

## Design

1. Add a controlled search input to the report index and derive a memoized,
   case-insensitive subset by matching both report ID and path. Show the visible
   count against the total and an explicit no-match state. Clearing the search
   restores the complete list; the selected document remains unchanged even
   when its row is temporarily filtered out.
2. Reuse the existing run-picker form-control presentation rather than adding
   unrelated global styles. No stylesheet selector outside the measurement
   map-frame rules will change.
3. Update the map copy and accessible label to describe viewport fitting rather
   than horizontal scrolling. At the mobile breakpoint, hide frame overflow and
   remove the forced image minimum width so the SVG renders at the frame width
   with automatic height.
4. Extend the focused GUI source-contract test and managed read-only Playwright
   smoke. The browser check will prove path filtering, no-match feedback, count
   restoration, selection retention, page-width fitting, and absence of either
   map-frame overflow axis at 390px.

## Verification

Run the focused GUI read-only contract test first, then JavaScript syntax,
TypeScript, lint, and production build checks. Build the GUI-enabled binary and
run the read-only browser smoke for both root and proxied base paths, recording
the rendered map geometry and screenshots. Finish with repository formatting,
Clippy, the full Rust suite, and `git diff --check` because the shared global
stylesheet and smoke contract are CI-facing.
