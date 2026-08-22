# Issue #221 verification

- Status: `passed`

## Checks

- `cargo test --lib cli_exit_code_maps_nested_interruption_to_130`: `passed`
- `cargo test --lib interrupted_headless_summary_requires_evidence_and_projects_exit_130`: `passed`
- `cargo test --lib headless_summary`: `passed`
- `cargo test --test headless_summary`: `passed`
- `cargo test --test doc_drift`: `passed`
- `cargo test --test corpus_regression generated_app_corpus_matches_detector_and_probe_expectations`: `passed`
- `cargo test --test protection_coverage_audit`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`

## Notes

The headless integration and final full-suite commands ran outside the
filesystem/network sandbox because the SIGINT regression binds a loopback
listener and signals its child process. The final full-suite run included the
terminal protection audit and completed with no failures.

## Post-base-sync recovery

- Merged `origin/develop` after #227 and the earlier Wave 4 prerequisites
  landed. The `src/lib.rs` resolution retains both the Git run reporter and the
  interruption-aware summary source.
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed` with the existing pyenv shims prepended to `PATH` so
  the Python reference checks use the repository's available PyYAML runtime.
