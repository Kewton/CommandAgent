# Issue #22 Implementation Summary

## Implemented

- Added `tests/doc_drift.rs` with four CI-discovered integration tests covering:
  - every non-hidden long flag derived from
    `Cli::command().get_arguments()`, compared bidirectionally with the English
    CLI reference table;
  - every slash-command name and alias parsed from `render_help`, compared with
    both the dispatch registry and the English slash-command table;
  - supported preset and top-level configuration keys compared bidirectionally
    with their English configuration tables; and
  - exact EN/JA guide file-set parity plus H2/H3 count parity for each pair.
- Added shared failure reporting that lists every item missing from either side
  and names both the runtime source and documentation path to fix.
- Added `SUPPORTED_PRESET_KEYS` and `SUPPORTED_TOP_LEVEL_KEYS` to
  `src/config.rs`. The parser checks these arrays before dispatching a key, so
  they are the authoritative supported-key inventory rather than test-local
  copies.
- Exposed the existing `render_help` function for integration testing. Help and
  `handle_command` continue to resolve through the same `SLASH_COMMANDS`
  registry.
- Updated both English and Japanese CLI/slash guides for Issue 25's `--doctor`,
  `--json`, and `/doctor` additions while preserving matching heading structure.

## Integration notes

- Integrated the completed Issue 19/20/21 documentation stack and the
  independent Issue 23 and Issue 25 commits before implementing the guard.
- The existing CI workflow already runs `cargo test --all-targets`; the exact
  command discovered and passed the new integration test, so no workflow change
  was necessary.
- No event schema, corpus contract, historical evidence, or `.anvil/` runtime
  namespace changed.
