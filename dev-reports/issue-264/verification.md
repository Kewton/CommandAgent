# Issue 264 verification

- Status: `passed`

## Starting point

- A fresh `git fetch origin develop` resolved both `HEAD` and
  `origin/develop` to `3eb1cca177daf968336fc53add86b789bcf06c4f` before
  implementation.
- The working tree was clean before `design.md` was written.

## Checks

- `node --check gui/scripts/smoke.mjs`: `passed`
- `cargo test --test gui_read_only_guard trial_ui_keeps_gate_one_confirmation_and_has_no_intervention_surface -- --exact`: `passed`
- `cargo test --test gui_read_only_guard extension_catalog_keeps_supply_warnings_and_trial_handoff_explicit -- --exact`: `passed`
- `cargo test --test gui_read_only_guard trial_status_polling_revalidates_with_durable_timing_metadata -- --exact`: `passed`
- `cargo test --test gui_read_only_guard trial_session_index_is_bounded_read_only_and_reconnects_by_link -- --exact`: `passed`
- `cargo test --test gui_read_only_guard run_detail_and_measurement_read_only_browsing_contracts_are_pinned -- --exact`: `passed`
- `cargo test --test gui_read_only_guard -- --test-threads=1`: `passed`
  (25 passed)
- `npm run lint` (from `gui/`): `passed`
- `npm run typecheck` (from `gui/`): `passed`
- `GUI_BASE_PATH=/ npm run build` (from `gui/`): `passed`
- `cargo build --release --bin commandagent`: `passed`
- `target/release/commandagent --version`: `passed`
  (`commandagent 0.1.0 3eb1cca1+dirty`)
- `cargo build --release --features gui --bin gui_server`: `passed`
- `target/release/gui_server ... --check --json` with the throwaway roots and
  current release binary: `passed`
- `node scripts/demo/record_gui_demo.mjs --base=http://127.0.0.1:4173 --model=qwen3.8:27b-mlx --out=/private/tmp/commandagent-issue-264-gui-recording.SqkFSy/capture --timeout-ms=1800000`: `passed`
- `npm run smoke -- --output /private/tmp/commandagent-issue-264-full-smoke.g2yDdt --commandagent-bin ../target/release/commandagent`
  (from `gui/`): `failed`; both root and proxy completed, but each aggregate
  case isolated the same Issue 75 run-selector mismatch that the parent then
  authorized for UAT repair loop 1.
- `npm run smoke -- --output /private/tmp/commandagent-issue-264-loop1-full-smoke.TiJEpY --commandagent-bin ../target/release/commandagent`
  (from `gui/`): `passed`; root and proxy both report aggregate `ok: true`.
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `cargo test`: `passed`
- `cargo test --features gui -- --test-threads=1`: `passed`
- `git diff --check`: `passed`

The first local `npm run typecheck` prerequisite attempt could not resolve the
lockfile-pinned development type packages because the shell has
`NODE_ENV=production` and npm omitted dev dependencies. Running
`npm ci --include=dev` installed the pinned dependencies without changing the
lockfile; the final lint, typecheck, and builds above passed.

## UAT repair loop 1

The report at
`/private/tmp/commandagent-issue-264-full-smoke.g2yDdt/browser-smoke.json`
records `ok: false`. Both completed cases have exactly the same failing
aggregate assertion:

- root: `issue_75.run_selection.options_include_dates_and_status: false`
- proxy: `issue_75.run_selection.options_include_dates_and_status: false`

The merged product page renders each run option with
`repositoryRunStatusLabel(run.state, run.status_text)` at
`gui/app/runs/page.tsx:210`, introduced by merge commit `3f995753`. The
pre-existing smoke expectation composed the option from raw `run.status_text`.
The parent scope amendment authorized only this final integration repair.

The smoke now contains a strict JavaScript equivalent of the product formatter:
`pass` maps to `成功`, `fail` to `失敗`, `pending` to either `記録あり` or
`進行中` after the same enum normalization, and other states to either
`未記録` or `判定不能`. The complete option comparison still includes the
formatted date and exact run ID. The directly corresponding Rust source guard
pins the semantic mapper and rejects the former raw-status interpolation.

The fresh report at
`/private/tmp/commandagent-issue-264-loop1-full-smoke.TiJEpY/browser-smoke.json`
records overall `ok: true`, root `ok: true`, proxy `ok: true`, and
`issue_75.run_selection.options_include_dates_and_status: true` in both cases.
No new failure appeared.

All seven prior repaired contracts remained green in both completed modes:

- visible Gate 1 text contains `GATE 1 / 見積り`;
- session-index rows contain the localized final gate label;
- incompatible-pack normalization reports `ok: true`;
- lease blocking reports `ok: true`, with contract checking disabled and both
  proposal and dispatch counts at zero;
- reconnect reports a visible `BUTTON`, `type="button"`, and an exact matching
  accessible name while the linked lifecycle remains GET-only;
- ten-minute polling reports `ok: true`, 600,000 ms simulated duration,
  conditional requests, and reductions above 92% (61 root calls, 62 proxy
  calls, versus 801 fixed-interval calls);
- both unexpected-console-error arrays are empty.

## Recording evidence

- Throwaway root:
  `/private/tmp/commandagent-issue-264-gui-recording.SqkFSy`
- Recorded session: `01a02dbf-2c1b-7cb0-ae24-d3c79137f2f0`
- Result: real terminal Gate 3 completion
- Installed asset: `docs/assets/demo/gui-demo.gif`
- Dimensions: 1,000 x 625
- Frames and duration: 245 frames, 30.630 seconds
- Size: 605,042 bytes
- SHA-256:
  `807641ac30b5aab8ebbe2ff52f5d4d12211769b8fbaf9d4a5d2cbd5ed14f1427`
- The installed asset and recorder output hashes are identical.

## Disposition

The real GIF recording, all focused/broader repository checks, and the fresh
aggregate root/proxy smoke passed. Verification is `passed`, so the authorized
owned-path local commit may be created. The parent checklist/comment,
post-merge ten-minute Trial, and `commandagent --plan-run` evidence remain
orchestrator-owned.
