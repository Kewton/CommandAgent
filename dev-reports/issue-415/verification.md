# Issue #415 verification

- Status: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo test tui::boundary_shell::recovery_run::tests --lib`: `passed`
- `cargo test --test gui_read_only_guard`: `passed`
- `cargo test --features gui --test gui_server -- --test-threads=1`: `passed`
- `cargo test --test corpus_regression generated_app_corpus_matches_detector_and_probe_expectations`: `passed`
- `cargo test`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run build`: `passed`
- `cd gui && npm run smoke:run-evidence`: `passed`
- `cd gui && npm run smoke:session-index`: `passed`
- `git diff --check`: `passed`

## Coverage notes

The focused GUI server suite executed all 54 tests. The repository-wide suite
included generality guardrails, documentation drift checks, the unchanged
historical corpus, and the new Issue #415 corpus case.
The session-index smoke ran outside the sandbox because Playwright and its
temporary loopback server require those process and bind permissions.

Two non-required exploratory smoke commands were not used as release gates:
`smoke:errors` consistently timed out after its mocked first reconnect response
was consumed by the existing cross-page redirect, and `smoke:storage` timed out
waiting for an existing storage response. Neither script or covered subsystem
was changed by Issue #415; the required GUI build, static GUI guard, full
session-index Playwright smoke, focused server integration suite, and full Rust
suite passed.
