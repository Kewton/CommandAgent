# Issue #74 Verification

- Status: `passed`

## Focused checks

- `cargo test --features gui --bin gui_server extract_status -- --nocapture`:
  `passed` (3 tests). The cases pin Markdown normalization, pass/fail/pending/
  unknown classification, failure precedence, fallback text, and exact-word
  rather than substring matching.
- `cargo test --features gui --test gui_server run_index_reports_total_before_limit_and_normalized_status_state -- --nocapture`:
  `passed`. The 101-directory fixture returned `total: 101`, a 100-entry
  bounded window, the five expected summary fields, normalized status text,
  and serialized `pass` state.
- `npm run smoke -- --output /tmp/commandagent-issue74-smoke.0CmDK6 --overview-only`:
  `passed` with managed Playwright 1.61.1. Both `/` and
  `/proxy/commandagent/` reported `8 / 164`, matching the API-derived expected
  count. Every visible badge was free of `**` and backticks, all API probes
  returned 200, internal links honored the base path, and no unexpected
  console errors occurred. The report recorded `overview_only` mode and
  removed its scratch runtime after success; no Trial run was dispatched.
- The root dashboard screenshot from the smoke was inspected at its original
  resolution. The single run metric, capability map, unchanged formal-band
  panel, and eight-row ledger rendered without overlap or clipping.

## Full checks

- `npm run lint`: `passed`
- `npm run typecheck`: `passed`
- `npm run build`: `passed`
- `node --check gui/scripts/smoke.mjs`: `passed`
- `cargo test --features gui --test gui_server --test gui_read_only_guard`:
  `passed` (14 tests)
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `cargo test`: `passed`; the library target reported 1,868 passed and 15
  intentionally ignored, and every integration and documentation target was
  green.
- `cargo test --features gui --quiet`: `passed`; this repeated the full Rust
  suite with the GUI binary and GUI integration targets enabled.
- `git diff --check`: `passed`

## Audit notes

- The diff is confined to the runs API, its two GUI consumers, Overview
  styling, the managed smoke, focused tests, and Issue #74 reports.
- No runner growth-tripwire file or baseline, event name/schema, corpus
  contract, historical run/migration evidence, or live `.anvil/` namespace was
  changed.
- The `/api/runs` envelope migration is the Issue-authorized API change. The
  existing summary fields remain present and `state` is additive within each
  summary.
- Independent predecessor commits for Issues #71, #77, and #80 remain
  unmerged.

## Evidence audit and setup notes

- The existing worker's focused/full evidence was reconciled with the final
  diff. The finalizer independently repeated the focused Rust checks,
  Overview-only browser smoke, GUI lint/typecheck/build, both Clippy
  configurations, and the default and GUI-enabled full Rust suites.
- The existing worker restored lockfile-pinned GUI development dependencies
  with `npm ci --include=dev --offline` because this worktree initially lacked
  `gui/node_modules`.
- An early combined GUI server-suite attempt ran before `gui/out` existed and
  its static-dashboard check received 404. After the passing GUI build, the
  exact 14-test suite passed; the later GUI-enabled full suite also passed.
- Sandboxed smoke attempts could not bind `127.0.0.1:0`. The same
  Overview-only probe passed with approved localhost/headless-browser access;
  this did not stop or restart any pre-existing server.
