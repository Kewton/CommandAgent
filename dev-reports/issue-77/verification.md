- Status: `passed`

## Checks

- `cd gui && npm run lint`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run build`: `passed`
- `cd gui && node --check scripts/smoke.mjs`: `passed`
- `cargo test --test gui_read_only_guard`: `passed`
- `cargo build --release --bin commandagent`: `passed`
- `target/release/commandagent --version`: `passed`
- `cd gui && npm run smoke -- --output /tmp/commandagent-issue-77-smoke.2fHcog --commandagent-bin ../target/release/commandagent`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

## Evidence

- Smoke report: `/tmp/commandagent-issue-77-smoke.2fHcog/browser-smoke.json`
- Smoke result: both root and proxy-base-path cases reported `ok: true`.
- Screenshots: desktop and 390px mobile Gate 1, Gate 2, and terminal images for
  both base-path cases are stored beside the smoke report.
- The first sandboxed smoke attempt could not bind `127.0.0.1:0`; the exact
  command was rerun with approved localhost access and passed.
