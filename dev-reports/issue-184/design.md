# Issue #184 Design

## Goal

Make the bounded repository-run index honest and usable when the repository
contains more than 100 runs. Operators must see both the number of indexed
runs and the repository total, be able to open an older run by its complete
ID, and be able to read the selected ID on a mobile viewport. The combined
Lane H decision also requires a valid heading sequence on the measurements
page.

## Existing contracts

- `GET /api/runs` already returns `{ runs, total }`, where `runs` is bounded to
  100 summaries and `total` is counted before that bound.
- `GET /api/runs/{id}` already accepts any valid run directory ID, including an
  ID omitted from the bounded index. The approved file scope therefore does
  not require an API or schema change.
- The runs page already filters the bounded summaries by ID, date, status, and
  state. That remains useful for the visible index but cannot discover an
  omitted entry by itself.
- `DocumentViewer` currently fixes its document heading at `h2`, which prevents
  a caller from expressing a nested document heading.

## UI behavior

1. Render an explicit `displayed / total` count from `runs.length` and `total`.
   The displayed number describes the bounded index, not the current filter,
   so the repository limit remains visible while the operator searches.
2. Keep the existing local filter and make its form submit the trimmed input
   as an exact run ID. Submitting calls the existing run-detail route, allowing
   a complete ID outside the first 100 entries to be opened without pretending
   that the bounded index is exhaustive.
3. When the selected run is outside the currently filtered options, preserve
   it as a direct-ID option so the controlled select continues to expose its
   value.
4. Render the selected ID separately in a live `output` with anywhere wrapping.
   This avoids relying on the native mobile select's truncated display.
5. Add a `headingLevel` (`2 | 3`) prop to `DocumentViewer`, defaulting to `2`
   for existing callers. On measurements, promote `レポート一覧` to `h2` and
   render the selected report document as `h3`, producing a sequential
   `h1 → h2 → h3` hierarchy. Other viewer callers retain their current level.

## Focused tests and verification

- Extend the read-only source contract test to pin the count, direct-ID form,
  wrapping selected-ID output, and explicit measurement viewer heading level.
- Extend the managed Playwright read-only smoke to assert the displayed/total
  count, open a repository run omitted from the first 100, verify the direct ID
  remains visible at mobile width, and inspect the rendered heading levels.
- Run the focused Rust contract test and read-only browser smoke first, then GUI
  lint, typecheck, and build. Run repository formatting, Clippy, and the full
  Rust test suite because the task requires the standard production handoff
  checks even though no Rust production contract changes.
