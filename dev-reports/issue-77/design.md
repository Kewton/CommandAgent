# Issue 77 design

## Observed defects

- The Trial token is a direct child of `.trial-compose`, but the desktop and
  mobile inset selectors include the direct label and textarea, not the direct
  input. Its `width: 100%` therefore starts at the panel edge and extends past
  the intended right inset.
- Mobile stage changes call `scrollIntoView({ block: "start" })` for the Gate 1,
  execution, and terminal sections. Those targets have no scroll margin, so the
  sticky 3.75rem top bar can cover the section heading.
- The run ledger is a collection of navigational links, but it declares ARIA
  `table` and `row` roles without the required header/cell role structure. The
  row role also replaces each anchor's native link role.

## Change

- Apply the same horizontal inset and calculated width used by the goal field
  to the direct Trial token input at desktop and mobile widths.
- Give every programmatic stage-scroll target a mobile-only top scroll margin
  slightly larger than the sticky top bar.
- Remove the false table/row roles and hide the visual column-heading strip
  from assistive technology. Keep each run as a native, keyboard-operable link
  whose accessible text includes its identity, state, and modified time.
- Extend the GUI smoke script to capture Gate 1 at 1440px and 390px, assert the
  token and goal controls share left/right edges at both sizes, assert no stale
  table/row roles remain, and verify mobile Gate 2 and terminal scrolling lands
  below the sticky top bar.

## Compatibility and verification

No API, event, runtime-state, or filesystem contract changes. Focused source
coverage will pin the CSS and ARIA contracts. Verification will run the GUI
lint, typecheck/build, focused Rust GUI guard, and the end-to-end GUI smoke;
repository formatting, Clippy, and full Rust tests will follow because the
checked-in GUI smoke contract is shared release evidence.
