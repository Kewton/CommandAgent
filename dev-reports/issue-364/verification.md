# Issue #364 verification

- Status: `passed`
- `cargo test --features gui --bin gui_server -- --nocapture`: `passed`
- `cargo test --features gui --test gui_server confirmed_session_delegates_with_cli_event_bytes_unchanged -- --exact --nocapture`: `passed`
- `cargo test --test corpus_regression generated_app_corpus_matches_detector_and_probe_expectations -- --exact`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo test --features gui`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run build`: `passed`
- `node --check gui/scripts/smoke.mjs`: `passed`
- `cargo build --release --bin commandagent`: `passed`
- `cd gui && npm run smoke -- --output /tmp/commandagent-issue-364-followup-feedback-smoke-final --feedback-only`: `passed`
- `cd gui && npm run smoke -- --output /tmp/commandagent-issue-364-followup-full-smoke --commandagent-bin ../target/release/commandagent --model qwen3:8b`: `passed`

Both smoke commands returned `ok: true` for `/` and
`/proxy/commandagent/`. Both reports recorded `gate_3_projection_ok: true`,
`gate_3_failure_count: 0`, `gate_4_ready_projection_ok: true`, and
`gate_4_ready_failure_count: 0` for both cases. The provider-backed full smoke
used Ollama `qwen3:8b` against the final release binary.
