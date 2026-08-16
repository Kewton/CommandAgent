# Issue 78 implementation summary

## Outcome

Trial now presents a compact **依頼 → 確認 → 実行 → 結果** stepper and one
active workflow state. The request form, Gate 1 contract/price confirmation,
Gate 2 progress, Terminal/D-3d controls, and closed acknowledgment no longer
accumulate into one long page.

At the 390 × 844 verification viewport, the request and Gate 1 confirmation
actions are fixed above the existing bottom navigation. Gate 2 begins with its
live progress heading. Terminal places the D-3d action card before the long
verdict in mobile layout while retaining verdict-left/action-right placement on
desktop.

## Changes

- `gui/app/try/page.tsx`
  - added the Japanese four-step progress indicator;
  - made compose, Gate 1, Gate 2, Terminal, and closed rendering mutually
    exclusive under one active-state wrapper;
  - separated the small Gate 1 confirmation action area from the price details;
  - retained the existing mobile stage refs and launch-identity lock;
  - kept every pre-existing `data-testid` and added only `trial-stage-nav` and
    `trial-active-stage` for layout verification.
- `gui/app/globals.css`
  - added desktop stage/rail grid areas and state-specific Gate 1/Terminal grids;
  - added the 390px four-column stepper, fixed request/confirmation action bar,
    and action-first Terminal layout;
  - reserved content space so fixed controls do not cover the active state.
- `gui/scripts/smoke.mjs`
  - measures all four interactive states at 390 × 844 for Japanese step labels,
    one visible workflow state, and a primary action or progress heading above
    the bottom navigation;
  - records those measurements in the existing two-base-path smoke report.
- `tests/gui_read_only_guard.rs`
  - pins mutually exclusive state rendering, Japanese-only new labels, mobile
    action positioning, Terminal ordering, and the Playwright measurements.
- `docs/user/gui.md`
  - documents the one-state flow and mobile action placement.

## Compatibility

No Rust production source changed. Trial API paths, methods, request/response
schemas, event names and bytes, `PolledSession`, Gate 1 confirmation semantics,
`proposal.card_hash`, the dedicated authorization header, CLI-only delegation,
D-3d hashing, corpus contracts, historical evidence, and `.anvil/` runtime
state are unchanged.

The predecessor branches were inspected but were not merged, cherry-picked, or
copied. Their polling/reconnect, lease, options, phase feedback, artifacts,
session index, coded errors, Japanese copy, accessibility, token decision, and
304 behavior remain state-local integration inputs for the later
dependency-order merge.
