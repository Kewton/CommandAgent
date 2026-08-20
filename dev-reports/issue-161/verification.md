# Issue 161 verification

- Status: `passed`

## Checks

- `cargo test --features gui --bin gui_server pack_catalog::tests::repository_catalog_skips_builtin_namespace_and_does_not_repeat_warnings`: `passed`
- `cargo test --test gui_read_only_guard gui_server_can_execute_only_through_the_confirmed_cli_delegate`: `passed`
- `cargo test --features gui --test gui_server extension_catalog_classifies_supply_and_warns_on_stale_local_pins`: `passed`
- `cd gui && npm run build`: `passed`
- `cargo test --features gui --test gui_server`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `cargo test`: `passed`

## Notes

The first full GUI-target attempt found the ignored `gui/out` export absent,
as documented by predecessor Issue 160. `npm ci --include=dev` restored the
lockfile dependencies and the recorded build/check reruns passed.

An intermediate full-suite run correctly rejected the unit fixture's initial
use of a filesystem-creation token forbidden anywhere in GUI-server source.
The fixture now uses `DirBuilder`, the focused guard passed, and the final full
suite passed. One focused integration attempt also encountered the sandbox's
localhost bind restriction; the same command passed with the repository test
permission used for the final GUI target.
