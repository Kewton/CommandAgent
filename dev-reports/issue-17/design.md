# Issue #17 Design

## Decision

Adopt the approved Option A and document it in
`docs/mechanism-ledger.md`: preserve `data-anvil-*`, `<anvil_tool_call>`,
`anvil_app`, the live `.anvil/` runtime namespace, JSON keys, event names, and
schemas. These are compatibility-sensitive internal protocol identifiers, not
remaining user-visible product branding.

## Scope

- Starting from verified aggregate predecessor tip `6cc03bd`, add one dated
  Phase 3 decision entry immediately after the existing Phase 2 record.
- State the preserved identifiers and the compatibility rationale explicitly.
- Record that a future rename requires a separately authorized, versioned
  migration with compatibility, fixture, corpus, and runtime-state coverage.
- Make no production-code, fixture, schema, event, JSON-key, or runtime-state
  changes. The only task artifact outside the ledger is this issue's required
  development reporting.

## Predecessor Context

- Issue #13 changes fixed-footer resize behavior and does not alter branding or
  protocol identifiers.
- Issue #15 changes user-visible product branding while preserving internal
  compatibility identifiers.
- Issue #16 makes `COMMANDAGENT_*` and `.commandagent/config*` canonical with
  legacy read fallback, while explicitly leaving live `.anvil/` state and
  event schemas unchanged.

## Overfitting Review

This decision fossilizes the existing internal `anvil` spellings at current
LLM, machine-data, and runtime-state boundaries. It narrows maintainers'
freedom to perform a visually complete global rename, but does not constrain
user-facing naming or future versioned protocols. The honest degradation path
is to describe these spellings as compatibility identifiers and, if their
maintenance cost becomes material, open a dedicated migration that versions
or dual-reads the affected contracts rather than silently rewriting them.

## Verification

Use a focused term scan to confirm the ledger names every preserved boundary,
then run `git diff --check`. No Rust or corpus tests are required because this
issue intentionally changes no executable behavior or fixture contract.
