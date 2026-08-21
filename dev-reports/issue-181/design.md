# Issue 181 design

## Observed defects

- The mobile top bar reduces badge padding and type size, but neither the
  runtime summary nor its badges prohibit wrapping. During an active session,
  the two status labels can therefore wrap internally or compress the brand at
  the 390px iPhone 13 viewport.
- The getting-started close button is a shrinking flex item with no no-wrap
  contract, so its visible `閉じる` label can split when the adjacent Japanese
  heading and description consume the row.
- Trial stage targets receive a `4.5rem` sticky-header scroll margin only below
  720px. The session history and row anchors use `1rem` at every width, which is
  smaller than the sticky top bar on both desktop and mobile.

## Change

- Keep the mobile brand and runtime summary as bounded, single-line flex
  groups. Prevent status-label wrapping and use slightly tighter mobile brand
  geometry so both active badges fit beside the recognizable CA/CommandAgent
  brand at 390px.
- Make the getting-started close control a non-shrinking, single-line flex item.
  Keep its existing text, accessible name, dismissal behavior, and storage key.
- Apply a shared `4.5rem` scroll margin to Trial stage, session-history, and
  session-row targets at the base breakpoint so it clears the 4rem desktop top
  bar and the 3.75rem mobile top bar. Remove the now-redundant mobile-only stage
  rule.

## Scope and compatibility

The implementation is limited to the Lane H stylesheet. The existing
`gui/components/getting-started.tsx` structure already supports the close-label
fix and requires no behavior or markup change. Trial components owned by Lane G
remain untouched. There are no API, event, route, runtime-state, or persisted
storage contract changes.

## Verification

Run the GUI internal-path lint, TypeScript check, production build, the focused
GUI source-contract test, and the GUI smoke at its 390px mobile viewport. Inspect
the active-session mobile capture for a one-line top bar and the getting-started
capture for an intact close label. Finish with repository formatting, Clippy,
full Rust tests, and `git diff --check` because the shared GUI stylesheet is
release-facing.
