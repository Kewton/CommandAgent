# Issues #152 and #149 verification

- Status: `passed`

## Checks

- `cargo test --features gui --test gui_server provider_model_discovery_uses_local_read_only_endpoints_and_degrades_to_empty -- --exact --nocapture`: `passed`
- `cargo test --features gui --test gui_server band_means_expose_catalog_keys_and_only_matching_measurements -- --exact --nocapture`: `passed`
- `cargo test --features gui --test gui_server confirmed_session_delegates_with_cli_event_bytes_unchanged -- --exact --nocapture`: `passed`
- `cargo test --test gui_read_only_guard -- --nocapture`: `passed`
- `npm run build` (from `gui/`): `passed`
- `cargo test --features gui --bin gui_server`: `passed`
- `cargo test --features gui --test gui_server`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

## Environment notes

- The first provider mock attempt was denied by the filesystem/network
  sandbox at loopback bind time. The identical command was rerun outside that
  sandbox and passed.
- The first full GUI-server target found the generated `gui/out/index.html`
  export absent. After an unchanged `npm run build`, the complete target was
  rerun and passed 34/34. The ignored export and dependencies did not change
  tracked GUI sources.
