# Issue 115 verification

- Status: `passed`

## Checks

- `cargo test --test gui_read_only_guard --test doc_drift`: `passed`
- `cargo test --features gui --test gui_server -- --test-threads=1`: `passed`
- `cd gui && node --check scripts/smoke.mjs`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run build`: `passed`
- `cd gui && npm run smoke -- --wizard-only --output ../dev-reports/issue-115/smoke --commandagent-bin ../target/debug/commandagent`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`

The first sandboxed browser-smoke attempt could not bind its loopback listener.
The same provider-free command was rerun with loopback permission and passed
for both supported base paths. Each case observed one intentional 422 for the
invalid YAML and no unexpected console errors; the resulting report has
`ok: true` and records scratch cleanup after success.
