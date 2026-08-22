# Design: Issues #210, #222, and #209

## Scope

Implement the approved combined TUI correction without changing provider HTTP
cancellation, event schemas, verification gates, or repair execution limits.
The two commits currently ahead on `origin/develop` affect only CommandMate
orchestration scripts and tests, so this row does not need their changes.

## Design

1. Add an explicit interrupt-cleared status transition and publish it whenever
   `InterruptMonitor::reset` clears the monitor flags. This keeps the monitor,
   footer, and `/status` projection in the same completed state before the next
   REPL command.
2. Keep command status tracking intact, but replace Playwright availability
   command excerpts with a stable `checking interaction probe` footer label.
   Detection is limited to the Node `require.resolve('playwright')` probe and
   the `npm root -g` lookup used by that availability path.
3. Put repair display normalization in a small TUI leaf helper. For a positive
   maximum, render `min(attempt, maximum)/maximum`; for an unknown zero maximum,
   retain the existing unbounded single-number form. Apply the helper to event
   projection, runtime status projection, and footer rendering so no visible
   `N/M` has `N > M`.
4. Render Python dependency lifecycle events as Python dependency setup rather
   than npm installation. Detect Python from the event profile or setup kind;
   preserve the existing npm label for non-Python lifecycle events.

## Tests

- Extend the focused ignored PTY interrupt regression to issue `/status` after
  the interrupted command and verify that neither the stopping footer nor the
  `interrupt requested` scope survives.
- Add footer rendering regressions for probe redaction and over-limit repair
  counters.
- Add activity rendering regressions for over-limit repair events and Python
  dependency lifecycle events.
- Add status/interrupt unit coverage for the new clear transition, then run the
  focused TUI tests followed by formatting, Clippy, and the full Rust suite.
