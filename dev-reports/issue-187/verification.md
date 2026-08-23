# Issues #187, #195, and #198 verification

- Status: `passed`
- `node --check scripts/smoke.mjs` (from `gui/`): `passed`
- `cargo test --test doc_drift gui_help_map_copy_is_owned_once_and_checked_by_smoke -- --exact --test-threads=1`: `passed`
- `cargo test --test gui_read_only_guard -- --test-threads=1`: `passed`
- `node -e 'const fs=require("node:fs"); const checks={"components/pack-wizard.tsx":["コミュニティ・ミニアプリ","id=\"pack-wizard-step-4\" tabIndex={-1}","document.getElementById(targetId)?.focus()","launcherRef.current?.focus()"],"app/assets/page.tsx":["コミュニティ・ミニアプリ","トライアルで使う"],"app/runs/page.tsx":["repositoryRunStatusLabel(run.state, run.status_text)","aria-current={selected === null ? \"true\" : undefined}","aria-current={selected?.path === item.path ? \"true\" : undefined}"],"app/measurements/page.tsx":["aria-current={selected?.path === report.path ? \"true\" : undefined}"]}; for(const [path,needles] of Object.entries(checks)){const source=fs.readFileSync(path,"utf8"); for(const needle of needles){if(!source.includes(needle)) throw new Error(path+": "+needle);}}'` (from `gui/`): `passed`
- `node /private/tmp/issue-187-browser-probe.mjs` (temporary focused Playwright probe, removed after cleanup): `passed`
- `npm run lint` (from `gui/`): `passed`
- `npm run typecheck` (from `gui/`): `passed`
- `npm run build` (from `gui/`): `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test -- --test-threads=1`: `passed`
- `npm run smoke -- --wizard-only --output /private/tmp/commandagent-issue-187-wizard-smoke --commandagent-bin /Users/maenokota/share/work/github_kewton/CommandAgent-issue-187-195-198/target/debug/commandagent` (from `gui/`, outside sandbox): `passed`
- `git diff --check`: `passed`

## Failure and amendment history

- The original full suite passed all 2,124 library tests and stopped in
  `tests/doc_drift.rs` because its predecessor-owned assertion still expected
  `Trial で使う`. The first amendment aligned that assertion. Its focused rerun
  then exposed the same stale help-map and smoke ownership entries; the second
  amendment aligned only those direct copies and the trial-link expectation.
- The next focused doc-drift rerun exposed `pack 作成ウィザードを開く` in the
  test, help map, and smoke help registry. The third amendment changed only
  those three entries to `パック作成ウィザードを開く`, after which the
  focused doc-drift contract passed.
- The next full-suite run reached the 25-test GUI guard and passed 22 tests.
  `extension_catalog_keeps_supply_warnings_and_trial_handoff_explicit`,
  `extension_pack_wizard_delegates_lifecycle_and_keeps_failures_actionable`,
  and `trial_session_index_is_bounded_read_only_and_reconnects_by_link` failed
  on five stale direct expectations. The fourth amendment aligned exactly the
  five authorized literals. The focused guard then passed 25/25 and the final
  complete Rust suite passed.
- The first local Next probe and the sandboxed official wizard smoke could not
  bind loopback and returned `listen EPERM` / `Operation not permitted`. These
  were out-of-scope final-integration environment restrictions, not application
  results. The temporary focused Playwright probe passed outside the sandbox.
- The first official outside-sandbox wizard smoke reached the real UI but
  timed out on the stale `保存済み bytes を再検証` selector. The fifth amendment
  changed only that direct button name to `保存済みの内容を再検証`; the rerun
  passed both `/` and `/proxy/commandagent/` cases with zero Axe violations and
  no unexpected console errors.
- The first dependency install inherited `NODE_ENV=production`, so TypeScript
  could not resolve dev-only type packages. `npm ci --include=dev
  --ignore-scripts` restored the lockfile-pinned development dependencies; the
  final typecheck and build pass and no dependency file changed.
