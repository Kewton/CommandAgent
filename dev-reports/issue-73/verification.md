# Issue #73 Verification

- Status: `passed`

## Checks

- `cargo test tui::boundary_shell::presentation::tests`: `passed`
- `cargo test --test gui_read_only_guard`: `passed`
- `node --check gui/scripts/smoke.mjs`: `passed`
- `(cd gui && npm run typecheck)`: `passed`
- `(cd gui && npm run lint)`: `passed`
- `(cd gui && npm run build)`: `passed`
- `cargo test --features gui --test gui_server`: `passed`
- `cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo build --release --bin commandagent`: `passed`
- `target/release/commandagent --version`: `passed`
- `(cd gui && npm run smoke -- --feedback-only --output /private/tmp/commandagent-issue73-feedback.YeiIbo --commandagent-bin ../target/release/commandagent)`: `passed`
- `(cd gui && npm run smoke -- --output /private/tmp/commandagent-issue73-green-smoke.wkYMkb --commandagent-bin ../target/release/commandagent --model qwen3:8b)`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `cargo test --test generality_guardrails nextjs_boundary_erosion_tripwire_keeps_dispatch_sites_audited -- --exact --nocapture` (CI follow-up): `passed`
- `ruff check workspace/management/scripts/test_first_loop_doc.py` (CI follow-up): `passed`
- `python3 workspace/management/scripts/test_first_loop_doc.py` (CI follow-up): `passed`
- `python3.12 -m unittest discover -s workspace/management/scripts -p 'test_*.py'` (CI-equivalent Python suite): `passed`
- `git diff --check`: `passed`

## Results

- The final integrated two-base-path Playwright report returned overall
  `ok: true`; both `root` and `proxy-commandagent` cases returned `ok: true`
  with no unexpected console errors. Its scratch runtime was removed after
  success.
- Both cases recorded `gate_1.copy_is_plain_japanese: true`, the rendered
  `card_markdown` text under the stable test ID, the updated
  `gate_1.visible_text`, and `terminal_heading_is_plain_japanese: true` while
  the API assurance/verdict fixture value was `static`.
- The full Rust suite passed with 1,868 tests passed and 15 ignored. All 15 GUI
  server integration tests passed, including unchanged delegated event bytes,
  exact-hash confirmation behavior, and the integrated coded-error/session
  contracts.
- The release binary reported `commandagent 0.1.0`; the build metadata marked
  the working tree dirty because verification ran before the required Issue
  commit.
- PR #92 initially failed both GitHub Actions workflows because the Issue added
  six reader-facing `nextjs` literals in `presentation.rs` without adding that
  leaf module to the exact-count boundary audit. The audit now pins the module
  at six literals; no production behavior or guardrail condition was weakened.
- The next CI attempt exposed an independent documentation drift assertion that
  still required the removed English Gate 1 heading and proposal sentence. It
  now pins the approved Japanese heading, required-check section, exact-change
  confirmation ID guidance, and proposal-not-result meaning. The Python 3.12
  suite passed all 137 tests after the update.
- Develop integration initially left the browser fixture expecting the old raw
  `pass` title, an uncoded synthetic 409 response, and `へ再接続`. Focused source
  guards and a two-base feedback probe passed after aligning those expectations
  with the plain Gate 3 title, Issue #72 `{code,error}` contract, and current
  `に再接続` button copy. The following full smoke then passed both base paths.
