# Issue 390 verification

- Status: `passed`
- `cargo check --features gui --bin gui_server`: `passed`
- `cargo test --features gui --test gui_server selected_working_directory -- --nocapture`: `passed`
- `cargo test --test gui_read_only_guard`: `passed`
- `cargo test --features gui --test gui_server`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test -- --test-threads=1`: `passed`
- `npm run typecheck` (from `gui/`): `passed`
- `npm run lint` (from `gui/`): `passed`
- `npm run build` (from `gui/`): `passed`
- `node --check scripts/smoke.mjs` (from `gui/`): `passed`
- `npm run smoke -- --gate-one-only --commandagent-bin /Users/maenokota/share/work/github_kewton/CommandAgent-issue-390-gui-trial/target/debug/commandagent --output /private/tmp/commandagent-issue-390-gui-smoke.MU7krK` (from `gui/`): `passed`
- `git diff --check`: `passed`

## Notes

An initial parallel full-suite run encountered the existing browser-probe test
port being occupied. That test passed in isolation, and the complete suite then
passed with `--test-threads=1`. The focused Gate 1 smoke mode was also corrected
to skip unrelated long-polling probes; its final root and proxy cases both
passed.
