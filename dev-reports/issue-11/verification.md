# Issue #11 Verification

- Status: `passed`

## Checks

- `cargo test tui::markdown:: --lib`: `passed`
- `cargo test --test tui_integration tui_markdown_raw_session_storage -- --exact`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
