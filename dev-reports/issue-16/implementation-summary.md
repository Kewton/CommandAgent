# Issue #16 Implementation Summary

## Outcome

CommandAgent now presents `COMMANDAGENT_*` environment variables and
`.commandagent/config(.toml)` as the canonical external interfaces while
continuing to read their pre-rename equivalents. The live `.anvil/` runtime
state namespace, event schemas, and historical evidence remain unchanged.

## Implementation

- Added a Rust environment compatibility leaf module with canonical-first
  lookup, derived legacy names, support for string and OS-string values, and a
  process-wide once-per-variable deprecation warning.
- Added injectable matrix coverage for new-only, old-only, both-present, and
  absent values, plus repeated old-only lookup coverage for the warn-once
  contract.
- Routed product, TUI, planner, minimal-loop, eval, build-verifier, conformance,
  PTY, and live-provider environment reads through the compatibility boundary.
  Child-process allowlists accept canonical or derived legacy names without
  duplicating legacy spellings at call sites.
- Migrated build-script inputs and embedded build metadata to canonical names.
  The build script registers both canonical and derived legacy force-rebuild
  inputs with Cargo.
- Added Python and Bash compatibility helpers for script-only eval and benchmark
  variables, and migrated eval suites, tests, docs, and examples to canonical
  names.
- Changed TOML discovery order to workspace new, workspace legacy, home new,
  then home legacy. Changed extensionless workspace config discovery to new
  before legacy. Existing per-field merging and workspace-over-home behavior are
  preserved.
- Added focused config tests for new-only, old-only, both-present, and
  extensionless precedence. Updated the README, UAT guide, and mechanism ledger
  for Phase 2.

## Scope Control

- Integrated the verified aggregate predecessor history before Issue #16 work.
- Kept tripwire changes to environment-name substitutions and helper wiring:
  `src/minimal_loop/loop_run.rs` is net-zero lines and
  `src/planner/runner.rs` changes only the affected reads, diagnostics, and test
  fixtures.
- Added no event-schema, recovery-contract, corpus, or runtime-state migration.
