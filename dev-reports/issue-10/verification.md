# Issue #10 Verification

- Status: `passed`
- `cargo test tui::editor::tests --lib`: `passed`
- `cargo test tui::slash::tests --lib`: `passed`
- `cargo test minimal_loop::build_verifier::tests::nextjs_build_with_foreign_next_on_path_is_dependency_missing_and_emits_event -- --exact`: `passed`
- `cargo test tools::registry::tests::bash_keeps_outside_root_cd_rejected_with_relative_retry_guidance -- --exact`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `ANVIL_PTY_TESTS=1 cargo test --test tui_pty -- --ignored`: `passed`
