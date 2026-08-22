# Issues #152, #171, and #178 verification

- Status: `passed`

## Checks

- `npm run typecheck` (from `gui/`): `passed`
- `npm run lint` (from `gui/`): `passed`
- `node --check scripts/smoke.mjs` (from `gui/`): `passed`
- `npm run build` (from `gui/`): `passed`
- `cargo test --test gui_read_only_guard extension_catalog_keeps_supply_warnings_and_trial_handoff_explicit -- --exact --nocapture`: `passed`
- `cargo test --test gui_read_only_guard trial_ui_keeps_gate_one_confirmation_and_has_no_intervention_surface -- --exact --nocapture`: `passed`
- `cargo test --test gui_read_only_guard extension_pack_wizard_delegates_lifecycle_and_keeps_failures_actionable -- --exact --nocapture`: `passed`
- `cargo test --test doc_drift introductory_surfaces_keep_cli_and_gui_sample_goals_explicit -- --exact --nocapture`: `passed`
- `npm run smoke -- --overview-only --output /private/tmp/commandagent-issue-171-smoke.ewxV2J` (from `gui/`, outside the filesystem/network sandbox): `passed`
- `cargo build --release --bin commandagent`: `passed`
- `target/release/commandagent --version`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

## Environment notes

- The first TypeScript check found this clean worktree had no `gui/node_modules`.
  `npm ci --include=dev` restored the exact lockfile graph, after which
  typecheck, lint, build, and browser smoke passed.
- The first browser-smoke attempt could not bind its loopback GUI server inside
  the sandbox (`Operation not permitted`). The identical probe was rerun
  outside the sandbox. Its first test-only assertion waited for a hidden
  `datalist` option to become visible; changing that assertion to require the
  option to be attached matched native `datalist` semantics. The corrected run
  passed for both `/` and `/proxy/commandagent/` with no unexpected console
  errors.
- The first full Rust suite exposed the obsolete shared-English GUI sample
  drift assertion. After separating the English CLI and Japanese GUI sample
  contracts, the focused drift test and the final full suite passed.
