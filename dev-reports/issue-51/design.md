# Issue #51 Design

## Goal

Make the REPL's existing multi-line input continuation discoverable from both
runtime `/help` and the English and Japanese user guides without changing the
editor behavior.

## Predecessor state

- Issue 43 (`6a226f6`) and Issue 45 (`6026317`) both passed verification but
  are not ancestors of this worktree.
- Issue 43 changes `src/tui/slash.rs` and `tests/doc_drift.rs`; Issue 45 builds
  on it and also changes both slash-command guides. This Issue will keep its
  edits localized so those commits can be integrated without behavioral
  coupling.
- The current editor continues input when a line ends in `\` or contains an
  unclosed double quote, renders `... ` before continued lines, removes
  continuation backslashes, and joins submitted lines with spaces.

## Design

1. Add one concise `Multi-line input:` line to `render_help` describing both
   continuation triggers, the `... ` prompt, and how to finish the command.
2. Add matching dedicated sections to the English and Japanese slash-command
   guides. Include examples and explain that line breaks become spaces and
   trailing continuation backslashes are removed before existing parsing.
3. Extend the focused slash help unit test with the exact new help line.
4. Extend `tests/doc_drift.rs` with a focused contract that checks the help and
   both language guides for the same continuation triggers, prompt, and
   normalization semantics. Existing heading-count parity continues to enforce
   matching EN/JA section structure.

## Verification

Run `cargo test --test doc_drift` and the focused slash/editor library tests
first. Then run the required formatting, Clippy, and full test-suite checks,
followed by `git diff --check`.
