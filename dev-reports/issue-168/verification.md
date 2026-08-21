# Issue #168 verification

- Status: `passed`

## Checks

- `node --check gui/scripts/smoke.mjs`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cargo test --test gui_read_only_guard run_detail_and_measurement_read_only_browsing_contracts_are_pinned -- --exact`: `passed`
- `cd gui && npm run smoke -- --read-only --output /tmp/commandagent-issue-168-browser-smoke-escalated --commandagent-bin ../target/debug/commandagent`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo test --test gui_read_only_guard`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `cargo fmt --all -- --check` (CI follow-up): `passed`
- `cargo +1.97.1 clippy --features gui --bin gui_server -- -D warnings`: `passed`
- `cargo test --features gui --test gui_server trial_session_files_`: `passed`
- `cargo test --test gui_read_only_guard run_detail_and_measurement_read_only_browsing_contracts_are_pinned -- --exact` (CI follow-up): `passed`
- `cd gui && npm run lint && npm run typecheck`: `passed`
- `cd gui && npm run smoke -- --read-only --output /tmp/commandagent-issue-168-ci-follow-up-smoke --commandagent-bin ../target/debug/commandagent`: `passed`
