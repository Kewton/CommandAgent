# Issue 122 verification

- Status: `passed`

## Checks

- `cargo test --test doc_drift`: `passed`
- `cargo test --test gui_read_only_guard`: `passed`
- `cd gui && node --check scripts/smoke.mjs`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run build`: `passed`
- `cargo build --release --bin commandagent`: `passed`
- `target/release/commandagent --version`: `passed`
- `cd gui && npm run smoke -- --overview-only --output /tmp/commandagent-issue-122-smoke.LSdMnJ --commandagent-bin ../target/release/commandagent`: `passed`
- `cd gui && npm run smoke -- --read-only --output /tmp/commandagent-issue-122-read-only-smoke.6846Z3 --commandagent-bin ../target/release/commandagent`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `cargo test --features gui`: `passed`

The first sandboxed smoke attempt could not bind its loopback listener. The
same provider-free command was rerun with the approved loopback permission and
passed for both `/` and `/proxy/commandagent/`. The read-only smoke also passed
for both base paths; all eight help-map bindings and the live onboarding,
Gate 1, and extension-action copy checks were green.
