# Issue 243 Verification

- Status: `passed`

## Checks

- `cargo test time_profile::tests`: `passed`
- `cargo test headless_summary::tests`: `passed`
- `cargo test tui_summary_renders_time_profile_from_existing_events`: `passed`
- `cargo test --test headless_summary`: `passed`
- `cargo test --test corpus_regression generated_app_corpus_matches_detector_and_probe_expectations`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
