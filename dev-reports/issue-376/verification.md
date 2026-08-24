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
