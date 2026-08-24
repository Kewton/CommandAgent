# Issue #376 verification

- Status: `passed`

## Checks

- `cargo test session_tasks --features gui --bin gui_server`: `passed`
- `cargo test --features gui --test gui_server`: `passed`
- `cargo test --test gui_read_only_guard`: `passed`
- `cargo test --test doc_drift`: `passed`
- `cd gui && node --check scripts/session-index-smoke.mjs && node --check scripts/smoke.mjs`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run build`: `passed`
- `cd gui && npm run smoke:session-index -- --output /tmp/commandagent-issue-376-session-index-smoke`: `passed`
- `cd gui && npm run smoke -- --feedback-only --output /tmp/commandagent-issue-376-feedback-smoke`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`
