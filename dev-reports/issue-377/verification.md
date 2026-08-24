# Issue #377 verification

- Status: `passed`

## Checks

- `cargo test failure_explanation`: `passed`
- `cargo test --test corpus_regression`: `passed`
- `cargo test --features gui --test gui_server -- --test-threads=1`: `passed`
- `cargo test --test gui_read_only_guard`: `passed`
- `cargo test --test doc_drift`: `passed`
- `cd gui && node --check scripts/session-index-smoke.mjs && node --check scripts/smoke.mjs`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run build`: `passed`
- `cd gui && npm run smoke:session-index -- --output /tmp/commandagent-issue-377-session-index-smoke`: `passed`
- `cd gui && npm run smoke -- --feedback-only --output /tmp/commandagent-issue-377-feedback-smoke`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

## Finalized dependency CI-fix propagation

The finalized dependency heads were incorporated with ordinary merge commits
in the requested canonical order. Each exact head is an ancestor of this
branch:

- Issue #375: `1acfc81aa0ba7d7f338db4013d94df95e0d7d779`
- Issue #370: `9e8e178b97b49c78411ad9d2ba1783168227cdd9`
- Issue #374: `839f9c335a4af780a72433130e452ad984b87c3e`
- Issue #376: `810dd041a20cf73b03c1979cc66015d26cf65a6e`

The resulting merge order is `#375 -> #370 -> #374 -> #376 -> #377`.
Only the dependency CI fixes and their committed reports entered the Issue
#377 tree. The existing Issue #377 failure projection, final-interval
correlation, recovery read boundary, Gate 1 confirmation, and honest-failure
behavior remain covered by the focused and smoke checks below.

The local default toolchain reported `rustc 1.94.0 (4a4ef493e 2026-03-02)`.
The exact CI-form command was run with warnings denied and incremental
compilation disabled. The Rust 1.98 Clippy finding is removed structurally:
`src/planner/runner/phase/plan_step_events.rs` uses
`as_chunks::<2>().0`, and the flagged `chunks_exact(2)` call is absent.

### Propagation checks

- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --features gui --test gui_server typed_trial_intents_are_validated_frozen_and_delegated -- --exact`: `passed` (`1 passed`, `41 filtered out`)
- `cargo test plan_step_events --lib`: `passed` (`3 passed`, `2158 filtered out`)
- `cargo test session_tasks --features gui --bin gui_server`: `passed` (`6 passed`, `25 filtered out`)
- `cargo test --features gui --test gui_server failed_session_projects_exact_interval_and_reads_only_current_recovery_documents -- --exact --test-threads=1`: `passed` (`1 passed`, `41 filtered out`)
- `cargo test --features gui --test gui_server trial_session_paths_are_token_only_confined_and_report_missing_workspaces -- --exact --test-threads=1`: `passed` (`1 passed`, `41 filtered out`)
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --features gui --test gui_server -- --test-threads=1`: `passed` (`42 passed`)
- `cargo test --test gui_read_only_guard`: `passed` (`28 passed`)
- `cargo test --test doc_drift`: `passed` (`23 passed`)
- `cargo test --test corpus_regression`: `passed` (`2 passed`)
- `cd gui && node --check scripts/session-index-smoke.mjs && node --check scripts/smoke.mjs && npm run lint && npm run typecheck && npm run build`: `passed`
- `cd gui && npm run smoke:session-index -- --output /tmp/commandagent-issue-377-dependency-ci-fix-session-index-smoke`: `passed`
- `jq '{schema_version, ok, cases: [.cases[] | {id, base_path, ok, lifecycle: {ok: .lifecycle.ok, failure_category_visible: .lifecycle.failure_category_visible, failure_sections_ordered: .lifecycle.failure_sections_ordered, recovery_documents_authenticated_get_only: .lifecycle.recovery_documents_authenticated_get_only, recovery_command_copied_by_keyboard: .lifecycle.recovery_command_copied_by_keyboard, apply_prepared_continuation_only: .lifecycle.apply_prepared_continuation_only, failure_heading_hierarchy_valid: .lifecycle.failure_heading_hierarchy_valid, failure_detail_mobile_fits: .lifecycle.failure_detail_mobile_fits, workspace_path_consistent: .lifecycle.workspace_path_consistent, duplicate_step_ids_kept_separate: .lifecycle.duplicate_step_ids_kept_separate}}]}' /tmp/commandagent-issue-377-dependency-ci-fix-session-index-smoke/session-index-smoke.json`: `passed` (`ok: true` for `/` and `/proxy/commandagent/`; every selected failure, recovery, accessibility, mobile, workspace, and execution-interval field was `true`)
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed` (library: `2145 passed`, `16 ignored`; all integration and doc tests passed)
- `git merge-base --is-ancestor 1acfc81aa0ba7d7f338db4013d94df95e0d7d779 HEAD`: `passed`
- `git merge-base --is-ancestor 9e8e178b97b49c78411ad9d2ba1783168227cdd9 HEAD`: `passed`
- `git merge-base --is-ancestor 839f9c335a4af780a72433130e452ad984b87c3e HEAD`: `passed`
- `git merge-base --is-ancestor 810dd041a20cf73b03c1979cc66015d26cf65a6e HEAD`: `passed`
- `git diff --check`: `passed`
