# Issue 61 Design

## Problem

The interrupt monitor restores crossterm raw mode as soon as REPL input is
accepted. In raw mode the terminal no longer expands LF into CRLF, but the
accepted-command scrollback writer and the Markdown output paths still write
LF-only strings. Each newline therefore advances the row without returning to
column one, producing stair-step receipts and the same defect in streamed or
multi-line command output.

## Design

Add a small terminal leaf helper that writes text with raw-mode-aware line
endings. For a TTY with raw mode enabled, each LF that is not already part of a
CRLF pair is emitted as CRLF. Outside that condition, bytes remain unchanged so
redirected output and normal cooked-mode behavior retain their existing
contracts.

Route the two shared stdout text boundaries through the helper:

- footer scrollback text, which carries the accepted-command receipt with the
  footer both enabled and disabled;
- complete and streaming terminal Markdown, which carries Markdown responses,
  failure blocks, status cards, and terminal summaries.

Keep rendering and capture buffers LF-based. Do not change receipt wrapping,
Goal preservation, footer control sequences, event names or schemas, or the
`.anvil/` runtime namespace.

## Verification

- Unit-test raw and cooked line-ending behavior, including existing CRLF and
  UTF-8/CJK text.
- Keep the focused receipt wrapping test for narrow CJK continuation indents
  and add a wide-width assertion where useful.
- Strengthen the existing PTY receipt matrix with an ANSI-aware cursor model
  that verifies the real start column of every receipt line across footer/color
  combinations while it exercises terminal resize, long CJK Goal preservation,
  intentional continuation indentation, failure output, status output, and
  cleanup. Content-only normalization remains separate from layout assertions.
- Run focused TUI tests, formatting, Clippy, and the full Rust test suite because
  the shared terminal output boundary changes.
