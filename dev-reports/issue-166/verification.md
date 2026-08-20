# Issue 166 verification

- Status: `passed`

## Checks

- `cargo test --test gui_read_only_guard --test doc_drift`: `passed`
- `cd gui && node --check scripts/smoke.mjs`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run build`: `passed`
- `cd gui && npm run smoke -- --wizard-only --output ../dev-reports/issue-166/smoke --commandagent-bin ../target/debug/commandagent`: `passed`
- `cargo test --features gui --test gui_server -- --test-threads=1`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`

## Notes

This fresh worktree initially had no `gui/node_modules`, so the first
typecheck stopped at missing lockfile dependencies. `npm ci --include=dev
--offline` restored the pinned packages from the local cache; the exact
typecheck command above then passed, as did lint and the production build.

The first sandboxed smoke attempt built successfully but could not bind its
loopback listener (`Operation not permitted`). The exact smoke command above
passed with localhost/headless-browser permission for both `/` and
`/proxy/commandagent/`, reported no unexpected console errors, and removed its
scratch runtime after success.
