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

## CI follow-up checks

- `cargo fmt --all -- --check`: `passed`
- `cargo +1.97.1 clippy --features gui --bin gui_server -- -D warnings`: `passed`
- `cargo test --features gui --test gui_server trial_session_files_`: `passed`
- `cargo test --test gui_read_only_guard extension_pack_wizard_delegates_lifecycle_and_keeps_failures_actionable`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && node --check scripts/smoke.mjs`: `passed`
- `cd gui && npm run smoke -- --wizard-only --output /private/tmp/commandagent-issue-165-ci-follow-up-smoke --commandagent-bin ../target/debug/commandagent`: `passed`

The two `trial_session_files_` cases preserve the authenticated status/header/
JSON response contract, bounded event-tail behavior, path confinement, and
symlink rejection. The follow-up browser report was written outside the
worktree; both `/` and `/proxy/commandagent/` cases passed with no unexpected
console errors and removed their scratch runtime.
