# Issue #355 verification

- Status: `passed`

## Checks

- `cargo test --features gui --bin gui_server session_diagnostics::tests::projects_terminal_release_gate_and_probe_findings`: `passed`
- `cargo test --features gui --test gui_server gui_server_check_reports_static_contract_mismatch -- --exact`: `passed`
- `cargo test --features gui --test gui_server session_index_requires_authentication_tracks_directories_and_caps_results -- --exact`: `passed`
- `cargo test --features gui --test gui_server confirmed_session_delegates_with_cli_event_bytes_unchanged -- --exact`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo test --features gui`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run build`: `passed`
- `cd gui && node --check scripts/smoke.mjs`: `passed`
- `cd gui && npm run smoke -- --output /tmp/commandagent-issue-355-final-smoke --feedback-only`: `passed`
- `git diff --check`: `passed`
