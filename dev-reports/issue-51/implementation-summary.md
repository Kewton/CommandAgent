# Issue #51 Implementation Summary

## Implemented

- Added a `Multi-line input:` line to runtime `/help` that explains the two
  existing continuation triggers: a trailing `\` and an unclosed double quote.
  The help also identifies the `... ` prompt and the conditions for submitting
  the completed command.
- Added matching `Multi-line input` / `複数行入力` sections to the English and
  Japanese slash-command guides. Both sections include the same examples and
  explain that continuation backslashes are removed and line breaks become
  single spaces before command parsing.
- Updated the `/help` command-reference row in both languages so its behavior
  description includes multi-line continuation.
- Extended the focused slash help unit test with the exact new runtime line.
- Added a `doc_drift` contract that checks runtime help plus both guide sections
  for the continuation triggers, prompt, submission rule, and normalization
  semantics. Existing bilingual heading-count parity also covers the new
  section.

## Scope notes

- The editor and parser behavior did not change; this Issue only documents the
  already-tested continuation behavior.
- No event schema, recovery contract, corpus fixture, historical evidence, or
  `.anvil/` runtime state changed.
- Issue 43 (`6a226f6`) and Issue 45 (`6026317`) were inspected as non-ancestor
  predecessor commits. This patch does not merge them and keeps its overlapping
  changes localized to help assertions and guide sections.
