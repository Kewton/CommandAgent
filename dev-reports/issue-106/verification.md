# Issue 106 verification

- Status: `passed`

## Checks

- `GUI_BASE_PATH=/ npm run build`: `passed`
- `cargo test --features gui --test gui_server confirmed_session_delegates_with_cli_event_bytes_unchanged -- --exact`: `passed`
- `cargo test --features gui --test gui_server --test gui_read_only_guard --test protection_coverage_audit`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `cargo test --features gui --quiet`: `passed`
- `cargo test --quiet`: `passed`
- `git diff --check`: `passed`
