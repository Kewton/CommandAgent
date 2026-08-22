# Issue #220 verification

- Status: `passed`

## Checks

- `cargo test --test issue218_220_cli_errors`: `passed`
- `cargo test --test pack_actions`: `passed`
- `cargo test --lib planner::pack`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
