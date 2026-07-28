# INGEST-4 ingest create 機械床全数監査

監査日: 2026-07-28 (JST)

対象revision: `541ad6b` (`Synthesize ingest create plans`)

基準: `docs/ingest-profile-contract.md` (fixed 2026-07-25)

## 結論

ingest createの**既定production経路**で、executor modelとN1〜N5 runtimeの
間に置く床を全数走査した。`planner由来`は0行である。UltraPlanの段構成、
各phaseのStepPlan、成果物所有権、guidance、verify command、構造gate、
N runtime起動までmachineが先に確定する。

executor modelは`ingest-implement`の固定instructionに従って納品物の内容を
生成する。これはprofileの生成主体であって「計画床のplanner由来」ではない。
明示的な`--plan-preset none`はoperator opt-outとして従来互換のplanner経路を
選べるが、suiteの`plan_preset = "default"`を含む既定production経路ではない。

## 監査表

正準性の語彙は`機械固定`、`字義例配布済み`、`planner由来`の3種とする。
状態`closed`は、production bindingと焦点testが存在することを示す。

| 床 | 正準性 | production binding | 状態・機械確認 |
|---|---|---|---|
| preset選択 | 機械固定 | `src/config.rs`の`default_create_ingest` | `ingest/create`は`PlanPreset::Profile`、明示`none`だけが優先。config両側testでclosed |
| UltraPlan源 | 機械固定 | `src/planner/ultra_preset.rs`→ingest manifest | profile preset使用eventは`planner_skipped=true`。closed |
| phase順序 | 機械固定 | `src/planner/profiles/ingest/manifest.toml` / `manifest::PHASE_IDS` | `implement → run → structural gate`の3段。manifest検証でclosed |
| StepPlan dispatch | 機械固定 | `src/planner/phase_plan_synthesis.rs`→`ingest_plan_synthesis.rs` | ingest presetをplanner fallbackより先に解決。fallback panic test 3/3でclosed |
| implement instruction | 字義例配布済み | `guidance::GENERATION_RULES`を固定instructionへ同梱 | selector、inspection、recordsの全字義形と「実snapshot値へ置換」を先行配布。closed |
| 納品物所有権 | 機械固定 | `IMPLEMENT_PATHS` | `pipeline/main.py`、`output/inspection.json`、`output/records.json`、`output/report.md`の4件だけをimplementが所有。closed |
| verifier artifact排除 | 機械固定 | 同じ`IMPLEMENT_PATHS`と計画snapshot | `smoke-check.py` / `verify_pipeline.py` / `smoke-check.js` / `verify-artifacts.js`を表現不能化。elev-003実測fixture両側でclosed |
| run command | 機械固定 | `RUN_COMMAND` | `python3 -B pipeline/main.py`の1本。planner代替commandなし。closed |
| StepPlan finalizer | 機械固定 | `step_plan_finalize::finalize_step_plan_for_execution` | 合成した3phaseすべてを既存repair+lint chokepointへ通す。closed |
| structural gate | 機械固定＋字義例配布済み | `profiles/ingest/phase_verify.rs` | 内部command`anvil-ingest-check:phase_structure`。構造だけを検査し意味検証をNへ委譲。closed |
| expected-path実行検査 | 機械固定 | generic StepPlan ownership/lint/verification | verify stepはpathを所有せず、implementの4件だけを検査。canonical snapshotでclosed |
| command分類 | 機械固定 | `planner/verify/dependency_classification.rs` | workspace script実行とdependency導入を分離するINGEST-1境界を使用。planner入力なし。closed |
| 実行型進捗 | 機械固定 | `minimal_loop/execution_progress.rs` | 相異なるexit 0 commandだけを進捗とし、反復・失敗・read-onlyは非進捗。planner入力なし。closed |
| profile構造確認 | 機械固定 | `IngestProfile::verify_final` | 4納品物と1件以上のsnapshot入力を確認。closed |
| final acceptance起動 | 機械固定 | `profile_behavior::run`→`IngestProfile::behavior_probe` | production acceptanceから`runtime::run_manifest_checks`を起動する既存実在testでclosed |
| N1〜N5束縛 | 機械固定 | ingest manifest catalog | N1 pipeline probe、N2 source binding、N3 accounting、N4 format、N5 rerunのIDとadapterを固定。closed |
| selector/candidate freeze | 機械固定 | `ingest/runtime.rs`→`accounting::freeze` | N1前にselectorと候補集合をfreezeし、N2〜N5へ同じlineageを渡す。closed |
| N evidence/assurance | 機械固定 | `ingest/runtime.rs` | N別evidenceを書き、固定§assurance写像でfull/partial/static/failedを分類。closed |
| final repair対象 | 機械固定 | `IngestProfile::source_paths` / `evidence_repair_target_paths` | `pipeline/main.py`と`output/inspection.json`へprofile境界を固定。repair内容はexecutor model、planner計画床ではない。closed |
| completion投影 | 機械固定 | `completion_metadata/ingest.rs` | runtime evidenceから同じ§assurance写像を再構成。runtime-shaped fixtureでclosed |
| admission cap | 機械固定 | `profile_admission.rs` + manifest `status=draft` | earned full/partialを表示staticへcap。admission offを維持。closed |

集計:

- 機械固定（`機械固定＋字義例配布済み`を含む）: 21床
- 字義例配布済みを含む床: 2床
- planner由来: **0床**
- open / unknown: **0床**

## elev-003 gapとの対応

実測一次資料は
`tests/fixtures/ingest-plan-synthesis/elev-003-gaps.yaml`へsha256付きで固定した。

| elev-003実測形 | INGEST-4の構造的遮断 |
|---|---|
| `smoke-check.js`がexpected pathへ残存 | implement ownershipを固定4納品物へ閉じた |
| `verify-artifacts.js`がexpected pathへ残存 | 同上。拡張子別の事後filterへ依存しない |
| verify段で`Repair or finalize pipeline/main.py`を要求 | run/structural gateのinstructionとcommandをmachine固定し、変更要求を表現不能化 |
| plannerのphase名・段数がrunごとに変動 | manifestの3段と各StepPlanをproduction dispatchで固定 |

従ってINGEST-4は、個別の`.js` path filter追加ではなく、plannerが
expected_paths・verify・段構成を発明しない構造へ転回して解消した。
