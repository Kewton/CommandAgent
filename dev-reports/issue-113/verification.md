# Issue #113 Verification

- Status: `passed`

## Checks

- `cargo test --features gui --test gui_server`: `passed`
- `cargo test --test gui_read_only_guard`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `cargo test`: `passed`
- `cargo test --features gui`: `passed`
- `cd gui && npm ci --include=dev`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run build`: `passed`
- `cd gui && npm run smoke -- --output /tmp/commandagent-issue113-smoke --read-only --commandagent-bin ../target/debug/commandagent`: `passed`

The read-only browser smoke passed at both the root deployment path and the
proxy base path. It observed admitted and unapproved repository source labels,
followed an eligible catalog link, and confirmed that Trial selected the
matching pack without browser console errors.
