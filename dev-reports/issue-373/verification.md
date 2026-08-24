- Status: `passed`

## Checks

- `cd gui && npm ci --include=dev`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && node --check scripts/smoke.mjs`: `passed`
- `cd gui && GUI_BASE_PATH=/ npm run build`: `passed`
- `cd gui && GUI_BASE_PATH=/proxy/commandagent/ npm run build`: `passed`
- `cd gui && npm run smoke -- --overview-only --output /tmp/commandagent-issue-373-overview-smoke-final --commandagent-bin ../target/debug/commandagent`: `passed`
- `cd gui && npm run smoke:session-index -- --output /tmp/commandagent-issue-373-session-index-smoke`: `passed`
- `cargo test --test gui_read_only_guard -- --nocapture`: `passed`
- `cargo test --test doc_drift -- --nocapture`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo test --features gui --test gui_server`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`
