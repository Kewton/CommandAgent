# Issue #370 verification

- Status: `passed`
- `git diff --check`: `passed`
- `cd gui && node --check scripts/session-index-smoke.mjs && node --check scripts/smoke.mjs && npm run lint && npm run typecheck`: `passed`
- `cd gui && npm run build`: `passed`
- `cd gui && npm run smoke:session-index -- --output /tmp/commandagent-issue-370-session-index-smoke-final`: `passed`
- `cd gui && npm run smoke -- --feedback-only --output /tmp/commandagent-issue-370-feedback-smoke-final-2`: `passed`
- `cargo test --features gui --test gui_server session_index_requires_authentication_tracks_directories_and_caps_results -- --exact`: `passed`
- `cargo test --features gui --test gui_server confirmed_session_delegates_with_cli_event_bytes_unchanged -- --exact`: `passed`
- `cargo test --test gui_read_only_guard`: `passed`
- `cargo test --test doc_drift`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `cargo test --features gui --test gui_server`: `passed`

The session-index browser report ended with `ok: true` for both `/` and
`/proxy/commandagent/`. The focused feedback/history report also ended with
`ok: true` for both base paths. Temporary browser evidence was written outside
the repository, and no raw logs or credentials were added to the worktree.

## Dependency CI-fix propagation

Issue #369 source commit `45e27d0a148ce161a02d14bf9170864fa8d92b8b`
was incorporated as explicit cherry-pick commit `f0fb9ccf`. Its three Issue
#369 report files match the incoming commit exactly. The only later differences
in `tests/gui_server.rs` are Issue #370's four session-index profile/intent
assertions; the incoming deterministic argv-file completion-boundary test is
otherwise unchanged.

- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --features gui --test gui_server typed_trial_intents_are_validated_frozen_and_delegated -- --exact`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --features gui --test gui_server`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --test gui_read_only_guard`: `passed`
- `cd gui && npm run smoke:session-index -- --output /tmp/commandagent-issue-370-dependency-ci-fix-route-smoke`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --exit-code 45e27d0a148ce161a02d14bf9170864fa8d92b8b -- dev-reports/issue-369`: `passed`

The exact test's first sandboxed attempt could not bind `127.0.0.1:0`
(`Operation not permitted`). The identical authoritative command above was
rerun outside the filesystem/network sandbox and passed. The full GUI-server
suite passed all 40 tests, the read-only guard passed all 25 tests, and the
route smoke ended with `ok: true` for both `/` and `/proxy/commandagent/`.
