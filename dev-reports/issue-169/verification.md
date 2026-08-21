# Issue #169 Verification

- Status: `passed`

## Checks

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
- `cargo test --features gui --test gui_read_only_guard --test gui_server`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `cargo test`: `passed`
- `cargo test --features gui`: `passed`
- `git diff --check`: `passed`

## Evidence

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
