# Issue 285 verification

- Status: `passed`

## Checks

- `cargo test --lib completion_metadata::cli::tests -- --nocapture`: `passed`
- `cargo test --lib planner::runner::acceptance::plan_final_probe::tests:: -- --nocapture`: `passed`
- `cargo test --lib planner::runner::acceptance::plan_final_probe::tests::python_cli_src_package_plan_run_binds_full_probe_to_terminal_summaries -- --exact --nocapture`: `passed`
- `cargo test --test cli_profile_conformance`: `passed`
- `cargo test --test corpus_regression generated_app_corpus_matches_detector_and_probe_expectations -- --exact --nocapture`: `passed`
- `cargo test --test generality_guardrails`: `passed`
- `cargo test --test protection_coverage_audit`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

## Notes

The exact focused UAT regression passed with one test and verifies the ordinary
`src/anvil_app/main.py` layout end to end: canonical
`evidence/cli-assurance.json` is absent; fallback
`.anvil/evidence/python-cli-behavior.json` reports `status: pass`, `ok: true`,
and `changed_by_input: true`; the plan-final corpus binds full assurance and
the fallback path; and `tui_command_stop`, `run_stop`, `summary.md`, and
headless summary remain full without `cli_probe_not_run`.

The eight completion-metadata tests also passed the failed, missing, malformed,
unexecuted, wrong-path, and failed-current-gate non-elevation matrix. The full
suite covers the updated corpus fixture and both runner growth and
protected-execution audits. Live provider and PTY-only ignored tests remained
ignored by the repository's default `cargo test` contract. No push, PR,
merge, or external orchestration action was performed.
