# Issue #364 verification

- Status: `passed`
- `cargo test --features gui --bin gui_server session_diagnostics::tests -- --nocapture`: `passed`
- `cargo test --features gui --bin gui_server public_projection::tests -- --nocapture`: `passed`
- `cargo test --features gui --test gui_server confirmed_session_delegates_with_cli_event_bytes_unchanged -- --exact`: `passed`
- `cargo test --features gui --test gui_server trial_session_files_are_authenticated_confined_and_bounded -- --exact`: `passed`
- `cargo test --test corpus_regression generated_app_corpus_matches_detector_and_probe_expectations -- --exact`: `passed`
- `cargo test --test doc_drift gui -- --nocapture`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo test --features gui`: `passed`
- `cargo test`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run build`: `passed`
- `cargo build --release --bin commandagent`: `passed`
- `cd gui && npm run smoke -- --output /tmp/commandagent-issue-364-feedback-smoke-final --feedback-only`: `passed`
- `cd gui && npm run smoke -- --output /tmp/commandagent-issue-364-full-smoke-final --commandagent-bin ../target/release/commandagent --model qwen3:8b`: `passed`

Both smoke commands returned `ok: true` for `/` and
`/proxy/commandagent/`. The final full-smoke report also recorded
`execution_root_hidden: true`, `execution_root_placeholder_visible: true`,
`gate_3_projection_ok: true`, and `gate_3_failure_count: 0` for both cases.
