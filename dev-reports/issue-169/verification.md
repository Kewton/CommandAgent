# Issue #169 Verification

- Status: `passed`

## Checks

- `git range-diff 7b0d47ca4dae2db2bab50034a3b5a6026990d07a..714017cacef2728ac9276e920b561612e2609464 7275b7fb..5925b8ec`: `passed`
- `git range-diff 551fa209..ea8f8fbdc0d0a7fc9e23cdff38fa30b874e95e6d 0ca9c5cb..a37495fd`: `passed`
- `cargo build --release --bin commandagent`: `passed`
- `cd gui && node --check scripts/smoke.mjs`: `passed`
- `cd gui && node --check scripts/session-index-smoke.mjs`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run build`: `passed`
- `cd gui && npm run smoke:session-index -- --output /private/tmp/commandagent-issue169-followup-session-index`: `passed`
- `cd gui && npm run smoke -- --output /private/tmp/commandagent-issue169-followup-feedback --feedback-only --commandagent-bin ../target/debug/commandagent`: `passed`
- `cd gui && npm run smoke -- --output /private/tmp/commandagent-issue169-followup-full --commandagent-bin ../target/release/commandagent --model qwen3:8b`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo +1.97.1 clippy --features gui --bin gui_server -- -D warnings`: `passed`
- `cargo test --features gui --test gui_server trial_session_files_`: `passed`
- `cargo test --features gui --test gui_server`: `passed`
- `cargo test --test gui_read_only_guard trial_status_polling_revalidates_with_durable_timing_metadata`: `passed`
- `cargo test --features gui --test gui_read_only_guard --test gui_server`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `cargo test`: `passed`
- `cargo test --features gui`: `passed`
- `git diff --check`: `passed`

## Evidence

- `git range-diff` reports
  `714017ca = 5925b8ec Box GUI session file errors`, proving exact code-patch
  equality after cherry-pick. The Issue #160 report-path diff from `7275b7fb`
  through `5925b8ec` is empty, so report commit `1f28c021` and Issue #160 report
  files were not propagated.
- Rust 1.97.1 Clippy passes with `-D warnings` and no lint allow. The two
  `trial_session_files_` regressions preserve authentication, response status,
  headers and coded JSON bytes, bounded path confinement, and symlink
  rejection. The complete 26-test `gui_server` target also passes, including
  `confirmed_session_delegates_with_cli_event_bytes_unchanged`, which retains
  #169's exact goal, profile, model-pin, and pack response assertions.
- `git range-diff` reports
  `ea8f8fbd = a37495fd Fix GUI Trial auth retry race for Issue 162`, proving
  patch equality after cherry-pick.
- `/private/tmp/commandagent-issue169-followup-session-index/session-index-smoke.json`
  records top-level and per-case `ok: true` for `/` and
  `/proxy/commandagent/`. Both cases record `rejected_token_removed: true`,
  `retry_button_enabled: true`, and `reconnect_get_only: true`.
- `/private/tmp/commandagent-issue169-followup-feedback/browser-smoke.json`
  records top-level and per-case `ok: true`. Gate 2, reconnect, and terminal
  each show `Synthetic Gate 2 feedback probe`, `python-cli`,
  `cli-assist@1.0.0`, and both `ollama / synthetic-model` pins. Both cases also
  preserve elapsed time and measured mean after reconnect.
- `/private/tmp/commandagent-issue169-followup-full/browser-smoke.json`
  records top-level and per-case `ok: true` for the rebuilt release binary.
  Both cases retain locked Gate 2/terminal identities, an editable seven-field
  new-run identity, GET-only reconnect, rejected-token removal, tab-scoped token
  storage, and the same exact #169 identity values across Gate 2, reconnect,
  and terminal.

## Environment note

Browser smokes used the existing local-loopback/Chromium permission and the
locally available `qwen3:8b` model. No timeout, click behavior, auth gate,
reconnect gate, root/proxy case, identity assertion, or Rust check was removed,
forced, skipped, weakened, or extended.
