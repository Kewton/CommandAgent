# Issue #14 Implementation Summary

## Implemented

- Added `src/tui/input_queue.rs`, a process-local pending buffer and FIFO queue
  capped at 10 submitted lines and 4096 UTF-8 bytes per line. It provides
  multibyte-safe Backspace editing, 40-character previews, explicit limit
  rejection notices, and terminal-control filtering.
- Kept crossterm event ownership in the existing interrupt monitor. Printable
  keys, Backspace, and Enter now edit/submit pending input during REPL command
  execution. Ctrl+C always uses the existing interrupt escalation path; Esc
  clears non-empty pending input and interrupts only when pending input is empty.
- Enabled queueing only when the REPL's interrupt monitor and fixed footer both
  start successfully. `ANVIL_NO_INTERRUPT`, non-TTY operation, footer-off mode,
  terminal setup failure, and direct one-shot actions retain the previous
  interrupt-only or no-op behavior without retaining queued input.
- Extended Issue #13's resize-aware footer geometry with one reserved input row.
  Pending echo, queue count, `queued:`/rejection feedback, and context-sensitive
  key hints render through the footer's saved-cursor frame and freeze guard.
- Drained queued lines in FIFO order before the next rustyline prompt. Each line
  emits a `processing queued:` preview and follows the same normalization,
  history, exit-command, dispatch, rendering, and interrupt-reset path as normal
  REPL input. An unsubmitted partial buffer is seeded into the next prompt after
  the monitor is parked, preventing text from being stranded at command end.
- Updated `/help` and the TUI UAT guide with queue limits, editing behavior,
  disabled modes, and the distinct Esc/Ctrl+C semantics.

## Tests

- Added focused queue state-machine, key routing, footer composition, geometry,
  disabled-mode, UTF-8 limit, FIFO, preview, and help tests.
- Added a gated PTY acceptance test backed by a delayed local fake Ollama. It
  verifies footer confirmations, multiple queued lines, FIFO replay, processing
  notices, normal history persistence, and clean exit through the real binary.
- No event, recovery, or corpus contract changed, so no corpus fixture update
  was required.

## Predecessors

- Integrated Issue #13 before implementation so queued-row transitions build on
  the committed resize-safe footer behavior.
- Integrated Issue #15 before editing the overlapping REPL prompt block.
- Inspected Issue #11's committed Markdown renderer changes; they are orthogonal
  to the queue and were not copied into this branch.
