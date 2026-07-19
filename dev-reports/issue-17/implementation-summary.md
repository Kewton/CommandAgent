# Issue #17 Implementation Summary

## Outcome

The Issue #17 branch was fast-forwarded to verified aggregate predecessor tip
`6cc03bd` before the decision record was applied. The mechanism ledger now
records the approved Option A for Phase 3.

## Decision Recorded

- Preserve `data-anvil-*`, `<anvil_tool_call>`, `anvil_app`, and the live
  `.anvil/` runtime-state namespace.
- Preserve JSON keys, event names, and schemas unchanged.
- Treat these spellings as compatibility-sensitive internal protocol
  identifiers rather than user-visible product branding.
- Require any future rename to proceed through a separately authorized,
  versioned or dual-read migration with fixture, corpus, and state-migration
  coverage.

## Scope Control

- Changed `docs/mechanism-ledger.md` only outside the required Issue #17
  development reports.
- Made no production-code, test, corpus, fixture, event, JSON-key, schema, or
  runtime-state changes.
- Added no behavior tests because executable behavior and shared machine
  contracts remain byte-for-byte at the aggregate predecessor tip.
