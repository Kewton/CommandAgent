# Issue 49 design

## Observed mechanism

`tui::presentation::fit` delegates to `util::excerpt_with_marker`, whose limit
is measured in UTF-8 bytes. A 120-byte Goal therefore retains 120 ASCII
characters but only about 40 three-byte Japanese characters. The footer
already measures terminal columns and skips ANSI CSI sequences, but keeps that
logic private; the Markdown table renderer and the completed Issue 43 command
receipt also carry local display-width implementations.

The required predecessor, Issue 43 commit `6a226f6`, changes the Plan card and
adds a durable accepted-command receipt. Issue 49 must build on that committed
presentation behavior so its Goal fitting and receipt wrapping do not diverge
when the branches are integrated.

## Change

- Fast-forward this worktree to the completed Issue 43 commit before production
  edits, preserving the dependency as an explicit ancestor.
- Add shared `char_display_width`, `display_width`, `display_width_ansi`, and
  `fit_display_width` helpers in `src/util.rs`. Use the existing
  `unicode-width` transitive crate as a direct dependency for Unicode column
  widths, while retaining the footer's ANSI CSI rule that escape sequences
  occupy zero columns. Fitting will stop only at UTF-8 character boundaries,
  retain zero-width combining characters that follow the accepted prefix, and
  never split a recognized CSI sequence.
- Replace every call through `tui::presentation::fit` with the shared fitting
  API, preserving newline normalization and the existing ASCII content budget.
  Route footer fitting/padding, Markdown table measurement, and the Issue 43
  command-receipt wrapper through the same width functions.
- Keep `excerpt_with_marker` unchanged for persisted or protocol-oriented uses,
  including provider parsing, run summaries, and event snippets. No event
  schema, field, or truncation contract changes.

## Display truncation audit

- `input_queue::preview` is user-visible in queue confirmations and the footer,
  so its 40-character cap becomes a 40-column cap through
  `fit_display_width`.
- `status_bus::sanitize_command_excerpt` feeds the live footer/status display,
  so its 120-byte cap becomes a 120-column cap through the same API.
- Spinner labels are user-visible but have no truncation cap: they only replace
  terminal controls and bidi controls. There is no fitting implementation to
  migrate, so sanitization remains unchanged.

## Compatibility and verification

Focused unit tests will cover the 120-column Japanese/ASCII Plan Goal contract,
shared fitting for Japanese, emoji, combining marks, ANSI CSI, and very small
budgets, plus queue and command-excerpt migrations. Existing footer and
Markdown tests will verify shared measurement. Event snippet tests and the
standard event fixture projection will be run to confirm record output remains
unchanged. Because a shared Rust utility and multiple TUI consumers change,
formatting, Clippy, and the full Rust suite are required before handoff.
