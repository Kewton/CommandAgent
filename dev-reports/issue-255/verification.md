# Verification: Issue #255

- Status: `passed`
- `cargo test --lib preset`: `passed`
- `cargo test --lib runs_`: `passed`
- `cargo test --lib trace_is_opt_in_and_scrubs_prompt_reply_and_home_paths`: `passed`
- `cargo test --lib provider_call::tests::trace_flag_records_the_shared_provider_chokepoint`: `passed`
- `cargo test --lib state::tests`: `passed`
- `cargo test --lib tui::history::tests`: `passed`
- `cargo test --lib bash_redacts_engine_metadata_from_workspace_root_ls`: `passed`
- `cargo test --test corpus_regression`: `passed`
- `cargo test --test doc_drift`: `passed`
- `cargo test --test generality_guardrails`: `passed`
- `cargo test --test gui_read_only_guard`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
