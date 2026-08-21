# Issue 150 verification

- Status: `passed`

## Checks

- `bash -n scripts/setup.sh`: `passed`
- `cargo test --features gui --test gui_server gui_server_init -- --nocapture`: `passed`
- `cargo test --features gui --test gui_server gui_server_check -- --nocapture`: `passed`
- `cargo test --test setup_script gui_mode_builds_private_scaffolding_and_prints_the_init_start_command -- --nocapture`: `passed`
- `cargo test --test gui_read_only_guard gui_server_mutates_only_init_roots_or_through_the_confirmed_cli_delegate -- --nocapture`: `passed`
- `npm ci --include=dev` (from `gui/`): `passed`
- `GUI_BASE_PATH=/ npm run build` (from `gui/`): `passed`
- `cargo test --features gui --test gui_server`: `passed`
- `cargo test --test setup_script`: `passed`
- `uv run --offline --with PyYAML==6.0.3 cargo test --quiet`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `git diff --check`: `passed`

## Evidence

- The focused `--init` target passed both initialization cases: default roots
  were created at exact mode `0700`, preflight completed before the listener
  message, automatic sibling binary discovery passed, and an explicit `0755`
  extension root remained `0755` while reporting the exact `chmod 700` fix.
- The complete GUI server integration target passed 31 tests after the ignored
  root-base static export was generated. Its disposable listeners required
  loopback permission outside the filesystem/network sandbox.
- The complete setup-script target passed 8 tests. Its GUI fixture observed the
  locked npm build, Rust GUI-server build, one `--init` startup command, no
  separately printed `--check` command, private extension scaffolding, and no
  token disclosure.
- The full default Rust suite passed with the repository-pinned PyYAML 6.0.3
  dependency: the library target reported 1,972 passed and 16 intentionally
  ignored tests, and all integration/doc targets passed. The host `python3`
  lacked `yaml`, so the final authoritative run used the cached dependency from
  `requirements/ci.txt` through `uv` rather than skipping the reference-parity
  test.
- `src/bin/gui_server/api.rs`, GUI routes, Trial Origin/authentication code,
  event schemas, corpus fixtures, historical run evidence, and the live
  `.anvil/` namespace were unchanged.
