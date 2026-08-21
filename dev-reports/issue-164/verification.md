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

## CI Follow-up Checks (2026-08-21)

- `cargo fmt --all -- --check`: `passed`
- `cargo +1.97.1 clippy --features gui --bin gui_server -- -D warnings`: `passed`
- `cargo test --features gui --test gui_server trial_session_files_`: `passed`
- `cargo test --test gui_read_only_guard gui_visibility_revalidation_and_shared_time_format_are_pinned`: `passed`
- `npm run lint` (working directory: `gui`): `passed`
- `npm run typecheck` (working directory: `gui`): `passed`
- `npm run smoke -- --read-only --output /tmp/commandagent-issue-164-ci-followup.rTyaAC --commandagent-bin /Users/maenokota/share/work/github_kewton/CommandAgent-issue-164-gui/target/release/commandagent` (working directory: `gui`): `passed`
- `git diff --check`: `passed`

The GUI server focused tests passed both session-file cases, including the
symlinked runtime-root rejection. The Issue #164 browser smoke again passed for
both `/` and `/proxy/commandagent/`, with
`selection_retained_after_visibility: true` in each case.
