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
