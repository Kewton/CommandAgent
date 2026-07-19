# Issue 10 Design

## Scope

Modernize only the interactive REPL input layer. Preserve the existing non-TTY
guard, command execution path, EOF behavior, history location, and the separate
runtime interrupt monitor.

## Design

- Add `src/tui/editor.rs` as a leaf module around rustyline. It owns command,
  flag, profile, and workspace-relative path completion; history/command hints;
  multiline validation and normalization; bracketed-paste configuration; and
  prompt-time Ctrl+C state.
- Define the 14 canonical slash commands once in `src/tui/slash.rs`, with
  `/quit` retained as an alias of `/exit`. Generate help from that table and
  resolve dispatch through its command kind, so completion/help/dispatch cannot
  drift independently.
- Expose canonical profile IDs from the existing domain-profile registry in
  `src/planner/profile.rs`. Completion consumes that function rather than
  copying profile strings into the editor.
- Keep multiline input in one rustyline buffer. A validator treats a trailing
  backslash or unmatched double quote as incomplete; the highlighter renders
  an ASCII `... ` continuation marker. Before dispatch, embedded newlines and
  backslash continuations are normalized to spaces for the existing parser.
- Track prompt key activity through rustyline custom bindings. Ctrl+C on a
  non-empty buffer clears it; the first uninterrupted Ctrl+C on an empty buffer
  warns; the next exits through the loop's normal history-saving path. Ctrl+D
  and execution-time interrupt handling remain unchanged.

## Verification

Add focused unit tests for registry synchronization, each completion class,
hints, color policy, multiline validation/normalization, workspace path
boundaries, and Ctrl+C state transitions. Then run formatting, clippy, and the
full Rust test suite because shared TUI and profile registry contracts change.
