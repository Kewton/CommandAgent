# Issue 77 implementation summary

## Implemented

- Added the direct Trial token input to the same desktop and mobile inset and
  width rules as the goal input, keeping both control edges identical.
- Added a mobile `4.5rem` scroll margin to the Gate 1, execution, and terminal
  stage targets so `scrollIntoView` leaves their headings below the sticky
  `3.75rem` top bar.
- Removed invalid ARIA table/row roles from the run ledger, retained each run as
  a native link, hid the visual-only column headings from assistive technology,
  and hid the decorative row arrow.
- Extended `gui/scripts/smoke.mjs` with 1440px and 390px control-edge
  measurements and screenshots, run-ledger role/link checks, and mobile stage
  heading visibility checks against the computed sticky-header geometry.
- Added a focused Rust source-contract test for the inset, scroll-margin, and
  run-ledger accessibility behavior.

## Compatibility

No API, event, persisted runtime state, or `.anvil/` schema changed. Existing
desktop smoke artifact names remain intact; the smoke adds `*-mobile.png`
artifacts and additive JSON measurement fields.

## Smoke evidence

The successful report is at
`/tmp/commandagent-issue-77-smoke.2fHcog/browser-smoke.json`. Both `/` and
`/proxy/commandagent/` cases passed. In both cases:

- desktop 1440px token/goal left and right edge deltas were `0px`;
- mobile 390px token/goal left and right edge deltas were `0px`;
- the run ledger had `0` invalid table/row roles and every run remained a
  native link;
- Gate 2 and terminal headings cleared the 60px sticky top bar with a computed
  72px scroll margin;
- unexpected console errors were empty.

Desktop and mobile screenshot artifacts were visually inspected after the
successful run.
