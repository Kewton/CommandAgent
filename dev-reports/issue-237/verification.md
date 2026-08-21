# Issue 237 Verification

- Status: `passed`

## Checks

- `cargo test request_body_ --lib`: `passed`
- `cargo test context_budget_check_exposes_ollama_num_ctx_and_source --lib`: `passed`
- `cargo test --test corpus_regression generated_app_corpus_matches_detector_and_probe_expectations`: `passed`
- `cargo test providers::ollama --lib`: `passed`
- `cargo test doctor::tests --lib`: `passed`
- `cargo test --test doctor_cli`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --cached --check`: `passed`
