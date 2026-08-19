# Issue #110 Implementation Summary

## Implemented

- Integrated the completed Issue #109 prerequisite chain, including the shared
  admitted pack catalog and exact pack-selection support.
- Added `--packs`, `--pack-verify <DIR>`, and `--pack-pin <DIR>` to the public
  CLI with help text, required listing context, and Clap-level conflicts.
- Added `src/pack_actions.rs` as an offline leaf module dispatched before
  configuration/provider/run setup.
- Made `--packs` enumerate compatible admitted catalog entries and conformant
  extension-root entries in deterministic order with `admitted` or `local`
  source labels.
- Made `--pack-verify` call the same `conform_directory` implementation as the
  standalone `pack_conformance` binary and render the same pretty JSON report.
- Made `--pack-pin` conform before mutation, create a new `pack.sha256`, treat
  an identical existing pin as a no-op, and reject stale pins without
  overwriting them.

## Tests and documentation

- Added CLI help, requirement, and conflict tests in `src/cli.rs`.
- Added `tests/pack_actions.rs` coverage for two admitted plus one local list
  entries, direct-versus-standalone verification equality, and pin
  create/no-op/tamper behavior including exit status 1.
- Updated the English and Japanese CLI references, the first-loop guide, and
  `packs/README.md` with the new direct commands and source/pin semantics.
- No event schema, pack schema, planner chokepoint, historical evidence, or
  live `.anvil/` runtime namespace was changed.
