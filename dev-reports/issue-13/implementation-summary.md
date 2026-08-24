# Issue #13 Implementation Summary

## Result

The fixed footer now follows terminal width and height changes throughout a
run. Each render-loop tick samples the terminal size, retains the previous
geometry if the query fails, and forces a geometry transition plus a freshly
fitted redraw when the size changes.

## Changes

- Added one `FooterGeometry` value for width, height, and the width-dependent
  one-line/two-line reservation.
- Added an ordered resize sequence that resets DECSTBM, clears both plausible
  old-footer locations and the new reservation without emitting scrolling
  newlines, establishes the new scroll region, clamps the body cursor above
  the footer, and draws the re-fitted frame.
- Kept resize handling active while normal footer updates are frozen for an
  interactive prompt.
- Returned the render thread's latest geometry through its join handle and
  rechecked terminal size during `Drop`, so normal, interrupted, and panic
  unwinding cleanup removes the current footer even if shutdown races a resize.
- Reset and suppress the footer at temporarily unusable tiny heights, allowing
  the next usable geometry to establish it again.
- Preserved the existing no-footer, non-TTY, startup-size-failure, status bus,
  and event-schema behavior.

## Tests

Focused footer tests cover terminal-size query failure, height growth and
shrink cleanup, body-cursor clamping, both directions across the 100-column
threshold, final resize cleanup, and temporarily unusable heights. Existing
disabled/non-TTY and panic-unwind cleanup tests continue to pass.
