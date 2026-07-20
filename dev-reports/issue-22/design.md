# Issue #22 Design

## Goal

Add an integration-test guard that fails when the English user guide drifts
from the public CLI, slash-command, or configuration surfaces, or when the
English and Japanese guide trees lose structural parity.

## Predecessor state

- Issue 21 contains the stacked Issue 19 and Issue 20 documentation commits and
  supplies the reorganized bilingual guide.
- Issue 23 adds repository policy documents on top of Issue 19.
- Issue 25 adds the `doctor` CLI/slash surfaces and config inspection behavior.
- Every predecessor verification report records `Status: passed`. Their
  committed changes must be integrated before the guard is implemented because
  this worktree was created from their earlier common base.

## Design

1. Add `tests/doc_drift.rs`, which derives non-hidden long flags through
   `Cli::command().get_arguments()`, derives accepted slash-command names from
   rendered help and the dispatch registry, and reads supported configuration
   keys from production exports.
2. Give each guarded Markdown table a deliberately small contract: its first
   cell starts with a backticked `--flag`, `/command`, or config key. Compare
   parsed and runtime sets in both directions and report every missing or stale
   item together with the relevant source and documentation paths.
3. Export the supported preset and top-level config-key arrays from
   `src/config.rs`, and make `render_help` callable by the integration test.
   Keep command dispatch on the existing shared slash registry so help and
   dispatch remain mechanically identical.
4. Compare the direct file-name sets in `docs/guide/en/` and
   `docs/guide/ja/`, then compare each pair's H2 and H3 counts.
5. Update both language guides for the Issue 25 doctor surfaces and for any
   public Clap-generated long flags covered by introspection. Do not document
   the hidden completion-contract integration flag.

## Verification

Run the focused `doc_drift` integration test first. Because shared Rust API and
CI-discovered integration tests change, also run formatting, Clippy with
warnings denied, the full test suite, and `cargo test --all-targets` to confirm
the existing CI command discovers the new guard.
