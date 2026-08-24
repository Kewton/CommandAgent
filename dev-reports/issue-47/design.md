# Issue 47 design

## Observed mechanism

Ultra phase progress already reaches the process-wide TUI projection through
`eval_events::emit` and `tui::status_bus::publish_eval_projection`. REPL
commands also emit paired `tui_command_start` / `tui_command_stop` lifecycle
events, while direct CLI commands use the same stop event through their
completion guard. The fixed footer owns a separate status subscriber, so
attaching terminal-title or bell output to `Footer` would incorrectly disable
the new behavior under `--footer off`.

The completed Issue 46 branch contains Issues 43, 44, and 45; Issue 49 diverged
after Issue 43. Both predecessor histories were integrated before this design
was written. Issue 49's shared Unicode display-width utilities remain intact,
while title truncation will deliberately use a byte budget because OSC payload
size, rather than screen columns, is the contract here.

## Change

- Add a leaf `tui::terminal_notifications` module with a process-scoped guard.
  It detects stdout TTY status independently of the footer and honors both the
  current and legacy disable environment names.
- Project `ultra_phase_start` into an OSC 2 title of the form
  `CommandAgent — Phase <index>/<total>: <id>`. Flatten and sanitize the text
  with the established terminal control/bidi policy, then truncate on a UTF-8
  boundary to at most 120 payload bytes.
- Track command start time using an injectable monotonic duration. REPL starts
  come from the existing lifecycle event; direct CLI completion guards mark
  their start explicitly because they currently emit only the stop event. A
  matching stop emits exactly one standalone BEL when elapsed time is at least
  10 seconds.
- Clear the title with an empty OSC 2 sequence when the process guard drops.
  The direct SIGINT exit path will explicitly finish the notifier because
  `process::exit` does not run destructors.
- Keep terminal writes serialized through stdout locking. No event names,
  fields, footer state, spinner behavior, or `.anvil/` runtime paths change.

## Verification

Focused unit tests will cover TTY and non-TTY detection, current and `ANVIL_`
disable names, exact 10-second bell behavior with injected time, one-bell
semantics, title sanitization, UTF-8-safe 120-byte truncation, and title clear.
The existing provider-backed PTY scenario will assert the exact OSC 2 phase
bytes and empty-title cleanup with the footer both enabled and disabled. The
English and Japanese environment-variable references will document both
controls. Formatting, Clippy, and the full Rust suite are required because the
shared CLI/TUI lifecycle is touched.
