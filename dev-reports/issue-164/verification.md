# Issue #164 Verification

- Status: `passed`

## Checks

- `cargo test --test gui_read_only_guard gui_visibility_revalidation_and_shared_time_format_are_pinned`: `passed`
- `npm run lint` (working directory: `gui`): `passed`
- `npm run typecheck` (working directory: `gui`): `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo build --release --features gui --bin commandagent`: `passed`
- `target/release/commandagent --version`: `passed`
- `npm run smoke -- --read-only --output /tmp/commandagent-issue-164-smoke.eGaXZZ --commandagent-bin /Users/maenokota/share/work/github_kewton/CommandAgent-issue-164-gui/target/release/commandagent` (working directory: `gui`): `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

## Acceptance Evidence

The read-only browser smoke passed for both `/` and
`/proxy/commandagent/`. Each case recorded
`selection_retained_after_visibility: true`, with the same non-first report path
before and after the hidden-to-visible transition.
