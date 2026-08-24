# Issue #374 verification
- Status: `passed`

## Checks

- `cargo test --features gui --test gui_server trial_session_paths_are_token_only_confined_and_report_missing_workspaces -- --exact`: `passed`
- `cargo test --features gui --test gui_server confirmed_session_delegates_with_cli_event_bytes_unchanged -- --exact`: `passed`
- `cd gui && npm run typecheck && npm run lint && node --check scripts/session-index-smoke.mjs && npm run build`: `passed`
- `cd gui && npm run smoke:session-index -- --output /tmp/commandagent-issue-374-session-index-smoke`: `passed`
- `cargo test --test gui_read_only_guard`: `passed`
- `cargo test --test doc_drift`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `cargo test --features gui --test gui_server -- --test-threads=1`: `passed`
- `cd gui && npm run smoke -- --feedback-only --output /tmp/commandagent-issue-374-feedback-smoke`: `passed`
- `git diff --check`: `passed`
