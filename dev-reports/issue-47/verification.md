# Issue 47 verification

- Status: `passed`

## Checks

- `cargo test --lib tui::terminal_notifications`: `passed`
- `cargo test --lib tui::status_bus`: `passed`
- `cargo test --test doc_drift`: `passed`
- `ANVIL_PTY_TESTS=1 cargo test --test tui_pty tui_pty_screen_state_preserves_long_accepted_goal_across_footer_modes -- --ignored --exact --nocapture`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

## Notes

- The focused PTY command initially received `PermissionDenied` in the sandbox.
  The identical command passed after it was allowed to create a pseudo-terminal
  and loopback fake-provider socket.
- The first full parallel `cargo test` attempt hit a pre-existing planner-test
  port collision (`port_in_use`). The exact failing test passed alone, and a
  second unchanged `cargo test` run passed the complete suite with 1,563 library
  tests passed and 15 ignored, followed by every integration and documentation
  test binary.
