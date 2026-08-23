# Issues 176, 187, 188, 195, and 196 verification

- Status: `passed`

## Checks

- `node --experimental-strip-types --input-type=module -e 'const f=await import("./gui/lib/format.ts"); if(f.repositoryRunStatusLabel("pending","recorded")!=="記録あり"||f.trialGateLabel("GATE_2")!=="Gate 2（実行）"||f.trialStatusLabel("RUNNING")!=="実行中"||f.phaseStatusLabel("COMPLETED")!=="完了"||f.phaseStageLabel("ultra_phase_context_attached")!=="実行条件を準備中"||f.trialStatusLabel("future")!=="状態不明")process.exit(1)'`: `passed`
- `node --check scripts/smoke.mjs && node --check scripts/session-index-smoke.mjs` (from `gui/`): `passed`
- `npm run lint` (from `gui/`): `passed`
- `npm run typecheck` (from `gui/`): `passed`
- `npm run build` (from `gui/`): `passed`
- `cargo test --test doc_drift gui_help_map_ -- --test-threads=1`: `passed`
- `cargo test --test gui_read_only_guard -- --test-threads=1`: `passed`
- `npm run smoke:session-index -- --output /private/tmp/commandagent-issue-176-session-smoke` (from `gui/`): `passed`
- `npm run smoke -- --overview-only --output /private/tmp/commandagent-issue-176-overview-smoke` (from `gui/`): `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test -- --test-threads=1`: `passed`
