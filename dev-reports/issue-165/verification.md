# Issue 165 verification

- Status: `passed`

## Checks

- `cargo test --test gui_read_only_guard --test doc_drift`: `passed`
- `cd gui && node --check scripts/smoke.mjs`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run build`: `passed`
- `cd gui && npm run smoke -- --wizard-only --output ../dev-reports/issue-165/smoke --commandagent-bin ../target/debug/commandagent`: `passed`
- `cargo test --features gui --test gui_server -- --test-threads=1`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`

## Notes

The sandboxed smoke attempt built successfully but could not bind its loopback
listener. The same provider-free command passed with loopback/browser
permission for both supported base paths and removed its scratch runtime.

The first cold full-suite attempt hit three unrelated timeout-sensitive process
harness tests. Each failed test passed immediately in isolation, and the
subsequent exact `cargo test` command completed with no failures.
