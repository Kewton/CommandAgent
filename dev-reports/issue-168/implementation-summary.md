# Issue #168 implementation summary

## Implemented

- Keyed repository run details and evidence documents to the run ID that
  requested them. A document is renderable only while its owner matches the
  current selection.
- Added request-version guards so superseded detail/evidence responses cannot
  publish data, errors, or loading completion into the current selection.
- Abort outstanding evidence reads on run changes, newer evidence choices, and
  return to the acceptance sheet.
- Clear run detail, evidence selection, errors, and loading state immediately
  when the selection changes. Returning to `実行を選択…` now removes both the
  evidence list and document viewer.
- Preserved existing API routes and schemas, source links, history URL updates,
  filtering, and read-only server behavior.

## Tests

- Extended the Rust GUI source guard to pin run-owned state, cancellation, and
  clearing behavior.
- Extended the read-only Playwright smoke with controlled, delayed detail
  responses. The probe verifies superseded-request isolation, newest-run
  rendering, loading ownership, and the completely empty unselected state for
  both root and proxy base paths.

## CI follow-up

- Cherry-picked only Issue #160 code commit `714017ca` as `45daeddc`; its
  report commit `1f28c021` and Issue #160 report files were not applied.
- The compatibility change boxes the session-file handler error response and
  unwraps that same `Response` in `IntoResponse`. It adds no lint allowance and
  preserves the existing response status, headers, JSON bytes, path
  confinement, and symlink rejection behavior.
- Re-ran the Rust 1.97.1 GUI Clippy gate, focused session-file contract tests,
  and Issue #168 source/type/browser regressions after the cherry-pick.
