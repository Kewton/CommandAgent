# Issue 213 verification

- Status: `passed`

## Checks

- `cargo test --test doctor_cli doctor_preserves_other_preset_validation_error_without_reporting_selected_not_found`: `passed`
- `cargo test --test doctor_cli doctor_reports_malformed_toml_as_syntax_error_instead_of_unknown_key`: `passed`
- `cargo test config --lib`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

The full suite completed with 2,005 library tests passed and 16 ignored, all
integration suites passed, and 2 doctests passed.
