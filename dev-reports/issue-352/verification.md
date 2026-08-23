# Issue #352 verification

- Status: `passed`

## Checks

- `cargo test --lib caller_owned_top_level_exclusion_omits_nested_route_evidence`: `passed`
- `cargo test --features gui --test gui_server later_gate_one_ignores_products_from_an_isolated_session_workspace`: `passed`
- `cargo test --features gui --test gui_server delegated_session_rejects_a_symlinked_workspace_root`: `passed`
- `cargo test --features gui --test gui_server confirmed_session_delegates_with_cli_event_bytes_unchanged`: `passed`
- `cargo test --features gui --test gui_server`: `passed`
- `cargo test --test gui_read_only_guard`: `passed`
- `cargo test --test doc_drift`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `cd gui && npm run build`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run smoke -- --output /tmp/commandagent-issue-352-gui-smoke --provider-only`: `passed`
