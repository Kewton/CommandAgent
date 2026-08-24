# Issue #367 verification

- Status: `passed`

## Checks

- `cargo test --lib tui::boundary_shell::route::tests::explicit_intent_is_not_reinferred_from_conflicting_request_words -- --exact`: `passed`
- `cargo test --features gui --test gui_server typed_trial_intents_are_validated_frozen_and_delegated -- --exact`: `passed`
- `cargo test --features gui --test gui_server`: `passed`
- `cargo test --test gui_read_only_guard`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && node --check scripts/smoke.mjs`: `passed`
- `cd gui && npm run smoke -- --overview-only --output /tmp/commandagent-issue-367-smoke --commandagent-bin ../target/debug/commandagent`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --cached --check`: `passed`

The Playwright overview smoke completed successfully for both the root base path
and `/proxy/commandagent/`.
