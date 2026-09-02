# Issue #376 verification

- Status: `passed`

## Dependency CI-fix propagation

- Dependency commit: `1acfc81aa0ba7d7f338db4013d94df95e0d7d779`
- Dependency history: merged with a second parent so original Issue #375 commits
  `67001953` and `1acfc81a` remain explicit alongside the earlier equivalent
  cherry-pick `bb270e5d`.
- Resolved scope: the two add/add conflicts selected the incoming Issue #375
  verification note and the Rust 1.98-compatible `as_chunks::<2>().0` pairing
  test. No Issue #376 implementation file changed.
- Local toolchain: `rustc 1.94.0 (4a4ef493e 2026-03-02)`. Both default and GUI
  warnings-denied Clippy checks passed; the Rust 1.98 diagnostic is removed
  structurally because the flagged `chunks_exact(2)` call is absent.
- The first sandboxed GUI-server test attempt could not bind `127.0.0.1:0`.
  The unchanged suite was rerun outside the network sandbox and passed 40/40.

### Dependency propagation checks

- `cargo fmt --all -- --check`: `passed`
- `cargo test plan_step_events --lib`: `passed`
- `cargo test session_tasks --features gui --bin gui_server`: `passed`
- `cargo test --features gui --test gui_server`: `passed`
- `cargo test --test gui_read_only_guard`: `passed`
- `cd gui && node --check scripts/session-index-smoke.mjs`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run build`: `passed`
- `cd gui && npm run smoke:session-index -- --output /tmp/commandagent-issue-376-dependency-session-index-smoke`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`

## Second ordered dependency CI-fix propagation

- Dependency head: `9e8e178b97b49c78411ad9d2ba1783168227cdd9`.
- Dependency history: merged with a second parent so the ordered Issue #370
  chain `31410760` -> `f0fb9ccf` -> `9e8e178b` remains explicit. The prior
  Issue #375 merge commit `70afa5b8` remains the first-parent baseline.
- Merge scope: the incoming Issue #369/#370 reports and the deterministic
  `typed_trial_intents_are_validated_frozen_and_delegated` test merged without
  conflicts. Issue #376's later GUI-server assertions and Plan task projection
  remained present.
- CI-race boundary: the fixture deliberately creates an empty argument file,
  then assertions wait for the GUI server's `idle` state before reading it.
  Exact intent and executor/planner provider-model argument pairs still pass.
- The plan UI smoke ended with `ok: true` for both `/` and
  `/proxy/commandagent/`; all 100-task, execution-interval, failed-expansion,
  accessibility, reconnection, legacy, and bounded-payload checks remained
  green.

### Second dependency propagation checks

- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --features gui --test gui_server typed_trial_intents_are_validated_frozen_and_delegated -- --exact`: `passed`
- `cargo test plan_step_events --lib`: `passed`
- `cargo test session_tasks --features gui --bin gui_server`: `passed`
- `cargo test --features gui --test gui_server`: `passed`
- `cargo test --test gui_read_only_guard`: `passed`
- `cd gui && node --check scripts/session-index-smoke.mjs`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run build`: `passed`
- `cd gui && npm run smoke:session-index -- --output /tmp/commandagent-issue-376-issue-370-ci-fix-session-index-smoke`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`

## Checks

- `cargo test session_tasks --features gui --bin gui_server`: `passed`
- `cargo test --features gui --test gui_server`: `passed`
- `cargo test --test gui_read_only_guard`: `passed`
- `cargo test --test doc_drift`: `passed`
- `cd gui && node --check scripts/session-index-smoke.mjs && node --check scripts/smoke.mjs`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run build`: `passed`
- `cd gui && npm run smoke:session-index -- --output /tmp/commandagent-issue-376-session-index-smoke`: `passed`
- `cd gui && npm run smoke -- --feedback-only --output /tmp/commandagent-issue-376-feedback-smoke`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

## Merge recovery against exact origin/develop

- Exact fetched base: `7b1c2d8df37053d8719d24ed18094a8a8c18012b`.
  Both `origin/develop` and `FETCH_HEAD` resolved to that commit after the
  authorized fetch.
- Pre-merge Issue #376 head: `810dd041a20cf73b03c1979cc66015d26cf65a6e`.
- Pre-merge merge bases: `9e8e178b97b49c78411ad9d2ba1783168227cdd9`
  and `1acfc81aa0ba7d7f338db4013d94df95e0d7d779`.
- Merge method: `git merge --no-ff --no-commit origin/develop`. The recovery
  commit retains `810dd041a20cf73b03c1979cc66015d26cf65a6e` as first parent and
  `7b1c2d8df37053d8719d24ed18094a8a8c18012b` as second parent; no rebase or
  history rewrite was used.
- Conflicts: none. Git auto-merged `CHANGELOG.md`, `gui/app/globals.css`,
  `gui/lib/types.ts`, `src/bin/gui_server.rs`, `tests/gui_read_only_guard.rs`,
  and `tests/gui_server.rs`. The overlap audit confirmed that current develop's
  profile-supply work and deterministic idle-boundary GUI delegation test remain
  intact alongside Issue #376's typed task projection, 100-task bound,
  execution intervals, failed expansion, accessibility, reconnection, legacy,
  and compact-history behavior. No choose-one-side resolution was required.
- Historical evidence under `workspace/management/runs/` is inherited from the
  exact develop parent without modification; this recovery created no run
  directory.
- The session-index smoke completed with `ok: true` for both `/` and
  `/proxy/commandagent/`. It covered direct reload and reconnection, separate
  execution intervals for duplicate Step IDs, all terminal task outcomes,
  failed-task auto-expansion and evidence navigation, legacy unsupported state,
  keyboard and heading accessibility, compact terminal history, and bounded
  payload/rendering for 100 live tasks (plus 101 terminal tasks).

### Merge recovery checks

- `git fetch origin develop`: `passed` (`origin/develop` and `FETCH_HEAD` = `7b1c2d8df37053d8719d24ed18094a8a8c18012b`)
- `git merge-base --all HEAD origin/develop`: `passed` (`9e8e178b97b49c78411ad9d2ba1783168227cdd9`, `1acfc81aa0ba7d7f338db4013d94df95e0d7d779`)
- `git merge --no-ff --no-commit origin/develop`: `passed`
- `git diff --exit-code origin/develop -- workspace/management/runs`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo test plan_step_events --lib`: `passed` (3 passed)
- `cargo test session_tasks --features gui --bin gui_server`: `passed` (6 passed)
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --features gui --test gui_server typed_trial_intents_are_validated_frozen_and_delegated -- --exact`: `passed` (1 passed)
- `cargo test --features gui --test gui_server`: `passed` (42 passed)
- `cargo test --test gui_read_only_guard`: `passed` (28 passed)
- `cargo test --test doc_drift`: `passed` (23 passed)
- `cd gui && node --check scripts/session-index-smoke.mjs`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run build`: `passed`
- `cd gui && npm run smoke:session-index -- --output /tmp/commandagent-issue-376-merge-recovery-session-index-smoke`: `passed` (root and proxy `ok: true`)
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed` (library: 2144 passed, 16 ignored; all integration and doc tests passed)
- `git diff --cached --check`: `passed`

## Post-Issues 373/374 merge recovery

### Exact base and ancestry

- Fetched `origin/develop` and required both `origin/develop` and `FETCH_HEAD`
  to resolve to `ffe59416fa201b2450737fcac266f8519d3fee00` before merging.
- Pre-merge Issue #376 head: `14524404500cae0eb7936e3e2682f07bb6f96c45`.
- Exact merge base: `7b1c2d8df37053d8719d24ed18094a8a8c18012b`.
- Merge method: `git merge --no-ff --no-commit ffe59416fa201b2450737fcac266f8519d3fee00`.
  The recovery commit retains
  `14524404500cae0eb7936e3e2682f07bb6f96c45` as first parent and exact develop
  `ffe59416fa201b2450737fcac266f8519d3fee00` as second parent. No rebase or
  history rewrite was used.

### Conflicts and deliberate resolutions

- `CHANGELOG.md`: retained both the Issue #376 typed StepPlan task-progress
  entry and Issue #374's authenticated, copyable delegated-working-directory
  entry.
- `docs/dev/mechanism-ledger.md`: retained the complete GUI-374 authenticated
  path/confinement contract and the complete GUI-376 typed-event/task contract,
  ordered as GUI-374 then GUI-376. No event schema, honest-failure rule, or
  `.anvil/` contract was changed.
- `gui/scripts/session-index-smoke.mjs`: combined Issue #376's 100-task payload
  bound, live/terminal task assertions, duplicate execution intervals, failed
  evidence, legacy fallback, and accessibility fixtures with Issue #374's
  clipboard stub, authenticated path requests, cwd consistency, missing-state,
  copy/live-region, and mobile checks. Neither side was selected wholesale.
- Automatically merged GUI/API/test overlaps were audited against both parents.
  Issue #373's Overview landing/readiness/accessibility behavior and Issue
  #374's GET-only token-authenticated, canonically confined path projection
  remain present alongside Issue #376's typed `task_progress` projection and
  compact history.
- The first combined session-index smoke reached the copy check but timed out
  after focusing the button and sending `Enter` globally. Issue #376 status
  rerenders can replace that focused DOM node between the two operations. The
  probe now sends `Enter` through the current button locator with
  `copyButton.press("Enter")`; it still requires keyboard focus, the exact
  clipboard path, and the polite live-region success message. The final rerun
  passed both base paths without weakening an acceptance condition.
- `workspace/management/runs`, `docs/migration`, and `.anvil` remain identical
  to the pre-merge Issue #376 parent. This recovery created no run directory.

### Focused and GUI checks

- `cargo test plan_step_events --lib`: `passed` (3 passed)
- `cargo test session_tasks --features gui --bin gui_server`: `passed` (6 passed)
- `cargo test session_paths --features gui --bin gui_server`: `passed` (0 matched; path behavior verified by the integration tests below)
- `cargo test --test gui_read_only_guard gui_style_and_overview_landing_accessibility_contracts_are_pinned -- --exact`: `passed` (1 passed)
- `cargo test --test gui_read_only_guard trial_session_paths_are_dedicated_authenticated_and_copyable -- --exact`: `passed` (1 passed)
- `cargo test --test gui_read_only_guard trial_task_projection_is_typed_read_only_and_keeps_history_compact -- --exact`: `passed` (1 passed)
- `cargo test --features gui --test gui_server trial_session_paths_are_token_only_confined_and_report_missing_workspaces -- --exact`: `passed` (1 passed)
- `cargo test --features gui --test gui_server confirmed_session_delegates_with_cli_event_bytes_unchanged -- --exact`: `passed` (1 passed)
- `cargo test --features gui --test gui_server typed_trial_intents_are_validated_frozen_and_delegated -- --exact`: `passed` (1 passed)
- `cd gui && node --check scripts/session-index-smoke.mjs`: `passed`
- `cd gui && node --check scripts/smoke.mjs`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run build`: `passed`
- `cd gui && npm run smoke:session-index -- --output /tmp/commandagent-issue-376-post-373-374-session-index-smoke`: `failed` (initial combined run timed out at the global-keyboard copy boundary)
- `cd gui && npm run smoke:session-index -- --output /tmp/commandagent-issue-376-post-373-374-session-index-smoke-final`: `passed` (root and proxy `ok: true`)
- `cd gui && npm run smoke -- --overview-only --output /tmp/commandagent-issue-376-post-373-374-overview-smoke --commandagent-bin ../target/debug/commandagent`: `passed` (root and proxy `ok: true`; zero axe violations)
- `cargo test --features gui --test gui_server -- --test-threads=1`: `passed` (43 passed)
- `cargo test --test gui_read_only_guard`: `passed` (29 passed)
- `cargo test --test doc_drift`: `passed` (23 passed)

The final session-index report retained current phase/task position, 100 live
tasks and 101 terminal tasks, every terminal outcome, two execution intervals,
duplicate Step ID separation, failed-task auto-expansion/evidence navigation,
keyboard disclosure, heading hierarchy, direct reload/reconnection, compact
history, legacy unsupported state, and payload sizes of 41,421 and 42,549
bytes. It also passed the authenticated GET-only path, cwd consistency,
keyboard copy/live region, missing workspace, and mobile checks for both `/`
and `/proxy/commandagent/`.

### Required broad gates and repository audit

- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed` (library: 2144 passed, 16 ignored; all integration and doc tests passed)
- `git rev-parse origin/develop FETCH_HEAD`: `passed` (both `ffe59416fa201b2450737fcac266f8519d3fee00`)
- `git merge-base --all HEAD MERGE_HEAD`: `passed` (`7b1c2d8df37053d8719d24ed18094a8a8c18012b`)
- `git ls-files -u`: `passed` (no unresolved entries)
- `git diff --exit-code 14524404500cae0eb7936e3e2682f07bb6f96c45 -- workspace/management/runs docs/migration .anvil`: `passed`
- `git diff --check`: `passed`
- `git diff --cached --check`: `passed`
