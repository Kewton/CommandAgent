# Issues 174, 175, 176, 179, 180, 196, 197, 198, and 202 verification

- Status: `passed`

## Checks

- `cargo test --features gui --bin gui_server sessions::tests -- --test-threads=1`: `passed`
- `cargo test --test gui_read_only_guard -- --test-threads=1`: `passed`
- `npm run lint` (from `gui/`): `passed`
- `npm run typecheck` (from `gui/`): `passed`
- `npm run build` (from `gui/`): `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `cargo test --features gui -- --test-threads=1`: `passed`
