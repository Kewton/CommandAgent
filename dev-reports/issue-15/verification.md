# Issue #15 Verification

- Status: `passed`

## Checks

- `cargo test tui::banner::tests --lib`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo build && cargo test --quiet`: `passed`
- `ANVIL_PTY_TESTS=1 cargo test --test tui_pty`: `passed`
- `ANVIL_PTY_TESTS=1 cargo test --test tui_pty -- --ignored`: `passed`
- `rg -in 'anvil' src/tui src/repl.rs README.md docs/generality.md docs/perf-notes.md`: `passed`

## Notes

- The exact PTY acceptance command reports the smoke test ignored because the
  test retains its repository-level `#[ignore]` marker. The additional
  `-- --ignored` check executed the PTY test and passed.
- The first sandboxed full-suite attempt hit permission errors in existing
  local probe/process tests. The same required command passed outside the
  sandbox without code changes.
