# Issue 246 verification

- Status: `passed`

## Checks

- `cargo test --test pack_actions packs_warns_for_invalid_candidates_and_keeps_listing_valid_local_packs`: `passed`
- `cargo test --test pack_actions`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`
