# Issue #74 Verification

- Status: `passed`

## Focused checks

- `cargo test --features gui --bin gui_server extract_status -- --nocapture`:
  `passed` (3 tests). The cases pin Markdown normalization, pass/fail/pending/
  unknown classification, failure precedence, fallback text, and exact-word
  rather than substring matching.
- `cargo test --features gui --test gui_server run_index_reports_total_before_limit_and_normalized_status_state -- --nocapture`:
  `passed`. The 101-directory fixture returned `total: 101`, a 100-entry
  bounded window, all six expected summary fields, matching normalized
  `status`/`status_text` values, and serialized `pass` state.
- `npm run smoke -- --output /private/tmp/commandagent-issue74-integrated.ZSbdhW --overview-only`:
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
  `passed` (16 GUI server tests and 16 source-guard tests)
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `cargo test`: `passed`; the library target reported 1,868 passed and 15
  intentionally ignored, and every integration and documentation target was
  green.
- `git diff --check`: `passed`

## Audit notes

- The diff is confined to the runs API, its two GUI consumers, Overview
  styling, the managed smoke, focused tests, and Issue #74 reports.
- No runner growth-tripwire file or baseline, event name/schema, corpus
  contract, historical run/migration evidence, or live `.anvil/` namespace was
  changed.
- The `/api/runs` envelope migration is the Issue-authorized API change. The
  existing summary fields remain present, while `status_text` and `state` are
  additive within each summary.
- Current `develop`, including the independently merged GUI predecessor
  issues, was integrated before the final checks.

## Evidence audit and setup notes

- The existing worker's focused/full evidence was reconciled with the final
  diff. After integrating current `develop`, the finalizer repeated the
  focused Rust checks, Overview-only browser smoke, GUI lint/typecheck/build,
  both Clippy configurations, and the full default Rust suite.
- The existing worker restored lockfile-pinned GUI development dependencies
  with `npm ci --include=dev --offline` because this worktree initially lacked
  `gui/node_modules`.
- An early integrated GUI server-suite compile exposed a stale test helper
  signature after `develop` added an explicit static root. The helper was
  reconciled with both repository-root and static-root inputs; the exact
  16+16-test GUI suite then passed.
- The first sandboxed `npm run typecheck` could not write its incremental
  build-info file. The unchanged command passed with approved worktree write
  access; this was an environment restriction, not a type failure.
- Sandboxed smoke attempts could not bind `127.0.0.1:0`. The same
  Overview-only probe passed with approved localhost/headless-browser access;
  this did not stop or restart any pre-existing server.
