# Issue 398 verification

- Status: `passed`

## Checks

- `cargo test --test critic_shadow_conformance`: `passed`
- `cargo test --test verification_spec_v0`: `passed`
- `cargo test --test create_shadow_oracle --test fix_shadow_conformance --test investigate_shadow_conformance`: `passed`
- `cargo test --test corpus_regression generated_app_corpus_matches_detector_and_probe_expectations`: `passed`
- `cargo test --test generality_guardrails`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`

## Notes

The first sandboxed full-suite attempt encountered loopback/process-permission
failures in unrelated local-provider and browser tests and was interrupted
after a planner integration test stalled. The required `cargo test` check was
then rerun outside the sandbox, where the affected tests and the entire suite
passed. No live provider probe was required or performed.
