# Verification: Issues #210, #222, and #209

- Status: `passed`

## Checks

- `cargo test --lib tui::`: `passed`
- `COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty tui_pty_planner_stream_interrupt_cleans_spinner_footer_and_status -- --ignored --nocapture`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
