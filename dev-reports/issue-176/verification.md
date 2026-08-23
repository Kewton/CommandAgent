# Issues 176, 187, 188, 195, and 196 verification

- Status: `blocked`

## Checks

- `node --experimental-strip-types --input-type=module -e 'const f=await import("./gui/lib/format.ts"); if(f.repositoryRunStatusLabel("pending","recorded")!=="記録あり"||f.trialGateLabel("GATE_2")!=="Gate 2（実行）"||f.trialStatusLabel("RUNNING")!=="実行中"||f.phaseStatusLabel("COMPLETED")!=="完了"||f.phaseStageLabel("ultra_phase_context_attached")!=="実行条件を準備中"||f.trialStatusLabel("future")!=="状態不明")process.exit(1)'`: `passed`
- `node --input-type=module -e 'import{readFile}from"node:fs/promises";const s=await readFile("gui/components/shell.tsx","utf8"),i=await readFile("gui/components/trial-session-index.tsx","utf8"),p=await readFile("gui/app/page.tsx","utf8"),h=await readFile("docs/user/gui-help-map.md","utf8");if(![s.includes("aria-live=\"polite\""),s.includes("aria-current={item.route === active ? \"page\" : undefined}"),i.includes("aria-current={highlight === session.id ? \"true\" : undefined}"),i.includes("trialGateLabel(session.gate)} / {trialStatusLabel(session.status)"),p.includes("repositoryRunStatusLabel(run.state, run.status_text)"),h.includes("## Canonical terminology"),h.includes("## Shared status labels")].every(Boolean))process.exit(1)'`: `passed`
- `npm run lint` (from `gui/`): `passed`
- `npm run typecheck` (from `gui/`): `passed`
- `npm run build` (from `gui/`): `passed`
- `cargo test --features gui --test gui_server gui_lists_and_proposes_an_external_draft_profile_with_a_local_pack -- --exact`: `passed`
- `cargo test --features gui --test gui_server -- --test-threads=1`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `blocked` — the default parallel run passed 2,117 library tests but seven subprocess-heavy tests exceeded their two-second bounds; every affected test passed in the serial rerun.
- `cargo test -- --test-threads=1`: `blocked` — all 2,124 serial library tests passed, then `tests/doc_drift.rs::gui_help_map_copy_is_owned_once_and_checked_by_smoke` required the obsolete copy `Trial がファイルを変更できる、専用の作業ディレクトリです。` in `gui/components/getting-started.tsx`.

## Blocker

The approved scope assigns final smoke/harness updates to parent integration
and forbids edits outside the six owned files. The parent must update
`tests/doc_drift.rs`, `gui/scripts/smoke.mjs`, and
`gui/scripts/session-index-smoke.mjs` to the frozen Japanese help/status
contract before the broad suite can pass. Restoring the old English/raw-enum
copy merely to satisfy those stale assertions would violate the acceptance
criteria.
