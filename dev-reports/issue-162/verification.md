# Issue 162 verification

- Status: `passed`

## Checks

- `cd gui && npm ci --include=dev --offline`: `passed`
- `cd gui && node --check scripts/smoke.mjs`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run smoke -- --output /private/tmp/commandagent-issue162-feedback --feedback-only --commandagent-bin ../target/debug/commandagent`: `passed`
- `cargo test --test gui_read_only_guard`: `passed`
- `cargo test --features gui --test gui_server`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `cargo test`: `passed`
- `cargo test --features gui`: `passed`
- `git diff --check`: `passed`

## Evidence

- `/private/tmp/commandagent-issue162-feedback/browser-smoke.json` reports
  overall `ok: true` for `/` and `/proxy/commandagent/`.
- In both base-path cases, elapsed time advanced from 3 seconds before reload to
  4 seconds after reconnect (`elapsed_preserved_after_reconnect: true`).
- In both cases, the measured mean remained `平均 10.2 分` after reconnect
  (`mean_preserved_after_reconnect: true`) instead of becoming `未記録`.
- The GUI server integration target passed all 26 tests, including equality of
  the Gate 1 and reconnected status average-duration values. The GUI source
  guard passed all 24 tests.

## Environment note

The first sandboxed browser-smoke attempt could not bind a loopback port. The
same command was rerun with the required local-loopback permission and passed;
no test assertion was changed or weakened.
