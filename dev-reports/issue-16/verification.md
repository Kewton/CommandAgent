# Issue #16 Verification

- Status: `passed`

## Checks

- `cargo test env_compat --lib`: `passed`
- `cargo test config_path_precedence_covers_new_old_and_both --lib`: `passed`
- `cargo test extensionless_commandagent_config_precedes_legacy_config --lib`: `passed`
- `cargo test config::tests --lib`: `passed`
- `cargo test bounded_process::tests --lib`: `passed`
- `cargo test tui:: --lib`: `passed`
- `ruff check scripts/env_compat.py scripts/eval-run.py scripts/eval_lib/artifacts.py tests/eval/test_env_compat.py`: `passed`
- `python3 -m pytest tests/eval/test_env_compat.py tests/eval/test_eval_run_dry.py -q`: `passed`
- `bash -n scripts/env_compat.sh scripts/bench.sh`: `passed`
- `bash -c 'source scripts/env_compat.sh; export ANVIL_TEST_VALUE=legacy; commandagent_env_get first COMMANDAGENT_TEST_VALUE missing; commandagent_env_get second COMMANDAGENT_TEST_VALUE missing; test "$first" = legacy; test "$second" = legacy; test "$_commandagent_warned_legacy_names" = "|ANVIL_TEST_VALUE|"'`: `passed`
- `rg -n 'ANVIL_' src build.rs scripts tests eval docs README.md`: `passed`
- `git diff --check`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo build && cargo test --quiet`: `passed`

## Notes

- The acceptance scan reports legacy names only in the Rust, Python, and Bash
  compatibility helpers and their fallback-behavior tests.
- The first sandboxed full-suite attempt encountered the repository's known
  loopback/process-group permission failures and was interrupted after those
  failures left long timeout cases running. The exact required command was
  rerun outside the sandbox on the unchanged implementation and passed: 1,521
  library tests passed, 15 were intentionally ignored, and all integration
  suites passed.
