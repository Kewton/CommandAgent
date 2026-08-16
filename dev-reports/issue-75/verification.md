# Issue #75 Verification

- Status: `passed`

## Checks

- `cd gui && npm run lint`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run build`: `passed`
- `node --check gui/scripts/smoke.mjs`: `passed`
- `cargo build --bin commandagent`: `passed`
- `cd gui && npm run smoke -- --output /tmp/commandagent-issue-75-read-only-smoke --read-only --commandagent-bin ../target/debug/commandagent`: `passed`
- `cargo test --features gui --test gui_server --test gui_read_only_guard`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

## Evidence

- Browser report:
  `/tmp/commandagent-issue-75-read-only-smoke/browser-smoke.json`
- Managed Playwright `1.61.1` reported `ok: true` for `/` and
  `/proxy/commandagent/`, with no console errors in either case.
- Both cases matched all 100 displayed run options to their API-derived date
  and ID, omitted `NO RECORDS` before selection, and switched the document
  class from `document-content--wrapped` to
  `document-content--unwrapped` and back.
- At the 390px viewport, both map frames measured 322px client width and
  1120px scroll width. The whole page still fit the viewport, and each
  full-size SVG link retained its configured base path.
- Root and proxy mobile Measurements screenshots plus desktop Run detail
  screenshots are stored beside the browser report and were visually checked
  for clipping and control overlap.

## Setup note

- `npm ci --include=dev --offline` restored the lockfile-pinned GUI
  dependencies because this worktree initially had no `gui/node_modules`.
- The first sandboxed browser attempt could not bind `127.0.0.1:0`. The exact
  smoke command recorded above was rerun with localhost/headless-browser
  permission and passed for both base paths.
