# Issue 43 design

## Observed mechanism

The fixed footer installs a DECSTBM scroll region and repaints it from a
background thread while `rustyline`, Markdown presentation, and the spinner use
independent terminal writers. After Enter, the normalized REPL line is saved to
history and emitted to events, but there is no durable presentation of the
accepted command. A long wrapped prompt can therefore be cleared or displaced
by footer repaint/resize even though command execution proceeds normally.

## Change

- Add a leaf command-receipt projection that parses the accepted slash command
  once and renders a sanitized, line-oriented breadcrumb before preflight or
  provider work. It will always show command and Goal, and will show profile,
  style, and prompt layout when the user supplied those flags. The receipt will
  preserve the full accepted Goal by wrapping instead of truncating it.
- Serialize durable presentation output with footer repainting by exposing a
  footer write operation. While frozen, it resets DECSTBM, clears the old
  footer rows, writes the receipt through the full-screen scroll region, then
  reinstalls the current resized footer geometry and requests a repaint. This
  prevents the background painter from racing the receipt and allows terminal
  emulators to retain receipt lines in normal scrollback.
- Extend presentation/status state with the active accepted command, Goal,
  run ID, profile/style/layout, and the latest phase. `/status` will include
  that state. Phase and provider events continue to drive the existing footer,
  with notable retry/recovery/interrupt events promoted to concise scrollback
  breadcrumbs.
- Make direct-action final presentation action-aware. Setup interaction probe,
  model probe, step-plan generation, and UltraPlan generation retain their
  own result output and lifecycle/exit semantics without receiving the generic
  coding-task gate table. Doctor, runs, offline UX demo, completions, and man
  generation remain outside that generic summary path as already audited.
- Clarify in both READMEs and the demo notes that `--ux-demo` and the hand-made
  SVG are scripted documentation artifacts, distinct from the checked-in
  recording of an actual provider-backed REPL walkthrough.

## Compatibility and verification

No event name or schema is changed, and `.anvil/` layout is untouched. Receipt
text uses the repository's control/bidi sanitization policy and remains plain
under `NO_COLOR`. Focused unit tests will cover explicit/default fields, CJK and
long wrapping, control/bidi/escape sanitization, live status projection, event
classification, and direct-action applicability. PTY screen-state tests will
exercise footer on/off and color/no-color; at least one case will leave spinner
and Markdown enabled. Then formatting, Clippy, doc-drift checks, and the full
Rust suite will run because shared TUI and CLI behavior are affected.
