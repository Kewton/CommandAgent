# Issue #362 verification

- Status: `passed`

## Checks

- `cargo test planner::completion_contract_path::tests --lib`: `passed`
- `cargo test isolated_plan_run_reaches_contract_verification_instead_of_rejecting_its_path --lib`: `passed`
- `cargo test isolated_ultra_run_binds_generated_completion_contract_inside_its_workspace --lib`: `passed`
- `cargo test --features gui --test gui_server confirmed_session_delegates_with_cli_event_bytes_unchanged -- --exact`: `passed`
- `cargo test --features gui --test gui_server`: `passed`
- `cargo test --test generality_guardrails runner_test_modules_do_not_grow_past_transferred_budget -- --exact`: `passed`
- `cargo test --test doc_drift gui -- --nocapture`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `cargo test --features gui`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run build`: `passed`
- `cd gui && npm run smoke -- --output /tmp/commandagent-issue-362-gui-smoke --provider-only`: `passed`
- `git diff --check`: `passed`

The loopback GUI integration tests and provider-only browser smoke were run
outside the filesystem/network sandbox. The smoke report completed with
`"ok": true` for both `/` and `/proxy/commandagent/`; its temporary output is
not part of the commit. Expected ignored Rust tests remained ignored.
