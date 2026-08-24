# Issue 120 verification

- Status: `passed`

## Checks

- `cargo test --features gui --test gui_server -- --test-threads=1`: `passed`
- `cargo test --test gui_read_only_guard`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run build`: `passed`
- `cd gui && npm run smoke -- --overview-only --output ../dev-reports/issue-120/smoke`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`

The smoke report records `ok: true` for both `/` and
`/proxy/commandagent/`. Each case observed three runtime prerequisite rows, the
Python CLI sample goal/profile/pack preset, the Gate 1 primer, and persistent
same-tab dismissal with no unexpected browser console errors.
