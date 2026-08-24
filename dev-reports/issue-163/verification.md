# Issues 163 and 170 verification

- Status: `passed`

## Checks

- `cargo test --features gui --test gui_server recovery_required_lease_is_exposed_by_an_authenticated_get`: `passed`
- `cargo test --features gui --test gui_server malformed_session_events_return_a_dedicated_error_code`: `passed`
- `cd gui && npm ci --include=dev`: `passed`
- `cd gui && npm run build`: `passed`
- `cargo test --features gui --test gui_server`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`

## Notes

The first full GUI server test attempt found the existing dashboard fixture
absent (`gui/out/index.html`), causing its unrelated dashboard GET assertion to
return 404. `NODE_ENV=production` also required `npm ci --include=dev` so the
existing TypeScript build dependencies were installed. After generating the
ignored static export with the successful build recorded above, the exact full
GUI server target passed all 32 tests. No generated GUI output is staged.
