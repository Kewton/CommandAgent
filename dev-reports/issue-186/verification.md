# Issues #186, #194, and #199 Verification

- Status: `passed`

GUI commands below were run from `gui/`; Rust commands were run from the
repository root. The browser smoke used the repository's managed Playwright
installation and an outside-sandbox loopback server after the sandbox correctly
rejected local port binding.

## Checks

- `node --check scripts/smoke.mjs`: `passed`
- `npm run typecheck`: `passed`
- `npm run lint`: `passed`
- `npm run build`: `passed`
- `npm run smoke -- --wizard-only --output /tmp/commandagent-issue-186-wizard-smoke`: `passed`
- `cargo test --test gui_read_only_guard extension_pack_wizard_delegates_lifecycle_and_keeps_failures_actionable`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
