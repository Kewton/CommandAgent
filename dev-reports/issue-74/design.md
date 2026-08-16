# Issue #74 Design

## Goal

Make the Overview run metric report what an operator can actually see, and
make run badges expose normalized text plus an exact machine state instead of
deriving presentation from arbitrary status substrings.

## Predecessor state

- The branch is integrated with the current `develop`, including Issues #71,
  #73, #77, and #80. Their Trial session index, terminal-card presentation,
  run-ledger semantics, polling, and asset-cache behavior remain intact.
- None of those predecessor changes owned the `/api/runs` total or normalized
  status contract. This change remains localized to the runs API, its two
  consumers, focused GUI tests, and the existing smoke probe.

## API and status contract

1. Wrap the bounded run summaries in a `RunIndex` response containing `runs`
   and `total`. Count directories before applying the existing 100-entry
   response limit, so `total` is not another projection of that limit.
2. Retain `RunSummary.id`, `modified_epoch_seconds`, `report_path`, and the
   existing `status` compatibility field. Add `status_text` for normalized
   human-readable evidence and `state`, serialized as one of `pass`, `fail`,
   `pending`, or `unknown`.
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
- Select badge color from the exact `state` field. Display `status_text` so
  useful evidence such as `FULL 3/3 (date)` is preserved; keep `status` as an
  additive-compatibility alias with the same normalized value.

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
