# Issue 206 verification

- Status: `passed`

## Checks

- `cargo test tools::bash_write_guard::tests --lib`: `passed`
- `cargo test runtime_bash_workspace_policy --lib`: `passed`
- `cargo test --test bash_workspace_confinement`: `passed`
- `cargo test --test corpus_regression`: `passed`
- `cargo test --test generality_guardrails`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
