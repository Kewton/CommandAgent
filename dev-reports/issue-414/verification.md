# Issue #414 verification

- Status: `passed`

## Checks

- `cargo test eval_events::recovery_resolution::tests --lib`: `passed`
- `cargo test rejected_treatment --lib`: `passed`
- `cargo test tui_command_stop_keeps_control_recovery_after_treatment_rejection --lib`: `passed`
- `cargo test directive_continuation_uses_retained_control_recovery_plan --lib`: `passed`
- `cargo test --test corpus_regression generated_app_corpus_matches_detector_and_probe_expectations`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
