# Issue #14 Design

## Scope and predecessor integration

Keep queued input in a new `src/tui/input_queue.rs` leaf module and limit the
existing interrupt monitor, footer, and REPL changes to ownership and dispatch
wiring. Issue #13's committed resize-aware footer geometry is the base for the
footer changes because queued input adds a reserved row whose geometry must
remain correct across resize. Issue #15 changes the same REPL prompt block and
will also be integrated before implementation. Issue #11 is confined to the
Markdown renderer, so its implementation was inspected but does not need to be
copied into this branch.

## Key-event ownership

- Keep the execution-time interrupt monitor as the sole reader of crossterm
  events. It will route printable characters, Backspace, Enter, and Esc into a
  shared in-memory input queue instead of adding a second terminal reader.
- Ctrl+C always follows the existing first-interrupt/force-finalize path. Esc
  clears a non-empty pending buffer without interrupting; when that buffer is
  empty, Esc follows the same existing interrupt path. Other control and
  navigation keys remain ignored.
- The queue is enabled only when both the interrupt monitor and fixed footer
  actually started. `ANVIL_NO_INTERRUPT`, non-TTY operation, a disabled footer,
  terminal setup failure, or an unusable terminal therefore preserves the old
  no-queue behavior. Prompt-time input remains owned by rustyline while the
  monitor is parked by the existing pause guard.

## Display and limits

- Reserve one input row immediately above the existing live-footer lines while
  queued input is enabled. The footer render thread reads an immutable queue
  snapshot and composes the pending echo, queue count, confirmation/rejection
  notice, and context-sensitive Esc/Ctrl+C hint into its normal saved-cursor
  frame. This keeps input echo inside the existing footer rendering and freeze
  guard rather than writing asynchronously from the key monitor.
- Extend Issue #13's geometry transitions so enabling the input row and later
  terminal resizes atomically clear and re-establish the DECSTBM reservation.
  Pending text is width-fitted with the footer's multibyte-safe helper.
- Bound the queue to 10 submitted lines and each pending line to 4096 UTF-8
  bytes. Overflow keeps the current pending text and shows an explicit footer
  rejection. Queue confirmations use a sanitized 40-character preview.

## REPL dispatch and verification

- Before opening the next rustyline prompt, take the oldest queued line, print
  a `processing queued:` preview, then send it through the same normalization,
  exit-command, history, command-dispatch, rendering, and interrupt-reset path
  as manually entered input. Lines added while a queued command runs remain in
  FIFO order. If execution ends while the pending buffer is non-empty but not
  submitted, park the monitor first and seed that text into the next rustyline
  prompt so a partially typed instruction is not stranded or split.
- Add focused state-machine tests for editing, UTF-8 byte limits, queue limits,
  FIFO order, previews, Esc versus Ctrl+C, disabled behavior, and footer
  composition/geometry. Update `/help` coverage and the manual TUI UAT scenario.
  No event, recovery, or corpus contract changes, so no corpus fixture update is
  required.
