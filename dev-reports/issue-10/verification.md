# Issue #10 Verification

- Status: `passed`
- `cargo test tui::editor::tests --lib`: `passed`
- `cargo test tui::slash::tests --lib`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `ANVIL_PTY_TESTS=1 cargo test --test tui_pty -- --ignored`: `passed`
