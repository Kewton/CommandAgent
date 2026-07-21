# Issue 61 Implementation Summary

## Outcome

Raw-mode TTY text writes now return to column one for every logical line. The
accepted-command receipt and complete or streaming Markdown output no longer
depend on the terminal driver's cooked-mode LF expansion.

## Changes

- Added a terminal leaf helper that converts bare LF to CRLF only while stdout
  is a TTY and crossterm raw mode is enabled. Existing CRLF, cooked-mode output,
  redirected output, ANSI bytes, and UTF-8 content are preserved.
- Routed footer scrollback text through the helper, covering accepted receipts
  with the footer enabled or disabled while retaining the existing footer
  freeze, resize, repaint, cleanup, and scrollback sequences.
- Routed batch Markdown, raw and rendered Markdown streams, stream completion,
  failure blocks, status cards, and terminal summaries through the same helper.
  Markdown capture remains LF-based and therefore keeps its existing storage
  contract.
- Strengthened receipt tests for exact CJK continuation indentation and
  column-zero field prefixes, including the optional requested-port and run-ID
  fields.
- Strengthened the PTY matrix with an ANSI-aware cursor model that verifies the
  rendered column of the receipt heading, every top-level field, and every CJK
  continuation line across footer/color combinations. Layout checks no longer
  rely on the content-only normalization that trims indentation; resize,
  failure/status, Goal preservation, and cleanup coverage remain intact.

## Contract Impact

No event names, JSON schemas, corpus fixtures, accepted Goal storage, or
`.anvil/` runtime paths changed. README and demo assets did not require updates
because the user-facing command or documentation contract is unchanged.
