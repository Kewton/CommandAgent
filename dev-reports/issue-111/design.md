# Issue #111 Design: REPL pack selection and switching

## Scope and dependency base

Add pack selection to the existing Gate 1 REPL boundary, expose the Issue #110
pack listing through `/packs`, and make the Gate 4 `pack_change` action usable
through `/pack <id@version>`. The implementation will first integrate the
committed Issue #110 dependency stack (#107 through #110), which owns the
catalog, exact-byte selection, CLI flags, and listing behavior. The other
completed predecessors were inspected but are unrelated documentation, GUI,
Next.js runtime, or setup work and will remain separate.

Pack schemas, confirmation schemas, runtime environment names, existing event
schemas, and the live `.anvil/` namespace remain unchanged. Confirmed REPL
dispatch adds one backward-compatible schema-v1 `pack_injected` event so the
runtime installation required by the acceptance criteria is machine-visible.

## Parsing and selection

- Extend the non-slash request parser so a trailing or embedded
  `--pack <id@version>` is removed from the request text and retained as a
  typed selector. Missing, duplicated, or malformed values fail before Gate 1.
- Resolve the selector through the same exact-byte catalog/locator contract as
  direct CLI selection. Gate 1 therefore freezes the id, version, hash,
  verification location, and supply source already rendered by the boundary
  presentation.
- `/pack <id@version>` is accepted only while a failed terminal identity is
  active. It keeps the failed request and route pins, resolves the replacement
  pack, and starts a fresh Gate 1 confirmation. It does not dispatch until the
  new card is confirmed.

## Dispatch and event evidence

When `/confirm` dispatches a pinned identity, install that frozen pack through
the existing scoped runtime pack environment used by Issue #109. The guard
will cover command execution and restore the previous process environment
afterward. After successful installation and before command execution, emit the
additive `pack_injected` record with the frozen identity, point, source, and
card hash. No existing event or verification gate is weakened.

## `/packs` parity

Move or expose Issue #110 listing as a reusable renderer driven by profile,
intent, extension root, and workspace root. Both `--packs` and `/packs` call
that renderer, so headers, ordering, hashes, sources, filtering, and errors are
identical. `/packs` derives profile/intent from the current REPL configuration.

## Tests and documentation

Add focused parser tests in `src/tui/slash.rs` for inline `--pack`; boundary
tests in `src/tui/repl.rs` for Gate 1 selection, confirmed runtime injection,
listing parity, and Gate 4 pack switching; and focused listing tests if the
shared renderer requires them. Update the English and Japanese slash-command
guides, the first-loop guide, integration notes, and the D-3c design where the
user-visible boundary contract is described. Run focused TUI/pack tests first,
then `tests/doc_drift.rs`, formatting, Clippy, and the full Rust suite because
shared CLI/TUI pack behavior is touched.
