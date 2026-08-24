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

## Merge recovery from origin/develop

The branch was synchronized with exact base
`7abad1484fa29051a692f0b452b8158bb68808e2` by normal merge commit
`b9864ee82fd5d87d5fcb79734afe5e93b4464d03`. The pre-merge Issue #370 head
was `9e8e178b97b49c78411ad9d2ba1783168227cdd9`; those two commits are the
merge commit's first and second parents, respectively. The final evidence
commit is a direct first-parent child of that merge commit, so the final
ancestry retains both the Issue #370 implementation history and the exact
current `origin/develop` history without a rebase or rewrite.

The merge required four textual conflict resolutions:

- `README.md` and `README.ja.md` retain Issue #370's separate Trial page
  wording while linking Issue #371's four-layer extension guides and detailed
  lifecycle documentation.
- `docs/guide/README.md` retains the Issue #370 route description and the
  Issue #371 English/Japanese extension-guide entry points.
- `docs/user/gui-help-map.md` retains the Issue #370 compose/status/history/
  detail vocabulary and the Issue #371 extension-layer/reference vocabulary.

The preservation audit confirmed that Issue #369's deterministic idle
completion boundary and exact `--intent`, executor provider/model, and planner
provider/model assertions remain present. Issue #370's four session-index
profile/intent assertions are the only differences in `tests/gui_server.rs`
from the merged Issue #369 version. Issue #371's extension-root work and Issue
#375's plan-step event implementation, tests, and corpus fixture were retained
from `origin/develop`. No Gate 1, honest-failure, event schema, or read-only
contract was weakened.

- `git diff --cached --check`: `passed`
- `git merge-base --is-ancestor 7abad1484fa29051a692f0b452b8158bb68808e2 HEAD`: `passed`
- `git diff --exit-code 7abad1484fa29051a692f0b452b8158bb68808e2 -- dev-reports/issue-369`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --features gui --test gui_server typed_trial_intents_are_validated_frozen_and_delegated -- --exact`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --features gui --test gui_server`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --test gui_read_only_guard`: `passed`
- `cargo test --test doc_drift`: `passed`
- `cd gui && npm run smoke:session-index -- --output /tmp/commandagent-issue-370-merge-recovery-route-smoke`: `passed`
- `cd gui && npm run smoke -- --feedback-only --output /tmp/commandagent-issue-370-merge-recovery-feedback-smoke`: `passed`
- `cd gui && node --check scripts/session-index-smoke.mjs`: `passed`
- `cd gui && node --check scripts/smoke.mjs`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run build`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`

The post-merge GUI-server suite passed all 40 tests, the read-only guard passed
all 26 tests, and doc drift passed all 23 tests. Both browser-smoke reports
ended with `ok: true` for `/` and `/proxy/commandagent/`; their temporary
artifacts remain outside the repository.
