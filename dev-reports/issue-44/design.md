# Issue 44 design: make the documented PTY launchers execute ignored tests

## Context

All three tests in `tests/tui_pty.rs` are deliberately protected by both
`#[ignore]` and the `COMMANDAGENT_PTY_TESTS` environment gate (with the legacy
`ANVIL_PTY_TESTS` alias handled by `env_compat`). The `just test-pty` recipe
and the matching raw command in `CONTRIBUTING.md` set the environment gate but
do not tell libtest to include ignored tests, so both commands report success
without executing any PTY test.

Issue 43 is complete on its predecessor branch but is not merged into this
worktree. Its committed change adds a fourth ignored, environment-gated PTY
test. The launcher fix must therefore select all ignored tests rather than
encode the current test count, so it remains compatible when that predecessor
is integrated.

## Scope and design

- Append `-- --include-ignored` to the `test-pty` recipe and to the raw Cargo
  command in `CONTRIBUTING.md`. This passes the flag to libtest and runs both
  ordinary and ignored cases in the selected `tui_pty` integration target.
- Keep `#[ignore]` on PTY tests because they require a Unix-like pseudo-terminal
  and are intentionally excluded from ordinary `cargo test` runs; the
  environment gate remains a second explicit opt-in safeguard.
- Add a focused `tests/doc_drift.rs` regression that reads the `test-pty`
  recipe command, requires the ignored-test opt-in flag, and requires the same
  command to remain documented in `CONTRIBUTING.md`.
- Add an Unreleased changelog entry because the fix changes a documented
  contributor workflow.

No production code, event schema, corpus fixture, release workflow, or live
runtime state changes are required. An opt-in CI job is intentionally outside
this smallest coherent fix because the acceptance criteria require repairing
the local documented launchers, not expanding platform CI coverage.

## Verification

Run the doc-drift target first, then run the documented PTY recipe and confirm
that its result is `3 passed` rather than `0 passed`. Because the doc-drift
test is CI-sensitive Rust test code, also run formatting, Clippy across all
targets, the full Rust suite, and a whitespace-error check.
