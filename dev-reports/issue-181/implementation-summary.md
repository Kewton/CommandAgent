# Issue 181 implementation summary

## Implemented

- Kept the top-bar brand, runtime summary, and individual runtime badges as
  bounded single-line flex items. Added an explicit top-bar gap and tightened
  the existing mobile brand mark, brand gap, and brand type geometry so the two
  active runtime badges fit beside CommandAgent at 390px.
- Made the getting-started close control non-shrinking and `nowrap`, preserving
  the existing visible `閉じる` text, accessible name, click behavior, and
  session-storage dismissal contract. The existing component markup required
  no change.
- Moved the `4.5rem` sticky-header scroll margin to the base styles and applied
  it consistently to Gate 1, execution, terminal, session-index, and session-row
  targets. It now clears the sticky top bar on desktop and mobile instead of
  applying only to mobile stages while leaving history anchors at `1rem`.
- Extended the GUI smoke with browser-rendered line-count and geometry checks.
  It now rejects a 390px running top bar that grows beyond 60px, wraps either
  runtime label, overlaps the brand, or leaves the viewport. It also captures a
  mobile getting-started screenshot and rejects a wrapped `閉じる` label.
- Updated the focused Rust GUI source-contract test to pin the shared scroll
  margin, no-wrap flex rules, mobile header gap, and smoke measurements.

## Compatibility and ownership

No API, event, route, runtime state, or persisted storage schema changed.
`gui/components/getting-started.tsx` remains behaviorally and byte unchanged
because its existing button structure already supports the stylesheet fix.
No Trial component owned by Lane G was edited.

## Browser evidence

The passing report is
`/private/tmp/commandagent-issue-181-smoke.j9Fa3M/browser-smoke.json`.
Both `/` and `/proxy/commandagent/` cases reported overall `ok: true` and:

- a 390px viewport and 60px running top bar;
- one rendered line for `Trial 利用可` and `実行中 <id>`;
- a 15.796875px gap between the brand and runtime summary;
- one rendered `閉じる` line with computed `white-space: nowrap`;
- a 72px stage scroll margin clearing the 60px sticky header.

The root and proxy `*-gate-2-mobile.png` and
`*-getting-started-mobile.png` artifacts were visually inspected. Both running
headers stay on one row without crowding the brand, and both close labels stay
intact at 390px.
