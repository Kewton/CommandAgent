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

## Dependency CI-fix propagation

Dependency head `9e8e178b` and its Issue #369 CI-race fix `f0fb9ccf` are
incorporated while the Issue #374 path-safety checks remain present.

- `git diff --exit-code f0fb9ccf -- dev-reports/issue-369`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --features gui --test gui_server typed_trial_intents_are_validated_frozen_and_delegated -- --exact`: `passed`
- `cargo test --features gui --test gui_server trial_session_paths_are_token_only_confined_and_report_missing_workspaces -- --exact`: `passed`
- `cargo test --features gui --test gui_server confirmed_session_delegates_with_cli_event_bytes_unchanged -- --exact`: `passed`
- `cd gui && node --check scripts/smoke.mjs`: `passed`
- `cd gui && npm run smoke:session-index -- --output /tmp/commandagent-issue-374-dependency-session-index-smoke`: `passed`
- `cd gui && npm run smoke -- --overview-only --output /tmp/commandagent-issue-374-dependency-overview-smoke-final --commandagent-bin ../target/debug/commandagent`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --features gui --test gui_server -- --test-threads=1`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --test gui_read_only_guard`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

The first overview-smoke attempt stopped at its inherited pre-Issue #370
`トライアル` heading expectation. After aligning that one assertion with the
verified `トライアル実行指示` page heading, the final command above passed
for both `/` and `/proxy/commandagent/`, including desktop and mobile Trial
coverage. No timeout or acceptance threshold was changed.
