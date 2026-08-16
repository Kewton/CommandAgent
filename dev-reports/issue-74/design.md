# Issue #74 Design

## Goal

Make the Overview run metric report what an operator can actually see, and
make run badges expose normalized text plus an exact machine state instead of
deriving presentation from arbitrary status substrings.

## Predecessor state

- Issue #71 (`c312eb75`), Issue #77 (`e99547fa`), and Issue #80 (`b84034b6`)
  passed their recorded verification but are not ancestors of this worktree.
- #71 adds the separate Trial session index, #77 corrects the Overview run
  ledger's link/table semantics, and #80 adds conditional Trial polling and
  static-asset caching. None changes the `/api/runs` payload or status
  extraction behavior.
- This change will not merge those independent commits. Its edits remain
  localized to the runs API, the two runs consumers, focused GUI tests, and
  the existing smoke probe so the predecessor changes can be integrated
  without behavioral coupling.

## API and status contract

1. Wrap the bounded run summaries in a `RunIndex` response containing `runs`
   and `total`. Count directories before applying the existing 100-entry
   response limit, so `total` is not another projection of that limit.
2. Retain `RunSummary.id`, `modified_epoch_seconds`, `report_path`, and
   `status`. Add `state`, serialized as one of `pass`, `fail`, `pending`, or
   `unknown`.
3. Have `extract_status` return both normalized text and the enum state.
   Remove Markdown emphasis/code marker characters (`*` and backticks) from
   extracted text. Classify known success, failure, and pending vocabulary by
   exact normalized words; unrecognized, `recorded`, and `not recorded`
   values remain `unknown`.

## Overview behavior

- Consume the `RunIndex` envelope on both the Overview and run-detail pages.
- Render `recentRuns.length / total` as `Runs shown / total`, matching the
  eight rows actually rendered on the Overview rather than the API's
  100-entry ceiling.
- Remove the `Recent positive`, `Formal bands`, and static `Execution surface`
  cards identified by the Issue as decorative. Keep the formal-band content
  panel itself unchanged.
- Select badge color from the exact `state` field. Continue displaying the
  normalized `status` text so useful evidence such as `FULL 3/3 (date)` is
  preserved.

## Tests and verification

- Add the required `extract_status` unit test for the Markdown-wrapped FULL
  value, plus focused fallback/classification coverage.
- Add a GUI server integration test with 101 synthetic run directories to pin
  the pre-limit total, 100-entry window, additive summary fields, normalized
  text, and serialized state.
- Extend the managed-Playwright smoke with an Overview-only mode that checks
  the rendered count against the API and asserts every visible badge is free
  of `**` and backticks. This mode avoids dispatching a Trial run.
- Run the focused Rust and Overview smoke checks first, then GUI lint,
  typecheck/build, formatting, Clippy (default and GUI features), and the full
  Rust suite because the shared GUI API contract changes.
