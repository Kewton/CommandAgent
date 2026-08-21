# Issue 156 verification

- Status: `passed`

## Checks

- `git merge-base --is-ancestor 551fa209 HEAD`: `passed`
- `git merge-base --is-ancestor 0ca9c5cb HEAD`: `passed`
- `git merge-base --is-ancestor 5239f9b9 HEAD`: `passed`
- `cd gui && npm ci --include=dev --offline`: `passed`
- `cd gui && node --check scripts/smoke.mjs`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run build`: `passed`
- `cargo test --test gui_read_only_guard trial_ui_keeps_gate_one_confirmation_and_has_no_intervention_surface`: `passed`
- `cargo test --features gui --test gui_server confirmed_session_delegates_with_cli_event_bytes_unchanged`: `passed`
- `cd gui && npm run smoke:provider -- --output /private/tmp/commandagent-issue156-provider-smoke-final`: `passed`
- `cargo test --test gui_read_only_guard`: `passed`
- `cargo test --features gui --test gui_server`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `cargo test`: `passed`
- `cargo test --features gui`: `passed`
- `git diff --check`: `passed`
- `git diff --check`: `passed`

## Evidence

- `/private/tmp/commandagent-issue156-provider-smoke-final/browser-smoke.json`
  reports top-level and per-case `ok: true` for `/` and
  `/proxy/commandagent/`.
- In both base-path cases, the OpenAI and Gemini rows record matching values
  for `request_provider`, `request_planner_provider`, `cli_provider`, and
  `cli_planner_provider`. Each row also preserves its distinct executor and
  planner model ID, and the frozen run identity displays the same pins.
- The smoke used the local argv-recording probe and the established terminal
  event fixture; it made no provider call and required no OpenAI/Gemini
  credential.

## Environment notes

- The initial GUI typecheck was run before `node_modules` existed and could not
  resolve the lockfile dependencies. The offline `npm ci` prerequisite then
  passed, and typecheck, lint, and build all passed on the installed tree.
- The first browser-smoke attempt was denied local loopback binding by the
  sandbox. The approved loopback rerun exposed an incomplete synthetic terminal
  event; the probe was corrected to reuse `tests/fixtures/gui_cli_events.jsonl`,
  and the final approved smoke command above passed without weakening any
  assertion.

## Follow-up combined-tree checks

- `git cherry HEAD a37495fd`: `passed`
- `cargo build --release --bin commandagent`: `passed`
- `target/release/commandagent --version`: `passed`
- `cd gui && node --check scripts/smoke.mjs`: `passed`
- `cd gui && node --check scripts/session-index-smoke.mjs`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run smoke:session-index -- --output /private/tmp/commandagent-issue156-followup-session-index-approved`: `passed`
- `cd gui && npm run smoke:provider -- --output /private/tmp/commandagent-issue156-followup-provider`: `passed`
- `cd gui && npm run smoke -- --output /private/tmp/commandagent-issue156-followup-feedback-identity --feedback-only --commandagent-bin ../target/release/commandagent`: `passed`
- `cd gui && npm run smoke -- --output /private/tmp/commandagent-issue156-followup-full --commandagent-bin ../target/release/commandagent --model qwen3:8b`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `cargo test`: `passed`
- `cargo test --features gui`: `passed`

## Follow-up combined-tree evidence

- The rebuilt release candidate reported
  `commandagent 0.1.0 5d6773f9 2026-08-21T09:52:20+09:00` before any browser
  smoke was run against it.
- `/private/tmp/commandagent-issue156-followup-session-index-approved/session-index-smoke.json`
  reports root and `/proxy/commandagent/` cases `ok: true`, with rejected-token
  removal, an enabled retry action, and GET-only reconnect traffic in both.
- `/private/tmp/commandagent-issue156-followup-provider/browser-smoke.json`
  reports both base paths `ok: true`. OpenAI and Gemini each retain matching
  request and CLI executor/planner providers while preserving their distinct
  executor and planner model IDs.
- `/private/tmp/commandagent-issue156-followup-feedback-identity/browser-smoke.json`
  reports both base paths `ok: true`, including elapsed-time and measured-mean
  persistence plus exact Gate 2, reconnected, and terminal identities.
- `/private/tmp/commandagent-issue156-followup-full/browser-smoke.json` reports
  top-level, root, and proxy `ok: true`. The unchanged full smoke observed the
  rejected reconnect request followed by a successful authenticated retry,
  GET-only session traffic, frozen run identities, seven editable next-run
  controls, lease protection, and terminal completion in both base paths.
- `git cherry HEAD a37495fd` reports `- a37495fd...`, confirming the source
  follow-up patch is present patch-equivalently as `5d6773f9`.

The first follow-up session-index smoke attempt was denied loopback access by
the sandbox before exercising the application. The approved rerun shown above
passed without changing the smoke, its assertions, or the application.
