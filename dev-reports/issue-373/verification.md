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

## Dependency CI-fix propagation

- Incoming Issue 370 head: `9e8e178b97b49c78411ad9d2ba1783168227cdd9`
- Incoming Issue 369 CI-race fix: `f0fb9ccfb6572f952c3a1b5d146d41b8b92eadac`
- Applied CI-race cherry-pick: `927f7ff3`
- Applied dependency-verification cherry-pick: `2a8d3891`
- `git diff --exit-code adbe8287 0a9d2a0a --`: `passed`
- `git diff --exit-code 31410760 032ed840 --`: `passed`
- `git diff --exit-code f0fb9ccf HEAD -- dev-reports/issue-369`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --features gui --test gui_server typed_trial_intents_are_validated_frozen_and_delegated -- --exact`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --features gui --test gui_server`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --test gui_read_only_guard`: `passed`
- `cd gui && npm run smoke -- --overview-only --output /tmp/commandagent-issue-373-dependency-ci-fix-overview-smoke --commandagent-bin ../target/debug/commandagent`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo clippy --all-targets -- -D warnings`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`
