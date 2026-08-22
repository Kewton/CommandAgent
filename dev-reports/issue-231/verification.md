# Issues #231 and #151 verification

- Status: `passed`
- `cargo test tui::history::tests`: `passed`
- `cargo test tui::editor::tests`: `passed`
- `cargo test tui::repl::tests`: `passed`
- `cargo test tui::slash::tests`: `passed`
- `cargo test --test tui_repl`: `passed`
- `cargo test --test doc_drift`: `passed`
- `cargo test --test corpus_regression`: `passed`
- `COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty tui_pty_scopes_history_and_supports_session_controls -- --ignored --nocapture`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test --quiet`: `passed`

The PTY scenario starts with separate legacy and workspace-A history entries,
then enters `/h` in workspace B before creating any B-local matching entry. It
therefore exercises the non-leak boundary independently of the leaf-helper unit
tests while also covering the new persistent session controls.
