# data UAT #6（uat-test0715-data-006）計測レポート

## 結論

指定された6 runを新規計測ワークスペースで各1回だけ実行した。6/6が分類済みの `run_stop` と具体的な停止理由を残して正直終端したが、成功runおよび `full` は0/6だった。

事前宣言した判定は、P0-a **PASS**、P0-b **PASS**、P0-c **PASS（観測範囲に限定）**、P1-a **PASS**、P1-b **N/A** である。P0-cのDATA-11型再発は0件だった一方、最終phaseへ到達したrunが0件で、動的最終phaseの実verify集合は生成されなかった。このため「再発ゼロ」は確認できるが、noneアームでB-2hの最終束縛が実行されたことまでは本計測から主張しない。

Run 1ではB-2hのinspection行数照合が初めて実戦発火した。初回の `input_row_count=24` は `expected=60:reported=24` で失敗し、2回目のrepair後に60へ修正され `data_inspection_schema` がPASSした。E2も46/46 claimでPASSした。一方、Run 2は `data-inspection` で `write_required exhausted for output/inspection.json` に再到達し、DATA-10型が1/6で残った。

| 判定 | 結果 | 計測事実 |
|---|---:|---|
| P0-a: 6/6 正直終端 | **PASS** | 6/6に `run_stop`、`failure_kind=process_failure`、具体的な `stop_reason`、repair prompt/planがある。panic・分類不能終端・理由なき中断は0件 |
| P0-b: assurance契約§4準拠 | **PASS** | pipeline実行probe未完の5 runは `failed` または `static`。probe PASSのRun 1もE3未実行のため `failed` であり、`partial` / `full` のインフレは0件 |
| P0-c: DATA-11型ゼロ | **PASS（観測範囲）** | 最終phaseのinspection検査で落ちたrunは0件。profileのoverall planはinspection検査を最終から明示除外。ただし最終phase到達0/6、none最終step plan生成0/4のため動的実行経路は未実戦 |
| P1-a: E2偽陽性ゼロ維持 | **PASS** | 有効なreport/resultsのRun 1は46/46 PASS。date label 24件とreconciliation照合6件は全て `ok=true`。Run 6のE2 failureはresults不在でclaim抽出前のため偽陽性ではない |
| P1-b: nearest_miss修復注入 | **N/A** | 数値claim violationが0件で `nearest_miss` も0件。evidence値を埋めた修復注入の発火条件が成立しなかった |

## 計測条件

- 対象: `develop`、HEAD `859cd08bc435cfadb214a64ded090546cab73700`（`859cd08 Inject claims-binding nearest-miss guidance`、`origin/develop` と一致）
- バイナリ: `commandagent 0.1.0 859cd08 2026-07-15T09:27:08Z`（`+dirty` なし）
- release/install binary SHA-256: `94904e379a41e35aa3d13fe5de1baa55b5adcf92cf9b3a8f30784e0633e9e57b`（両者一致）
- 計測ワークスペース: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0715_data_006`。実行前に不存在を確認して新規作成
- 成果物先: `workspace/management/runs/uat-test0715-data-006/`
- 共通入力 SHA-256: `2f6c04e42b0ebdff85a7eb6b52a342610155be6796bd89e5729075d87c78d873`。生成直後と退避後の6/6で一致
- 共通 planner: `qwen3.6:27b-coding-nvfp4` / `ollama`
- 共通 profile/provider/context: `data` / `ollama` / `65536`
- 共通 goal: `data/sales.csv を読み込み、月次×地域の売上集計と全体合計を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。`
- 外側のcommandagent invocationは各run 1回、再試行0回。planner/executorが同一run内で行ったbounded repair/replanはeventsにそのまま残した
- 所要時間は各runの `time_profile.profile.total_ms`。6本合計は3622.252秒

### Preflight

計測前に存在した別タスクの未追跡ディレクトリ `workspace/management/runs/uat-test0715-ff1-001/` は、ユーザー確認後にstash `83e315c60dcc63c97d5ad6d49165a5b02abf1ff0`（`isolate uat-test0715-ff1-001 for data UAT #6`）へ一時退避した。その後の `git status --porcelain` が空であることを確認してpreflightを開始した。このstashは本UATコミットに含めず、提出コミット後に復元する。

| 項目 | 結果 |
|---|---|
| `git status --porcelain` | 上記の一時退避後に空 |
| `git log -1 --oneline` | `859cd08 Inject claims-binding nearest-miss guidance`（指定された `859cd08` 以降） |
| `cargo test` | exit 0。主要lib 1317 passed / 13 ignored、conformance 18 passed / 1 ignored、data profile 10/10を含むfull suite失敗0 |
| `cargo build --release` | exit 0 |
| install | `install -m 755 target/release/commandagent /Users/maenokota/.local/bin/commandagent` 成功 |
| `commandagent --version` | `commandagent 0.1.0 859cd08 2026-07-15T09:27:08Z` |

## Run行列

| # | run / run id | executor / preset | exit | terminal / final acceptance | assurance | 失敗クラス（主要終端） | 所要時間 |
|---:|---|---|---:|---|---|---|---:|
| 1 | `data6_qwen35_profile_001`<br>`019f651b-d4ec-7940-9de1-82f6b7cc06b3` | qwen3.6:35b-a3b-coding-nvfp4 / profile | 1 | failed / not_checked | failed (`data_assurance_failed`) | `data-cleaning`: `model_stagnation:read_only_loop`、step `implement-pipeline-main`、`verify_attempts=0` | 1159.819 s |
| 2 | `data6_gemma31_profile_001`<br>`019f652f-89c4-74d0-9e65-8a6f4267e162` | gemma4:31b / profile | 1 | failed / not_checked | failed (`data_profile_script_not_generated`) | `data-inspection`: `model_stagnation:read_only_loop: write_required exhausted for output/inspection.json` | 275.111 s |
| 3 | `data6_qwen35_none_001`<br>`019f6534-28a3-79b2-b089-fe384c976dc3` | qwen3.6:35b-a3b-coding-nvfp4 / none | 1 | failed / not_checked | failed (`data_profile_script_not_generated`) | 第1phase scaffold: corrective retries後も `verify step requires at least one verify command` | 612.916 s |
| 4 | `data6_qwen35_none_002`<br>`019f653e-2bbf-7162-91f5-0de4aa3b207f` | qwen3.6:35b-a3b-coding-nvfp4 / none | 1 | failed / not_checked | failed (`data_profile_script_not_generated`) | 第1phase scaffold: corrective retries後も `verify command may not use shell control syntax` | 504.656 s |
| 5 | `data6_gemma31_none_001`<br>`019f6546-2f1d-7a63-9645-ee35a1b3b4b2` | gemma4:31b / none | 1 | failed / not_checked | failed (`data_profile_script_not_generated`) | 第1phase scaffold: attempt 3/3 `planner_empty_response` | 500.401 s |
| 6 | `data6_gemma31_none_002`<br>`019f654e-1eb8-7423-b9df-29f97254ab7e` | gemma4:31b / none | 1 | failed / not_checked | static (`data_profile_probe_not_run`) | 第1phase execute: `artifact_follow_through_exhausted`; missing `output/results.json`, `output/report.md` | 569.349 s |

全runで `final_acceptance_status=not_checked` だった。完了phaseはRun 1の `data-inspection` だけで、他5 runは完了phase 0だった。

## DATA-11監査

### 正準計画（profileアーム）

profile 2本のoverall plan実物は同じ5 phaseで、最終phase `data-validation` のprompt原文は次のとおりである。

```text
Run only the final-bound pipeline, results-schema, reconciliation,
claims-binding, and rerun-consistency checks against the canonical artifacts.
data_inspection_schema belongs only to data-inspection and must not be carried
into final acceptance. Do not claim full assurance unless every final-bound
E1-E4 check passes from observed artifacts.
```

`data-inspection` の実step planでは、両profile runとも次のverify集合が生成された。

```text
anvil-catalog-check:data_inspection_schema
test -f output/inspection.json
```

Run 1の第2phase `data-cleaning` 実step planは `test -f pipeline/main.py` のみを束縛し、`data_inspection_schema` を含まない。両runとも第5phaseへ到達しなかったため、最終phaseのstep planファイル自体は存在しない。したがって、profileではoverall plan原文による明示除外と到達済み非inspection phaseでの非混入を確認したが、最終step planの実行確認は未到達である。

### 動的計画（noneアーム）

| run | overall planの最終phase | 最終phase prompt原文 | 最終step plan実物 |
|---|---|---|---|
| qwen35 none 1 | `generate-report-and-verify` | `Compile the aggregation table, overall total, and invalid-row summary into a final report at output/sales_summary_report.md, then verify that the report accurately reflects the computed values, exclusion counts, and required structure.` | **存在しない**（第1phase scaffoldで停止） |
| qwen35 none 2 | `final-verification` | `生成された要約レポートの構造と数値が集計ロジックと除外結果と完全に一致することを確認し、出力の決定性と安定性を検証する。` | **存在しない**（第1phase scaffoldで停止） |
| gemma31 none 1 | `generate-report-and-verify` | `Generate a summary report containing the aggregation table, overall total, and invalid row breakdown with reasons and counts, save it to reports/sales_summary.md, and verify that numerical totals match, row counts are consistent, and output is fully deterministic.` | **存在しない**（第1phase scaffoldで停止） |
| gemma31 none 2 | `verify-report-output` | `Validate the final report output/sales_summary_report.md to confirm it includes all required sections, matches the calculated aggregation and exclusion counts, and follows the expected markdown structure.` | **存在しない**（第1phase executeで停止） |

none 4本の退避済みplanを全文検索した結果、`data_inspection_schema` の出現は0件だった。ただしoverall planはチェック集合を列挙する形式ではなく、変換後の最終step planも4/4で未生成である。このため動的最終verify集合にinspection検査が含まれないことは本UATの実物から直接確認できない。

DATA-11型、すなわち「E系を通過した後の最終phaseで `data_inspection_schema` により停止」は0件だった。最終phase到達自体が0件なので、P0-cは事前宣言どおり再発件数0でPASSとする一方、実行カバレッジは未達と明記する。

## E2監査

`claims` は `claims-binding.json` の配列要素数、`ok` と `violations` は各要素の `ok` で集計した。results/report不在によるevidence失敗は数値claim違反と混同しない。

| run | claims | ok | violations | `claim_kind=date_label` | `reconciliation.*` 照合でok | `nearest_miss` | evidence結果 |
|---|---:|---:|---:|---:|---:|---|---|
| qwen35 profile | 46 | 46 | 0 | 24（全ok） | 6 | 0 | **PASS** |
| gemma31 profile | — | — | — | — | — | — | evidence不在 |
| qwen35 none 1 | — | — | — | — | — | — | evidence不在 |
| qwen35 none 2 | — | — | — | — | — | — | evidence不在 |
| gemma31 none 1 | — | — | — | — | — | — | evidence不在 |
| gemma31 none 2 | 0 | 0 | 0 | 0 | 0 | 0 | FAIL。`output/results.json` 不在のためclaim抽出前に停止 |
| **有効なreport/results 1 run計** | **46** | **46** | **0** | **24（全ok）** | **6** | **0** | 数値claim偽陽性0件 |

claim-level violationは0件である。Run 6のfailure原文は次の1件で、`claims=[]` の成果物不在エラーである。

```text
claims_binding_violation:invalid_results_schema:failed to read /Users/maenokota/share/work/localwork/commandagent_mvp/01/test0715_data_006/data6_gemma31_none_002/output/results.json: No such file or directory (os error 2)
```

Run 1の照合例は次のとおりである。

```text
report raw=60        -> reconciliation.input_rows=60
report raw=3         -> reconciliation.excluded_rows_total=3
report raw=40497.0   -> values.region_total_名古屋=40497.0
```

ISO月ラベル由来の24 tokenは全て `claim_kind="date_label"`, `matched_key=null`, `nearest_miss=null`, `ok=true` と監査記録された。reconciliation照合6件は `input_rows`、`used_rows`、除外合計、除外理由の3行で、すべて成功した。

数値claim violationがないため、evidence由来の「違反claim原文 / 最近傍キーと値 / 差分」を埋めたrepair promptは生成されていない。step instructionに常設されたnearest_miss形式の一般guidanceは存在するが、実データを埋めた発火とは数えない。よってP1-bはN/Aである。

## inspection監査

「字義例スキーマ準拠」は、`column_names` / `input_row_count` / `type_summaries` / `distinct_values` / `sample_rows` の5キー構造を持つかで機械的に判定した。

| run | `data_inspection_schema` | 行数照合 expected / reported | 修復回数 | `inspection.json` と字義例5キー構造 |
|---|---|---|---:|---|
| qwen35 profile | 初回5キー欠落 → 行数不一致 → **PASS** | `60 / 24` でFAIL後、`60 / 60` でPASS | schema repair 2回 | **準拠（5/5）**。最終値は入力ヘッダ3列、地域3値、実sample、`input_row_count=60` |
| gemma31 profile | **FAIL** | artifact不在のためN/A | write_required no-write 2/2、書き込み0 | 成果物なし |
| qwen35 none 1 | 未実行 | N/A | 0 | 成果物なし |
| qwen35 none 2 | 未実行 | N/A | 0 | 成果物なし |
| gemma31 none 1 | 未実行 | N/A | 0 | 成果物なし |
| gemma31 none 2 | 未実行 | キー不在かつcheck未実行のためN/A | schema repair 0。artifact follow-throughでpipelineとinspectionへのwrite各1回 | **非準拠（0/5）**。独自キー `columns`, `anomalies` |

Run 1の行数照合イベント原文は次のとおりである。

```text
data_inspection_schema:inspection_schema_violation:
input_row_count_mismatch:expected=60:reported=24
```

repair attempt 1は `output/inspection.json` を変更したが上記理由で `ok=false`、attempt 2は同じ対象を変更し `primary_reason=pass`, `ok=true` になった。最終evidenceは `input_path=data/sales.csv`, `status=pass` を記録している。

Run 2は `output/inspection.json` を一度も生成せず、evidenceのfailure原文は次のとおりである。

```text
inspection_schema_violation:inspection_path:path does not exist:
output/inspection.json
```

その後の `stage=write_required` は `selected_targets=["output/inspection.json"]`, `selection_reason="required_path"` を記録した。`Glob` と `Bash` が順に `read_only_tool_rejected` となり、`write_required_no_write_attempts=2/2` で枯渇した。これは事前定義したDATA-10型の再発1件である。

## E系 evidence

| run | `pipeline-run.json` | `reconciliation.json` | `claims-binding.json` | `rerun-consistency.json` |
|---|---|---|---|---|
| qwen35 profile | あり / **PASS**。`python3 -B pipeline/main.py`, exit 0, 78 ms | あり / **PASS**。`60 = 57 + 3`; `invalid_amount=1`, `invalid_date=1`, `missing_date=1` | あり / **PASS**。46/46 ok | なし |
| gemma31 profile | なし | なし | なし | なし |
| qwen35 none 1 | なし | なし | なし | なし |
| qwen35 none 2 | なし | なし | なし | なし |
| gemma31 none 1 | なし | なし | なし | なし |
| gemma31 none 2 | なし | あり / FAIL。results不在 | あり / FAIL。results不在、claims 0 | なし |

到達率は `pipeline-run` 1/6（PASS 1）、`reconciliation` 2/6（PASS 1）、`claims-binding` 2/6（PASS 1）、`rerun-consistency` 0/6である。Run 1の `results-schema.json` もPASSしたが、E3は未実行で最終受け入れには到達していない。

Run 1のreconciliationは勘定式 `60=57+3` を満たす。除外内訳は `invalid_amount=1`, `invalid_date=1`, `missing_date=1` で、入力の負数 `-500` はpipeline実装上numericとして採用されている。この段落は観測事実の記録であり、合否基準には追加しない。

## assurance監査

契約 `docs/data-profile-contract.md` §4に従い、`full` はE1〜E4全pass、`partial` はpipeline実行成功かつE1/E3 passでE2またはE4未達、`static` はスクリプト生成済みだが実行probe未完、`failed` は実行失敗・E1・再現性等の失敗として照合した。

| run | assurance / 根拠 | pipeline実行probe | 契約§4照合 | 準拠判定 |
|---|---|---:|---|---:|
| qwen35 profile | failed / `data_assurance_failed` | あり / PASS | E1/E2/results-schema PASSだがE3未実行、run自体はdata-cleaningで失敗 | 準拠（保守側） |
| gemma31 profile | failed / `data_profile_script_not_generated` | なし | pipelineなし、inspection不在 | 準拠 |
| qwen35 none 1 | failed / `data_profile_script_not_generated` | なし | scaffoldでpipeline未生成 | 準拠 |
| qwen35 none 2 | failed / `data_profile_script_not_generated` | なし | scaffoldでpipeline未生成 | 準拠 |
| gemma31 none 1 | failed / `data_profile_script_not_generated` | なし | scaffoldでpipeline未生成 | 準拠 |
| gemma31 none 2 | static / `data_profile_probe_not_run` | なし | pipelineは生成、probe/results/report未完 | 準拠 |

pipeline実行probe未完runの `partial` / `full` は0件、assuranceインフレは0件だった。

## 正準化・runtime policy・拒否イベント

| run | `verify_canonicalized` | `runtime_bash_policy` | runtime blocked | `inspect_command_normalized` | `read_only_tool_rejected` | `planner_error` |
|---|---:|---:|---:|---:|---:|---:|
| qwen35 profile | 2 | 7 | 0 | 0 | 0 | 0 |
| gemma31 profile | 1 | 4 | 0 | 3 | 2 | 0 |
| qwen35 none 1 | 3 | 0 | 0 | 0 | 0 | 4 |
| qwen35 none 2 | 3 | 0 | 0 | 0 | 0 | 4 |
| gemma31 none 1 | 1 | 0 | 0 | 0 | 0 | 4 |
| gemma31 none 2 | 2 | 10 | 0 | 1 | 0 | 1 |
| **合計** | **12** | **21** | **0** | **4** | **2** | **13** |

`runtime_bash_policy` 21件の `normalization_kind` は全て空で、runtime rewrite発火は0件だった。Run 1ではworkspace外の誤った絶対パスを含むBashが2件 `bash_path_confinement_rejected` になった。eventsに保存されたcommand原文は次のとおりである。

```text
cd /Users/<user>/share/work/commandagent_mvp/01/test0715_data_006/
data6_qwen35_profile_001 && ls -la

cd /Users/<user>/share/work/commandagent_mvp/01/test0715_data_006/
data6_qwen35_profile_001 && ls -la data/ 2>/dev/null ||
echo "data/ directory does not exist"
```

none 4本は全て第1phaseでplanner corrective retryを経験した。Run 3は最終的にverify command空、Run 4はshell control syntax、Run 5はempty response、Run 6は1回のverify command空の後に有効planを得た。

## UAT #5との対比

| 指標 / 死因クラス | UAT #5 | UAT #6 | 変化の事実 |
|---|---:|---:|---|
| 正直終端 | 6/6 | 6/6 | 不変 |
| full | 0/6 | 0/6 | 不変 |
| DATA-11型 | 1/6 | 0/6 | 1→0。ただし#6は最終phase到達0 |
| noneで最終phase到達 | 1/4 | 0/4 | 1→0。#6では動的最終束縛を実戦確認できず |
| E3 rerun-consistency | 1/6 PASS | 0/6 | 実戦発火なし |
| claims-binding PASS | 1/4存在 | 1/2存在 | PASS数は同じ。#6 Run 1は46/46 |
| E2 claim violations | 3（偽陽性0） | 0 | 地域合計キーが#6 Run 1のvaluesに存在し0件 |
| nearest_miss records | 3 | 0 | violation不在により#6は注入条件N/A |
| qwen profile inspection行数 | 24をschema PASS | 初回24をFAIL、repair後60でPASS | B-2h行数照合が実戦発火 |
| DATA-10型 | 1/6 | 1/6 | gemma profileで同じinspection write_required枯渇 |
| pipeline-run PASS | 2/6 | 1/6 | 1減 |

フェーズ深度は、profileではqwenがinspection完了後の第2phase、gemmaがinspection内で停止し、UAT #5と同じだった。noneではUAT #5のRun 3が最終第4phaseへ到達したのに対し、UAT #6は4/4が第1phaseで停止した。死因はplan scaffold 3件とartifact follow-through 1件である。

## full監査

`full` は0/6であり、E1〜E4全pass evidence、最終受け入れイベント列、rerun一致を完全転記する対象runは存在しない。

## イベント語彙（6 run統合）

```text
   6 artifact_stagnation_feedback
   2 bash_path_confinement_rejected
   6 empty_response_escalation
   3 empty_response_recovered
   2 escalation_carryover
   6 host_env_contamination
   4 inspect_command_normalized
  10 loop_stop
   1 phase_verification_result
   6 plan_preset_resolved
  13 planner_error
  22 planner_quality_issue
  14 planner_raw_output_shape
   6 preset_step_converted
   2 preset_ultra_plan_used
  62 provider_turn_duration
   6 read_only_stagnation_feedback
   2 read_only_tool_rejected
   7 recovery_prompt_saved
   6 run_start
   6 run_stop
  21 runtime_bash_policy
  10 step_obligation_scope
   9 step_prompt_contract
   1 step_short_circuited
   1 step_verify_failure
   2 step_verify_repair
   6 time_profile
  46 tool_call_raw
  42 tool_execute
   2 tool_validation_error
   6 tui_command_stop
   6 ultra_context_initialized
   6 ultra_partial_artifact_summary
   1 ultra_phase_complete
   7 ultra_phase_context_attached
   4 ultra_phase_context_updated
   1 ultra_phase_execute_complete
   6 ultra_phase_failed
   4 ultra_phase_plan_validated
   1 ultra_phase_profile_check
   4 ultra_phase_scaffold_complete
   7 ultra_phase_start
   4 ultra_plan_generation_attempt
   4 ultra_plan_generation_metadata_normalized
   4 ultra_plan_generation_succeeded
   4 ultra_plan_raw_output_shape
  12 verify_canonicalized
```

## 成果物と不在資料

各runに `.anvil/` 一式と `data/sales.csv`、`data/sales.csv.sha256` を退避した。`pipeline/`、`output/`、`evidence/` は存在したrunのみ実物を退避し、存在しないものは補完していない。

| run | pipeline | output | evidence | `.anvil` events/summary/repair/plan |
|---|---|---|---|---|
| qwen35 profile | あり | inspection/results/reportあり | inspection/results/pipeline/reconciliation/claimsあり | あり |
| gemma31 profile | なし | なし | inspection-schema failureのみ | あり |
| qwen35 none 1 | なし | なし | なし | あり |
| qwen35 none 2 | なし | なし | なし | あり |
| gemma31 none 1 | なし | なし | なし | あり |
| gemma31 none 2 | main.pyあり | inspectionのみ | results/reconciliation/claims failureあり | あり |

退避後、6本のCSV SHA一致と全 `events.jsonl` / evidence JSON / output JSONのJSON parse成功を再確認した。
