# Issue #184 Verification

- Status: `passed`

## Checks

- `cargo test --test gui_read_only_guard run_detail_and_measurement_read_only_browsing_contracts_are_pinned`: `passed`
- `node --check gui/scripts/smoke.mjs`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run build`: `passed`
- `cargo test --features gui --test gui_server --test gui_read_only_guard`: `passed`
- `cd gui && npm run smoke -- --output /private/tmp/commandagent-issue184-smoke-20260822-final --read-only --commandagent-bin ../target/debug/commandagent`: `passed`
- `cd gui && node /private/tmp/issue184-axe.mjs http://127.0.0.1:57267`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

## Browser evidence

- Managed Playwright 1.61.1 returned `ok: true` for `/` and
  `/proxy/commandagent/` with no unexpected console errors.
- Both cases rendered `表示件数 100 / 総数 196` and matched every displayed
  option to the run-index payload.
- Both cases found and opened the real repository run
  `20260719-015733-orchestrate`, which was absent from the 100 returned run
  summaries.
- At 390 px, the selected-ID output and the full Runs page fit their containers
  without horizontal overflow. The final screenshots were inspected at their
  original resolution.
- The browser probe recorded the Measurements heading sequence as
  `h1, h2, h2, h3` and retained the selected report after visibility
  revalidation.

## Axe evidence

- axe-core 4.13.0 ran only the `heading-order` rule against the rendered
  Measurements and Runs pages.
- Measurements headings were `計測` (`h1`), `到達度 × 構成時間` (`h2`),
  `レポート一覧` (`h2`), and `uat-report.md` (`h3`): zero violations.
- Runs headings were `リポジトリ実行記録` (`h1`) and `uat-report.md` (`h2`):
  zero violations.

## Suite results

- Focused GUI tests: 24 source-contract tests and 27 GUI-server tests passed.
- Full Rust library target: 1,972 passed and 16 intentionally ignored. Every
  integration and documentation target also passed.

## Setup notes

- `npm ci --include=dev` restored lockfile-pinned GUI dependencies because this
  worktree initially had no `gui/node_modules`; `NODE_ENV=production` required
  the explicit `--include=dev` flag.
- The first sandboxed browser smoke could not bind `127.0.0.1:0`. The identical
  read-only test was rerun with loopback/headless-browser permission and passed.
- axe-core was installed verification-only with
  `--no-save --package-lock=false`; `npm ci --include=dev --offline` then
  restored the lockfile state. No axe dependency or generated runtime output is
  part of the commit.
