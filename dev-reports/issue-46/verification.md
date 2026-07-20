# Issue 46 verification

- Status: `passed`

## Checks

- `cargo test providers::`: `passed`
- `cargo test missing_api_key_error_includes_setup_and_doctor_remediation`: `passed`
- `cargo test banner_legacy_has_dynamic_lines_without_art`: `passed`
- `cargo test scripted_demo_contains_full_visual_journey`: `passed`
- `cargo test --test provider_onboarding`: `passed`
- `ANVIL_PTY_TESTS=1 cargo test --test tui_pty -- --include-ignored`: `passed`
- `xmllint --noout docs/assets/ux-demo.svg`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test -q -- --test-threads=1`: `passed`

## Notes

- The PTY commands required access to a pseudo-terminal and loopback sockets, so
  they were rerun with the required sandbox permission after the sandboxed
  launcher returned `PermissionDenied`.
- During verification, two unrelated pre-existing tests failed transiently: the
  shared terminal-capture unit test observed another test's output in a
  default-parallel run, and the planner budget-exhaustion test unexpectedly
  succeeded once in a serialized run. Each exact test passed in isolation, and
  the final complete serialized suite passed with 1,553 library tests passed and
  15 ignored, followed by all integration and documentation test binaries.
