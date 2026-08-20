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
