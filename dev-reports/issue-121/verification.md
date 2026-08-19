# Issue 121 verification

- Status: `passed`

## Checks

- `bash -n scripts/setup.sh`: `passed`
- `cargo test --test setup_script`: `passed`
- `cargo test --features gui --test gui_server`: `passed`
- `cargo test --test gui_read_only_guard`: `passed`
- `cargo test --test protection_coverage_audit`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `cargo test --features gui`: `passed`
- `cd gui && npm ci --include=dev`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && GUI_BASE_PATH=/proxy/commandagent/ npm run build`: `passed`
- `GUI_TRIAL_TOKEN="$(</private/tmp/commandagent-issue121-extension.lc9Bbc/gui-token)" target/debug/gui_server --check --base-path /proxy/commandagent --static-dir gui/out --repository-root . --extension-root /private/tmp/commandagent-issue121-extension.lc9Bbc --trial-token-auth on --commandagent-bin target/debug/commandagent`: `passed`

The focused GUI process tests independently cover base-path mismatch, root
overlap, missing binary, and invalid token `ng` results. The full GUI-feature
suite was run with localhost binding permitted. The acceptance reference to the
Japanese `はじめに` card belongs to still-open Issue 120 and was not duplicated
in this setup-focused change.
