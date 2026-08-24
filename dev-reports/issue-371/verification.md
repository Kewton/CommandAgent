# Issue 371 verification

- Status: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo test --test gui_read_only_guard -- --nocapture`: `passed`
- `cargo test --test doc_drift -- --nocapture`: `passed`
- `cargo test --features gui --test gui_server`: `passed`
- `cargo test`: `passed`
- `(cd gui && npm run typecheck)`: `passed`
- `(cd gui && npm run lint)`: `passed`
- `(cd gui && node --check scripts/smoke.mjs)`: `passed`
- `(cd gui && npm run build)`: `passed`
- `(cd gui && npm run smoke -- --output ../dev-reports/issue-371/browser-smoke --read-only --commandagent-bin ../target/debug/commandagent)`: `passed`
