# Issue #75 Verification

- Status: `passed`

## Checks

- `cd gui && npm run lint`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run build`: `passed`
- `node --check gui/scripts/smoke.mjs`: `passed`
- `cargo build --bin commandagent`: `passed`
- `cd gui && npm run smoke -- --output /private/tmp/commandagent-issue75-integrated.quXK6w --read-only --commandagent-bin ../target/debug/commandagent`: `passed`
- `cargo test --features gui --test gui_server --test gui_read_only_guard`:
  `passed` (16 GUI server tests and 17 source-guard tests)
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `cargo test`: `passed`; the library target reported 1,868 passed and 15
  intentionally ignored, and every integration and documentation target was
  green.
- `git diff --check`: `passed`

## Evidence

- Browser report:
  `/private/tmp/commandagent-issue75-integrated.quXK6w/browser-smoke.json`
- Managed Playwright `1.61.1` reported `ok: true` for `/` and
  `/proxy/commandagent/`, with no console errors in either case.
- Both cases matched all 100 displayed run options to their API-derived date,
  normalized status text, and ID; filtered to the requested ID; exposed the
  `該当なし` state; omitted `NO RECORDS` before selection; and opened the
  base-path-aware source GET in a new tab.
- Both cases switched the document class from `document-content--wrapped` to
  `document-content--unwrapped` and back.
- At the 390px viewport, both map frames measured 322px client width and
  1120px scroll width. The whole page still fit the viewport, and each
  full-size SVG link retained its configured base path.
- Root and proxy mobile Measurements screenshots plus desktop Run detail
  screenshots are stored beside the browser report. The final root images
  were inspected at original resolution: controls do not overlap, the desktop
  document remains readable, and mobile overflow stays inside the map frame.

## Audit

- `git diff develop` is limited to Issue #75 reports, GUI presentation/shared
  components, the managed smoke, and the focused source guard. No Rust server
  or API implementation differs from `develop`.
- The generated score/time SVG, existing run/migration evidence, `.anvil/`,
  event schemas, corpus contracts, and runner growth tripwires are unchanged.
- The read-only smoke created and stopped only its disposable test servers. It
  did not dispatch a Trial or stop/restart any user server, CommandMate, or
  Ollama process.

## Setup note

- `npm ci --include=dev --offline` restored the lockfile-pinned GUI
  dependencies because this worktree initially had no `gui/node_modules`.
- The first sandboxed browser attempt could not bind `127.0.0.1:0`. The exact
  predecessor smoke command was rerun with localhost/headless-browser
  permission. The final integrated smoke used the same approved test-only
  access and passed for both base paths.
- Integrating current `develop` produced presentation-only conflicts in the
  Japanese Measurements/Run detail copy and the accumulated smoke modes. The
  resolution retains both predecessor behavior and all Issue #75 assertions.
