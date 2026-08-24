# Issue #374 verification
- Status: `passed`

## Checks

- `cargo test --features gui --test gui_server trial_session_paths_are_token_only_confined_and_report_missing_workspaces -- --exact`: `passed`
- `cargo test --features gui --test gui_server confirmed_session_delegates_with_cli_event_bytes_unchanged -- --exact`: `passed`
- `cd gui && npm run typecheck && npm run lint && node --check scripts/session-index-smoke.mjs && npm run build`: `passed`
- `cd gui && npm run smoke:session-index -- --output /tmp/commandagent-issue-374-session-index-smoke`: `passed`
- `cargo test --test gui_read_only_guard`: `passed`
- `cargo test --test doc_drift`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `cargo test --features gui --test gui_server -- --test-threads=1`: `passed`
- `cd gui && npm run smoke -- --feedback-only --output /tmp/commandagent-issue-374-feedback-smoke`: `passed`
- `git diff --check`: `passed`

## Dependency CI-fix propagation

Dependency head `9e8e178b` and its Issue #369 CI-race fix `f0fb9ccf` are
incorporated while the Issue #374 path-safety checks remain present.

- `git diff --exit-code f0fb9ccf -- dev-reports/issue-369`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --features gui --test gui_server typed_trial_intents_are_validated_frozen_and_delegated -- --exact`: `passed`
- `cargo test --features gui --test gui_server trial_session_paths_are_token_only_confined_and_report_missing_workspaces -- --exact`: `passed`
- `cargo test --features gui --test gui_server confirmed_session_delegates_with_cli_event_bytes_unchanged -- --exact`: `passed`
- `cd gui && node --check scripts/smoke.mjs`: `passed`
- `cd gui && npm run smoke:session-index -- --output /tmp/commandagent-issue-374-dependency-session-index-smoke`: `passed`
- `cd gui && npm run smoke -- --overview-only --output /tmp/commandagent-issue-374-dependency-overview-smoke-final --commandagent-bin ../target/debug/commandagent`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --features gui --test gui_server -- --test-threads=1`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --test gui_read_only_guard`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

The first overview-smoke attempt stopped at its inherited pre-Issue #370
`トライアル` heading expectation. After aligning that one assertion with the
verified `トライアル実行指示` page heading, the final command above passed
for both `/` and `/proxy/commandagent/`, including desktop and mobile Trial
coverage. No timeout or acceptance threshold was changed.

## Merge recovery from origin/develop

### Base and ancestry

- Fetched `origin/develop` and confirmed both `origin/develop` and
  `FETCH_HEAD` resolve to the authorized base
  `7b1c2d8df37053d8719d24ed18094a8a8c18012b`.
- Created normal merge commit
  `b00af551a537ef398b364f482566a86df2a568b8` without rebasing or rewriting
  history.
- The merge parents are Issue #374 head
  `839f9c335a4af780a72433130e452ad984b87c3e` and exact develop
  `7b1c2d8df37053d8719d24ed18094a8a8c18012b`, in that order.
- `git rev-parse origin/develop FETCH_HEAD`: `passed`
- `git rev-parse b00af551^1 b00af551^2`: `passed`
- `git merge-base --is-ancestor 7b1c2d8df37053d8719d24ed18094a8a8c18012b b00af551 && git merge-base --is-ancestor 839f9c335a4af780a72433130e452ad984b87c3e b00af551`: `passed`

### Conflicts and resolutions

- `README.md`: retained Issue #374's copyable per-session working-directory
  entry and retained develop's four-layer extension guide, bounded draft
  profile registration, assurance boundaries, and detailed lifecycle link.
- `README.ja.md`: applied the equivalent Japanese resolution, retaining both
  the copyable session working directory and develop's extension guidance.
- No source or test file had a textual conflict. The auto-merged result was
  audited against `origin/develop`: its net source/test differences are the
  Issue #374 authenticated path endpoint, cwd/path equality, copy/accessibility
  and missing-state UI, four-route session smoke, and the one corrected
  `/try/` heading expectation. The inherited deterministic delegated-argument
  test remains identical to develop at its completion boundary.
- Existing historical evidence was accepted from develop unchanged; this
  recovery created no `workspace/management/runs` artifact.
- `git diff --exit-code origin/develop -- workspace/management/runs docs/migration`: `passed`

### Checks

- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --features gui --test gui_server trial_session_paths_are_token_only_confined_and_report_missing_workspaces -- --exact`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --features gui --test gui_server confirmed_session_delegates_with_cli_event_bytes_unchanged -- --exact`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --features gui --test gui_server typed_trial_intents_are_validated_frozen_and_delegated -- --exact`: `passed`
- `cd gui && npm run smoke:session-index -- --output /tmp/commandagent-issue-374-merge-recovery-session-index`: `passed`
- `cd gui && npm run smoke -- --overview-only --output /tmp/commandagent-issue-374-merge-recovery-overview --commandagent-bin ../target/debug/commandagent`: `passed`
- `cd gui && npm run smoke -- --feedback-only --output /tmp/commandagent-issue-374-merge-recovery-feedback --commandagent-bin ../target/debug/commandagent`: `passed`
- `cd gui && npm run lint && npm run typecheck && npm run build`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --features gui --test gui_server -- --test-threads=1`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --test gui_read_only_guard`: `passed`
- `cargo test --test doc_drift`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`
- `git diff --cached --check`: `passed`

The session-index report ended with `ok: true` for `/` and
`/proxy/commandagent/`, including desktop and 390 px mobile coverage, all four
Trial routes, authenticated path reads, cwd consistency, keyboard copy/live
feedback, distinct run-record paths, and the explicit deleted-workspace state.
The overview and feedback reports also ended with `ok: true` for both base
paths. Temporary browser output stayed below `/tmp`.
