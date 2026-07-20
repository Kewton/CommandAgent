# Issue 47 implementation summary

## Implemented

- Integrated the completed Issue 46 predecessor history (including Issues 43,
  44, and 45) and merged Issue 49's display-width work before changing the
  overlapping TUI surface.
- Added `tui::terminal_notifications` as a leaf module. A process-scoped guard
  enables output only when stdout is a TTY and independently detects terminal
  title and bell suppression through `COMMANDAGENT_*` or legacy `ANVIL_*`
  environment names.
- Projected existing `ultra_phase_start` events to exact OSC 2 sequences. The
  title contains phase index, total, and ID; terminal controls and bidi controls
  are neutralized, and the payload is truncated to at most 120 bytes without
  splitting UTF-8.
- Timed existing REPL command lifecycle events and direct CLI completion guards
  with monotonic time. A stop at or beyond 10 seconds emits one standalone BEL;
  shorter commands and duplicate stops emit none.
- Cleared an active title with an empty OSC 2 sequence on normal process
  teardown, panic unwinding, and the direct SIGINT `process::exit` path.
- Kept the feature independent of `Footer`, so title progress and the bell work
  under `--footer off` without changing footer, spinner, or event behavior.

## Tests and documentation

- Added unit coverage for TTY gating, current and legacy suppression names,
  injected short/exact-threshold timing, one-bell behavior, direct and REPL
  starts, exact title bytes, sanitization, UTF-8 truncation, and one-time title
  cleanup.
- Extended the provider-backed PTY footer/color matrix to assert exact phase
  title bytes and empty-title cleanup, including footer-off cases.
- Documented the terminal-title progress, completion bell, and both environment
  controls in the English and Japanese user documentation.

## Compatibility

No event name or schema, `.anvil/` runtime path, footer state, spinner behavior,
or historical evidence was changed. No corpus fixture update was needed because
the emitted event contract is unchanged; the new terminal bytes are verified at
the TTY boundary instead.
