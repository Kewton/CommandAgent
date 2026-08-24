# Issue #369 verification

- Status: `passed`

## Checks

- `cargo test --test gui_read_only_guard`: `passed`
- `cd gui && node --check scripts/smoke.mjs`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run build`: `passed`
- `cd gui && npm run smoke -- --overview-only --output /tmp/commandagent-issue-369-smoke --commandagent-bin ../target/debug/commandagent`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --cached --check`: `passed`

The browser smoke passed for both `/` and `/proxy/commandagent/`. In each case,
the desktop 1200 px and mobile 390 px role-layout probes passed with the Tab
order `trial-provider`, `trial-executor-model`, `trial-planner-provider`,
`trial-planner-model`.
