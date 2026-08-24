# Issue #147 verification

- Status: `passed`

## Checks

- `cargo test --lib first_run_guidance_copy_snapshot`: `passed`
- `cargo test --lib banner_legacy_has_dynamic_lines_without_art`: `passed`
- `cargo test --lib help_lists_discovery_commands_and_interrupt_semantics`: `passed`
- `cargo test --test tui_integration tui_help_lists_recovery_commands_without_emitting_events`: `passed`
- `cargo test --test doc_drift`: `passed`
- `COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty tui_pty_smoke -- --include-ignored`: `passed`
- `cargo test --lib scripted_demo_contains_full_visual_journey`: `passed`
- `cargo test --lib slash_registry_is_the_help_and_alias_source`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

## Notes

The first PTY attempt sent both input lines before Rustyline was ready and did
not exercise the guard. The PTY driver was corrected to wait for startup and
send `/plan-run` and `/exit` separately; the recorded command above then passed
and observed both exact guidance strings.
