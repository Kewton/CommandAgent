# Issue #13 Design

## Scope

Keep the change inside `src/tui/footer.rs`. Issue #11 only changes terminal
Markdown rendering, while Issue #15 changes branding in `banner.rs` and
`tests/tui_pty.rs`; neither predecessor needs to be copied into this branch for
the footer implementation.

## Design

- Represent the active width, height, and derived footer-row count as one
  geometry value owned by the render thread. The render loop will call
  `terminal::size()` once per tick and retain the current value on errors.
- When geometry changes, emit one ordered transition: reset the old DECSTBM
  region, erase the old footer coordinates that remain visible plus the new
  bottom reservation, establish the new DECSTBM region, place the body cursor
  inside its new lower bound, and draw a freshly width-fitted footer frame.
- Return the render thread's last valid geometry when it joins so the existing
  RAII `Drop` path restores and clears the current reservation after normal,
  interrupted, or unwinding shutdown. Temporarily unusable tiny heights reset
  the reservation and suppress footer drawing until a usable size returns.
- Preserve startup detection and the disabled/non-TTY early returns.

## Verification

Add focused unit coverage for grow/shrink cleanup, the 100-column one-line to
two-line transition, body-cursor clamping, size-query failure fallback, and
cleanup using resized geometry. Then run the footer tests followed by format,
Clippy, and the full Rust test suite.
