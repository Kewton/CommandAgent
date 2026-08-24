# Issue 162 verification

- Status: `passed`

## Checks

- `cargo build --release --bin commandagent`: `passed`
- `cd gui && node --check scripts/smoke.mjs`: `passed`
- `cd gui && node --check scripts/session-index-smoke.mjs`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run smoke:session-index -- --output /private/tmp/commandagent-issue162-followup-session-index`: `passed`
- `cd gui && npm run smoke -- --output /private/tmp/commandagent-issue162-followup-full-passed --commandagent-bin ../target/release/commandagent --model qwen3:8b`: `passed`
- `cd gui && npm run smoke -- --output /private/tmp/commandagent-issue162-followup-feedback --feedback-only --commandagent-bin ../target/debug/commandagent`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `cargo test`: `passed`
- `cargo test --features gui`: `passed`
- `cargo +1.97.1 clippy --features gui --bin gui_server -- -D warnings`: `passed`
- `cargo test --features gui --test gui_server trial_session_files_`: `passed`
- `cargo test --features gui --test gui_server confirmed_session_delegates_with_cli_event_bytes_unchanged`: `passed`
- `cargo test --test gui_read_only_guard trial_status_polling_revalidates_with_durable_timing_metadata`: `passed`
- `git diff --check`: `passed`

## Evidence

- The freshly rebuilt pre-fix reproduction at
  `/private/tmp/commandagent-issue162-followup-repro/browser-smoke.json` exited
  nonzero at the same 30-second disabled-button click as the Issue 158 evidence.
  Its stack points to the first click after the wrong-token fill, proving the
  session-index 401 cleared the field before an explicit reconnect dispatched.
- `/private/tmp/commandagent-issue162-followup-session-index/session-index-smoke.json`
  reports overall `ok: true` for `/` and `/proxy/commandagent/`. Both cases
  report `rejected_token_removed: true`, `retry_button_enabled: true`, and
  `reconnect_get_only: true`.
- `/private/tmp/commandagent-issue162-followup-full-passed/browser-smoke.json`
  reports overall and per-case `ok: true`. Both reconnect traces contain the
  expected direct-session HTTP 401 followed by HTTP 200, retain the GET-only
  contract, and preserve tab-scoped token storage.
- The same full report records
  `elapsed_preserved_after_reconnect: true`,
  `mean_preserved_after_reconnect: true`, and
  `new_run_identity_editable: true` for both base paths.
- `/private/tmp/commandagent-issue162-followup-feedback/browser-smoke.json`
  independently reports elapsed-time and measured-mean preservation with
  overall `ok: true` for both base paths.
- The first post-fix full run passed every authentication and Issue 162 timing
  assertion but exposed the stale six-control lifecycle cardinality. The
  cardinality was strengthened to cover all seven current identity controls;
  the full two-base-path run was then repeated from start and passed.

## CI follow-up evidence

- Code commit `714017ca` was cherry-picked alone as `b4221a12`; its commit
  contains only `src/bin/gui_server/session_files.rs`. No Issue 160 report file
  is present in the cherry-pick or this report update.
- Rust 1.97.1 Clippy passed with `-D warnings` and no lint allowance.
- The two `trial_session_files_` integration tests passed, covering authenticated
  confined/bounded file responses and rejection of a symlinked runtime root.
- The confirmed-session integration test passed with its exact event bytes and
  durable start/average response assertions intact.
- The focused GUI source contract for durable reconnect timing metadata passed.

## Environment note

Browser smokes required the existing local-loopback/Chromium permission. No
timeout was extended, no click was forced, and no auth, reconnect, root/proxy,
terminal, elapsed-time, or measured-mean assertion was removed or weakened.
