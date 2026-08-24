# Issue #375 verification

- Status: `passed`

## CI toolchain fix

GitHub core job `97545304633` and acceptance job `97545299627` reported Rust 1.98
Clippy's `chunks_exact_to_as_chunks` diagnostic for the Issue #375 pairing test. The test now uses
`as_chunks::<2>().0`, preserving the same two-event grouping without an allow attribute or contract
change. Local verification used `rustc 1.94.0 (4a4ef493e 2026-03-02)`; the Rust 1.98 diagnostic is
removed structurally because the flagged `chunks_exact(2)` call no longer exists.

## Checks

- `cargo test plan_step_events --lib`: `passed`
- `cargo test verify_step_short_circuits_when_expected_path_and_verify_already_pass --lib`: `passed`
- `cargo test step_repair_unrelated_change_is_telemetry_and_handoff_saved --lib`: `passed`
- `cargo test generated_app_corpus_matches_detector_and_probe_expectations --test corpus_regression`: `passed`
- `cargo test runner_chokepoints_do_not_grow_past_interim_budget --test generality_guardrails -- --exact`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
