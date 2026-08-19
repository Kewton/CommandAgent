# Issue 119 verification

- Status: `passed`

## Checks

- `cd gui && npm ci --include=dev --offline`: `passed`
- `cd gui && node --check scripts/session-index-smoke.mjs`: `passed`
- `cd gui && node --check scripts/smoke.mjs`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run build`: `passed`
- `cd gui && npm run smoke:session-index -- --output /private/tmp/commandagent-issue119-session-index-final`: `passed`
- `cd gui && npm run smoke -- --read-only --output /private/tmp/commandagent-issue119-read-only-final --commandagent-bin ../target/debug/commandagent`: `passed`
- `cargo test --test gui_read_only_guard`: `passed`
- `cargo test --test doc_drift`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `cargo test`: `passed`
- `cargo test --features gui`: `passed`
- `git diff --check`: `passed`

## Evidence

- `/private/tmp/commandagent-issue119-session-index-final/session-index-smoke.json`
  reports `ok: true` for `/` and `/proxy/commandagent/`. Each case records
  `runtime_max_concurrent_requests: 1`, `runtime_paused_while_hidden: true`,
  `runtime_resumed_when_visible: true`, stale list retention, focus/visibility
  revalidation, shared ja-JP time labels, Terminal row highlighting, and
  runtime badge navigation.
- `/private/tmp/commandagent-issue119-read-only-final/browser-smoke.json`
  reports `ok: true` for both base paths. Navigation item 04, the run-page
  heading, and the browser title all use **リポジトリ実行記録**; internal links
  remain base-path safe and no unexpected console errors were observed.
- The GUI source guard passed 24 tests. The GUI-feature suite additionally ran
  12 GUI-server unit tests and 26 GUI-server integration tests with no failure.

## Environment note

The sandboxed first browser-smoke attempt could not bind `127.0.0.1`. The
provider-free smokes were rerun with approved loopback permission. During smoke
development, mock handler lifetime initially counted an already-aborted request
across a full page navigation; the final check counts browser request lifecycle
events within the mounted document and passed on both base paths.
