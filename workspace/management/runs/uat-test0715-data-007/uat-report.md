# data UAT #7 兼 preset A/B（uat-test0715-data-007）計測レポート

## 結論

指定された6 runを新規計測ワークスペースで、指定順 1→4→2→5→3→6、各1回だけ実行した。6/6が分類済みの `run_stop` と具体的な停止理由を残して正直終端した。Run 3（gemma31 / profile）とRun 4（qwen35 / none）が exit 0、全phase完走、`final_acceptance_status=full_success`、E1〜E4全PASSとなり、data profileで初めての `full` を2本記録した。

事前宣言した判定は、P0-a **PASS**、P0-b **PASS（保守側projectionを注記）**、P0-c **PASS** である。DATA-12は `step_short_circuited` 19件、`verify_default_bound` 8件が発火した。`expected_paths` が全存在し、実verifyがPASS可能なstepでのread-only枯渇は0件だった。

preset A/Bはprofileアームとnoneアームがともに「完走1/3、full 1/3」で同率だった。事前ルールの「profileがnoneを上回る」を満たさないため、判定は **dataのデフォルトplan-presetを現状のまま維持**、この問いへの追加計測は行わない、である。UAT #4〜#6のnone 12本・完走0は補強事実として併記するが、固定コード上の本A/B判定を置き換えない。

| 判定 | 結果 | 計測事実 |
|---|---:|---|
| P0-a: 6/6 正直終端 | **PASS** | 6/6に `run_stop`。completed 2、具体的理由付きfailed 4。panic・分類不能終端・理由なきinterruptedは0 |
| P0-b: assurance契約§4準拠 | **PASS（保守側）** | full 2本は `data-assurance.json` と `ultra_final_acceptance` が `full`、E1〜E4全PASS。他4本はfailed 3 / static 1でインフレ0。外側の `tui_command_stop/run_stop` はfull 2本を `partial:completion_contract_not_bound` と保守側に投影したため、その差も原文記録 |
| P0-c: DATA-12型ゼロ | **PASS** | 短絡19件の全てが `reason=pre_satisfied_verified`、実verify全PASS。条件成立stepでのread-only枯渇0。失敗4本は成果物不在、tool引数エラー、artifact不足 |
| A/B | **現状デフォルト維持** | profile=(完走1, full 1)、none=(完走1, full 1)で同率。noneの終端死因もplanner scaffold優勢ではない |
| 記録: full率 | **2/6** | profile 1/3、none 1/3 |
| 記録: DATA-10 | qwen profileで2件、gemma profileで0件 | Run 1 / 2がinspection write_required枯渇。Run 3 gemmaはinspectionを修復して完走 |

## 計測条件

- 対象: `develop`、HEAD `7b177fe16536f61b47b14f6736918dcc7547e4a9`（`7b177fe Bind default data verification`、計測開始時に `origin/develop` と一致）
- バイナリ: `commandagent 0.1.0 7b177fe 2026-07-15T12:11:05Z`（`+dirty` なし）
- release/install binary SHA-256: `a977598e18c255466908ee2e2fe4c7e3b93b7ad346a64d0e543c5bb2e08e953a`（両者一致）
- 計測ワークスペース: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0715_data_007`。実行前に不存在を確認して新規作成
- 成果物先: `workspace/management/runs/uat-test0715-data-007/`
- 共通入力 SHA-256: `2f6c04e42b0ebdff85a7eb6b52a342610155be6796bd89e5729075d87c78d873`。生成直後と退避後の6/6で一致
- 共通 planner: `qwen3.6:27b-coding-nvfp4` / `ollama`
- 共通 profile/provider/context: `data` / `ollama` / `65536`
- 共通 goal: `data/sales.csv を読み込み、月次×地域の売上集計と全体合計を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。`
- 外側のcommandagent invocationは各run 1回、再試行0回。planner/executorが同一run内で行ったbounded repair/replanはeventsにそのまま残した
- 実行順: Run 1 → 4 → 2 → 5 → 3 → 6
- 所要時間は各runの `time_profile.profile.total_ms`。6本合計は4953.315秒

### Preflight

計測前に存在した別タスクの未追跡ディレクトリ `workspace/management/runs/uat-test0715-ff1-001/` は、ユーザー確認後にstash commit `a591114d16452d648f86527c4f8c1f4c6633ac5f`（`On develop: uat-test0715-data-007-preflight`）へ一時隔離した。その後の `git status --porcelain` が空であることを確認した。このstashは本UATコミットに含めず、提出コミット後に復元する。

| 項目 | 結果 |
|---|---|
| `git status --porcelain` | 上記の一時隔離後に空 |
| `git log -1 --oneline` | `7b177fe Bind default data verification` |
| `cargo test` | 通常権限ではbrowser probeの `Operation not permitted` を検出したため無効扱い。指示どおり権限付きで再実行しexit 0。主要lib 1324 passed / 13 ignored、conformance 18 passed / 1 ignored、data profile 10/10を含むfull suite失敗0 |
| `cargo build --release` | exit 0 |
| install | `install -m 755 target/release/commandagent /Users/maenokota/.local/bin/commandagent` 成功 |
| `commandagent --version` | `commandagent 0.1.0 7b177fe 2026-07-15T12:11:05Z` |

## Run行列

「assurance」は `data-assurance.json / ultra_final_acceptance` を契約§4の主値とし、外側terminal projectionを括弧内に併記した。

| # | run / run id | executor / preset | exit | terminal / final acceptance | assurance | 到達phase | 失敗クラス（主要終端） | 所要 |
|---:|---|---|---:|---|---|---:|---|---:|
| 1 | `data7_qwen35_profile_001`<br>`019f65b2-bb0f-7032-952c-3348527dff65` | qwen3.6:35b-a3b-coding-nvfp4 / profile | 1 | failed / not_checked | failed (`data_profile_script_not_generated`) | 0/5 | `data-inspection`: `model_stagnation:read_only_loop: write_required exhausted for output/inspection.json`、step `inspect-workspace` | 280.151 s |
| 2 | `data7_qwen35_profile_002`<br>`019f65cf-2350-7c33-b4b9-3a903c3493cb` | qwen3.6:35b-a3b-coding-nvfp4 / profile | 1 | failed / not_checked | failed (`data_profile_script_not_generated`) | 0/5 | `data-inspection`: 同じwrite_required枯渇、step `run-inspection` | 242.272 s |
| 3 | `data7_gemma31_profile_001`<br>`019f65d7-fd53-7382-8b0e-0caa4ecf6d1c` | gemma4:31b / profile | 0 | completed / **full_success** | **full**（terminalはpartial / `completion_contract_not_bound`） | **5/5** | completed | 2029.719 s |
| 4 | `data7_qwen35_none_001`<br>`019f65b8-0a60-7b43-98c1-40f6aeb37997` | qwen3.6:35b-a3b-coding-nvfp4 / none | 0 | completed / **full_success** | **full**（terminalはpartial / `completion_contract_not_bound`） | **3/3** | completed | 1463.800 s |
| 5 | `data7_qwen35_none_002`<br>`019f65d3-ae61-7b81-b96d-9d5f871768b1` | qwen3.6:35b-a3b-coding-nvfp4 / none | 1 | failed / not_checked | failed (`data_profile_script_not_generated`) | 0/3 | `validate-and-clean-data`: `recoverable tool error repeated: missing_arg` | 234.149 s |
| 6 | `data7_gemma31_none_001`<br>`019f65f7-b518-7241-b015-9a0e7272c241` | gemma4:31b / none | 1 | failed / not_checked | static (`data_profile_probe_not_run`) | 0/4 | `load-and-inspect-data`: `artifact_follow_through_exhausted`、不足 `output/results.json` / `output/report.md` / `check_outputs.py` | 703.224 s |

## DATA-12監査

### 発火集計

| run | `step_short_circuited` | 対象step | `verify_default_bound` | 既定束縛対象 |
|---|---:|---|---:|---|
| qwen35 profile 1 | 0 | — | 0 | — |
| qwen35 profile 2 | 0 | — | 0 | — |
| gemma31 profile | **16** | cleaning 2、aggregation 6、reporting 4、validation 4 | **4** | inspection / cleaning / aggregation / reporting各1 |
| qwen35 none 1 | **3** | 各phaseのverify 1件ずつ | **2** | aggregation phase内2件 |
| qwen35 none 2 | 0 | — | **1** | `validate-and-clean-data:verify-outputs` |
| gemma31 none | 0 | — | **1** | `load-and-inspect-data:verify-artifacts` |
| **合計** | **19** | implement 6 / verify 13 | **8** | — |

19件は全て `reason=pre_satisfied_verified`、`verification_summary.status=pass`、`failure_count=0` だった。合計でexpected path 34件を存在確認し、verify command 56本を実行してから短絡した。推定によるskipは0件である。

### Run 1型の追跡

Run 3の `data-cleaning:implement-pipeline` は、step開始前に `required_paths=["pipeline/main.py"]` が存在し、`test -f pipeline/main.py` を実行してPASSしたため、モデルターンなしで次へ進んだ。

```json
{"event":"step_short_circuited","phase_scope":"data-cleaning","step_id":"implement-pipeline","step_kind":"implement","reason":"pre_satisfied_verified","required_paths":["pipeline/main.py"],"verification_summary":{"expected_paths_checked":1,"verify_commands_executed":1,"failure_count":0,"status":"pass"},"verify_commands":["test -f pipeline/main.py"]}
```

直後の `data-cleaning:verify-artifacts` も同じ実verifyで短絡し、`execute-pipeline` へ流れ、phaseを完了した。これはB-2iの根拠となったUAT #6 Run 1型、すなわち「成果物が先行完成し、実行すれば即PASSするverifyを持つstep」の実戦発火である。

### full 2本の短絡列

Run 3（gemma profile）の16件:

| phase | step（kind） | expected paths checked / verify commands |
|---|---|---:|
| data-cleaning | `implement-pipeline` (implement), `verify-artifacts` (verify) | 1/1、1/1 |
| data-aggregation | `inspect-workspace` (verify), `implement-pipeline` (implement), `verify-syntax` (verify), `run-pipeline` (implement), `verify-results-json` (verify), `verify-report-md` (verify) | 2/2、2/2、2/2、3/2、2/2、2/2 |
| data-reporting | `inspect-state` (verify), `implement-pipeline` (implement), `run-pipeline` (implement), `verify-output` (verify) | 1/1、1/1、2/1、1/1 |
| data-validation | `inspect-current-artifacts` (verify), `run-pipeline` (implement), `verify-schema-reconciliation` (verify), `verify-claims-rerun` (verify) | 4/9、4/9、4/9、0/2 |

Run 4（qwen none）の3件:

| phase | step（kind） | expected paths checked / verify commands | verify原文 |
|---|---|---:|---|
| data-ingestion-and-validation | `verify-results` (verify) | 0/1 | `python pipeline/main.py` |
| sales-aggregation-and-calculation | `verify-results` (verify) | 2/2 | `test -f pipeline/main.py`、`test -f output/results.json` |
| summary-report-generation | `verify-artifacts` (verify) | 0/6 | `python pipeline/main.py`、3成果物の `test -f`、`data_results_schema`、`data_reconciliation` |

### 空verifyの既定束縛

全 `verify_default_bound` 原文の機械転記:

| run / phase / step | `bound_checks` |
|---|---|
| Run 3 / data-inspection / `verify-inspection` | `anvil-catalog-check:data_inspection_schema`; `test -f output/inspection.json` |
| Run 3 / data-cleaning / `verify-artifacts` | `test -f pipeline/main.py` |
| Run 3 / data-aggregation / `verify-results-schema` | `test -f pipeline/main.py`; `test -f output/results.json` |
| Run 3 / data-reporting / `verify-output` | `test -f output/report.md` |
| Run 4 / sales-aggregation-and-calculation / `verify-outputs` | `test -f pipeline/main.py`; `test -f output/results.json` |
| Run 4 / sales-aggregation-and-calculation / `verify-results` | 同上 |
| Run 5 / validate-and-clean-data / `verify-outputs` | `test -f pipeline/main.py` |
| Run 6 / load-and-inspect-data / `verify-artifacts` | `anvil-catalog-check:data_inspection_schema`; `test -f output/inspection.json` |

Run 5は既定束縛stepに到達する前にtool引数エラーで停止し、Run 6は同step到達前に成果物follow-throughで停止した。無条件の空passは観測されていない。

### DATA-12型の有無

失敗4本の条件を確認した。

- Run 1 / 2: 短絡対象の `output/inspection.json` が存在せず、`expected_paths全存在` を満たさない。
- Run 5: `Glob` の `missing_arg` 反復で停止し、事前充足stepではない。
- Run 6: `output/results.json`、`output/report.md`、`check_outputs.py` が不在で、事前充足stepではない。

したがって、「expected_paths全存在＋実verify全PASS可能」なのにモデルReadだけで枯渇したDATA-12型は0件である。

## preset A/B対比

| 指標 | profile（Run 1〜3） | none（Run 4〜6） |
|---|---:|---:|
| run数 | 3 | 3 |
| 完走 | **1** | **1** |
| full | **1** | **1** |
| 最終受け入れ到達 | **1** | **1** |
| 完了phase深度分布 | 0 phase: 2、5 phase: 1 | 0 phase: 2、3 phase: 1 |
| 死因分布 | full 1、inspection write_required枯渇 2 | full 1、tool `missing_arg` 1、artifact follow-through 1 |
| planner_error発火run | 1/3 | 2/3 |

profileの（完走数, full数）=(1,1)、none=(1,1)で、profileはnoneを上回らない。またnoneの失敗2本は、実行可能なstep plan生成後のtool引数エラーと成果物follow-throughであり、terminal死因はplanner scaffold起因優勢ではない。

事前宣言に従う判定:

```text
dataのデフォルトplan-preset=profile: admitted候補にしない
dataのデフォルトplan-preset: 現状維持
この問いへの追加計測: 行わない
```

補強証拠として、UAT #4 / #5 / #6のnoneは各0/4完走、合計12/12未完走だった。本UATを加えた履歴合計はnone 1/15完走だが、本A/Bは固定コード上の同一条件3対3を事前ルールどおり主判定に用いた。

## E系 evidence

| run | E1 `reconciliation` | E2 `claims-binding` | E3 `rerun-consistency` | E4 `results-schema` / final-bound checks | pipeline probe |
|---|---|---|---|---|---|
| qwen35 profile 1 | なし | なし | なし | なし | なし |
| qwen35 profile 2 | なし | なし | なし | なし | なし |
| gemma31 profile | **PASS**: `60 = 57 + 3` | **PASS**: 26/26 | **PASS** | **PASS** / final phase PASS | **PASS**, exit 0, 54 ms |
| qwen35 none 1 | **PASS**: `60 = 57 + 3` | **PASS**: 46/46 | **PASS** | **PASS** / final phase PASS | **PASS**, exit 0, 56 ms |
| qwen35 none 2 | なし | なし | なし | なし | なし |
| gemma31 none | なし | なし | なし | なし | なし |

到達率はE1〜E4およびpipeline probeがそれぞれ2/6、PASS 2/2である。

## E2監査

| run | claims | ok | violations | `claim_kind=date_label` | `reconciliation.*` 照合ok | `nearest_miss` |
|---|---:|---:|---:|---:|---:|---:|
| gemma31 profile | 26 | 26 | 0 | 14 | 4 | 0 |
| qwen35 none 1 | 46 | 46 | 0 | 24 | 6 | 0 |
| 他4本 | — | — | — | — | — | — |
| **有効な2本合計** | **72** | **72** | **0** | **38** | **10** | **0** |

日付ラベル38件は全て `claim_kind="date_label"`、`matched_key=null`、`nearest_miss=null`、`ok=true`。reconciliation照合10件も全てPASSした。violationが0件なので、nearest_missを埋めた修復ガイダンスの実戦発火はN/Aである。

数量claimは全件を次に転記する。列は `byte_offset / raw / matched_key / matched_result_value`。全行 `ok=true`、`nearest_miss=null`。

Run 4:

```text
66  / 60     / reconciliation.input_rows                  / 60.0
82  / 57     / reconciliation.used_rows                   / 57.0
102 / 3      / reconciliation.excluded_rows_total         / 3.0
162 / 1      / reconciliation.excluded[0].rows            / 1.0
183 / 1      / reconciliation.excluded[0].rows            / 1.0
204 / 1      / reconciliation.excluded[0].rows            / 1.0
316 / 19990  / 2026-01_東京                                / 19990.0
345 / 18657  / 2026-02_大阪                                / 18657.0
377 / 20730  / 2026-03_名古屋                              / 20730.0
406 / 16824  / 2026-04_東京                                / 16824.0
435 / 21470  / 2026-05_大阪                                / 21470.0
467 / 19767  / 2026-06_名古屋                              / 19767.0
501 / 117438 / grand_total                                 / 117438.0
561 / 40497  / total_名古屋                                / 40497.0
580 / 40127  / total_大阪                                  / 40127.0
599 / 36814  / total_東京                                  / 36814.0
656 / 19990  / 2026-01_東京                                / 19990.0
676 / 18657  / 2026-02_大阪                                / 18657.0
696 / 20730  / 2026-03_名古屋                              / 20730.0
716 / 16824  / 2026-04_東京                                / 16824.0
736 / 21470  / 2026-05_大阪                                / 21470.0
756 / 19767  / 2026-06_名古屋                              / 19767.0
```

Run 3:

```text
67  / 60         / reconciliation.input_rows          / 60.0
83  / 57         / reconciliation.used_rows           / 57.0
115 / 2          / reconciliation.excluded[0].rows    / 2.0
144 / 1          / reconciliation.excluded[1].rows    / 1.0
175 / 122,938.00 / grand_total                        / 122938.0
237 / 19,990.00  / regional_東京_2026-01              / 19990.0
267 / 18,657.00  / regional_大阪_2026-02              / 18657.0
297 / 5,000.00   / regional_東京_2026-02              / 5000.0
329 / 20,730.00  / regional_名古屋_2026-03            / 20730.0
359 / 17,324.00  / regional_東京_2026-04              / 17324.0
389 / 21,470.00  / regional_大阪_2026-05              / 21470.0
422 / 19,767.00  / regional_名古屋_2026-06            / 19767.0
```

## assurance監査

契約 `docs/data-profile-contract.md` §4に従い、fullはE1〜E4全PASS、partialはpipeline実行成功＋E1/E3 PASSでE2またはE4未達、staticはscript生成済みだがprobe未完、failedは実行失敗・E1違反・再現性違反として照合した。

| run | data assurance / 根拠 | pipeline probe | E1〜E4 | 契約§4判定 |
|---|---|---|---|---:|
| qwen35 profile 1 | failed / script不在 | なし | 未到達 | 準拠 |
| qwen35 profile 2 | failed / script不在 | なし | 未到達 | 準拠 |
| gemma31 profile | **full** / reasons空 | PASS | 全PASS | 準拠 |
| qwen35 none 1 | **full** / inspection failureは最終gate外 | PASS | 全PASS | 準拠 |
| qwen35 none 2 | failed / script不在 | なし | 未到達 | 準拠 |
| gemma31 none | static / `data_profile_probe_not_run` | なし | 未到達 | 準拠 |

full 2本の `data-assurance.json` と `ultra_final_acceptance` はともに `assurance_level=full` で契約§4と一致した。一方、共通の外側projectionは次の保守側表示だった。

```text
ultra_plan_complete: assurance_level=static, assurance_reason=data_profile_probe_not_run
tui_command_stop:   assurance_level=partial, assurance_reason=completion_contract_not_bound
run_stop:           assurance_level=partial, assurance_reason=completion_contract_not_bound
```

これはfullへのインフレではなくunder-projectionである。run行列では契約に直接対応するdata assuranceを主値とし、外側projectionを隠さず併記した。

## full監査（歴史的記録）

全evidence JSON原文は各full runの `artifacts/<run>/evidence/` に退避した。以下はE1〜E4、pipeline probe、rerun値、report照合、final acceptanceイベント列を、第三者が退避物だけで追跡できる形で転記した。

### Run 4: data7_qwen35_none_001

#### pipeline probe

```text
capability_id=pipeline_probe
status=pass, ok=true, outcome=exited, exit_code=0, duration_ms=56
command=["python3","-B","pipeline/main.py"]
stdout="Pipeline complete: 60 input rows, 57 used, 3 excluded\n"
stderr=""
artifact hashes (fnv1a64):
  output/inspection.json  bytes=754  66fabbaa4d813e68
  output/report.md        bytes=764  bf32b41daef7bc41
  output/results.json     bytes=814  3fc3447ab5e15032
isolation=workspace_cwd_env_allowlist_bounded_offline_policy
bounded_timeout_ms=30000, offline_policy_applied=true
failure_kinds=[], capture_warnings=[]
```

#### E1 reconciliation

```json
{"status":"pass","ok":true,"input_rows":60,"used_rows":57,"excluded":[{"reason":"invalid_amount","rows":1},{"reason":"invalid_date","rows":1},{"reason":"missing_date","rows":1}],"excluded_rows":3,"equation":"60 = 57 + 3","failure_kinds":[]}
```

#### E2 claims binding

```text
status=pass, ok=true
claims=46, ok=46, violations=0
date_label=24, reconciliation matches=6, nearest_miss=0
failure_kinds=[]
```

数量claim 22件はE2監査節に全件転記済み。3照合例:

```text
report "Input rows: 60"       -> reconciliation.input_rows=60
report "Grand total: 117438"  -> values.grand_total=117438.0
report "名古屋 | 40497"       -> values.total_名古屋=40497.0
```

#### E3 rerun consistency

baselineとrerunの `reconciliation` / `values` は同一だった。

```json
{"reconciliation":{"input_rows":60,"used_rows":57,"excluded":[{"reason":"invalid_amount","rows":1},{"reason":"invalid_date","rows":1},{"reason":"missing_date","rows":1}]},"values":{"2026-01_東京":19990.0,"2026-02_大阪":18657.0,"2026-03_名古屋":20730.0,"2026-04_東京":16824.0,"2026-05_大阪":21470.0,"2026-06_名古屋":19767.0,"grand_total":117438.0,"total_2026-01":19990.0,"total_2026-02":18657.0,"total_2026-03":20730.0,"total_2026-04":16824.0,"total_2026-05":21470.0,"total_2026-06":19767.0,"total_名古屋":40497.0,"total_大阪":40127.0,"total_東京":36814.0}}
```

`status=pass`、`ok=true`、`pipeline_run_ok=true`、`failure_kinds=[]`。

#### E4 / final-bound checks

`results-schema.json` は `status=pass`、`ok=true`、`results_path=output/results.json`、`error=null`。`data-assurance.json` は `status=full`、`assurance=full`、final-boundの `data_claims_binding` / `data_reconciliation` / `data_rerun_consistency` / `data_results_schema` / `pipeline_probe` が全てtrue。

工程成果物の `inspection-schema.json` は5キー欠落でfailedだったが、最終受け入れのE1〜E4束縛外であり、`data-assurance.json` にも理由を残したままfullとなった。

```text
inspection_schema_violation:missing_keys:
column_names,input_row_count,type_summaries,distinct_values,sample_rows
```

#### final acceptanceイベント列

```text
phase_verification_result phase=data-ingestion-and-validation mode=intermediate_invariant ok=true
phase_verification_result phase=sales-aggregation-and-calculation mode=intermediate_invariant ok=true
phase_verification_result phase=summary-report-generation mode=intermediate_invariant ok=true
phase_verification_result phase=summary-report-generation mode=final_acceptance ok=true
ultra_phase_profile_check phase=summary-report-generation final_phase=true ok=true
ultra_phase_complete phase=summary-report-generation final_phase=true ok=true
profile_behavior_probe status=pass ok=true
ultra_final_acceptance final_acceptance_status=full_success assurance_level=full primary_reason=pass release_gate_status=pass runtime_acceptance_status=pass
final_acceptance_deterministic_remedies deterministic_remedies_applied=[]
ultra_plan_complete ok=true total_phases=3
tui_command_stop status=completed task_status=complete final_acceptance_status=full_success
run_stop status=completed task_status=complete final_acceptance_status=full_success
```

`step_short_circuited` は3件で、全件をDATA-12節に転記済み。

### Run 3: data7_gemma31_profile_001

#### pipeline probe

```text
capability_id=pipeline_probe
status=pass, ok=true, outcome=exited, exit_code=0, duration_ms=54
command=["python3","-B","pipeline/main.py"]
stdout="", stderr=""
artifact hashes (fnv1a64):
  output/inspection.json  bytes=371  dccf1f195abdf0e2
  output/report.md        bytes=431  634ecc8fba6eb961
  output/results.json     bytes=567  be6f10f828ad0da4
isolation=workspace_cwd_env_allowlist_bounded_offline_policy
bounded_timeout_ms=30000, offline_policy_applied=true
failure_kinds=[], capture_warnings=[]
```

#### E1 reconciliation

```json
{"status":"pass","ok":true,"input_rows":60,"used_rows":57,"excluded":[{"reason":"invalid_amount","rows":2},{"reason":"invalid_date","rows":1}],"excluded_rows":3,"equation":"60 = 57 + 3","failure_kinds":[]}
```

#### E2 claims binding

```text
status=pass, ok=true
claims=26, ok=26, violations=0
date_label=14, reconciliation matches=4, nearest_miss=0
failure_kinds=[]
```

数量claim 12件はE2監査節に全件転記済み。3照合例:

```text
report "Total input rows: 60"       -> reconciliation.input_rows=60
report "Grand Total: 122,938.00"    -> values.grand_total=122938.0
report "2026-02 | 東京: 5,000.00"   -> values.regional_東京_2026-02=5000.0
```

#### E3 rerun consistency

baselineとrerunの `reconciliation` / `values` は同一だった。

```json
{"reconciliation":{"input_rows":60,"used_rows":57,"excluded":[{"reason":"invalid_amount","rows":2},{"reason":"invalid_date","rows":1}]},"values":{"grand_total":122938.0,"regional_名古屋_2026-03":20730.0,"regional_名古屋_2026-06":19767.0,"regional_大阪_2026-02":18657.0,"regional_大阪_2026-05":21470.0,"regional_東京_2026-01":19990.0,"regional_東京_2026-02":5000.0,"regional_東京_2026-04":17324.0}}
```

`status=pass`、`ok=true`、`pipeline_run_ok=true`、`failure_kinds=[]`。

#### E4 / inspection

`results-schema.json` は `status=pass`、`ok=true`、`results_path=output/results.json`、`error=null`。`data-assurance.json` は理由空、6 checksすべてtrue、`status=full` / `assurance=full`。

inspectionは初回verifyで次の実データ照合failureを記録した。

```text
data_inspection_schema:
inspection_schema_violation:column_names_missing_headers:amount;
inspection_schema_violation:input_row_count_mismatch:expected=60:reported=10
```

repair 1/4が `output/inspection.json` を変更し、最終evidenceは `status=pass`、`ok=true`、`input_path=data/sales.csv`。最終JSONは5キーを持ち、`input_row_count=60`、`column_names=["date","region","amount"]`。

#### final acceptanceイベント列

```text
phase_verification_result phase=data-inspection mode=intermediate_invariant ok=true
phase_verification_result phase=data-cleaning mode=intermediate_invariant ok=true
phase_verification_result phase=data-aggregation mode=intermediate_invariant ok=true
phase_verification_result phase=data-reporting mode=intermediate_invariant ok=true
phase_verification_result phase=data-validation mode=intermediate_invariant ok=true
phase_verification_result phase=data-validation mode=final_acceptance ok=true
ultra_phase_profile_check phase=data-validation final_phase=true ok=true
ultra_phase_complete phase=data-validation final_phase=true ok=true
profile_behavior_probe status=pass ok=true
ultra_final_acceptance final_acceptance_status=full_success assurance_level=full primary_reason=pass release_gate_status=pass runtime_acceptance_status=pass
final_acceptance_deterministic_remedies deterministic_remedies_applied=[]
ultra_plan_complete ok=true total_phases=5
tui_command_stop status=completed task_status=complete final_acceptance_status=full_success
run_stop status=completed task_status=complete final_acceptance_status=full_success
```

`step_short_circuited` は16件で、全件をDATA-12節に転記済み。

契約§2のとおり、このfullはpipelineの機械的誠実性を意味し、業務的な分析解釈の正しさを主張しない。2本の除外理由内訳とtotalが異なる事実も、加工せず上記evidenceどおり記録した。

## DATA-10 / inspectionの記録

| run | inspection結果 |
|---|---|
| qwen35 profile 1 | `output/inspection.json` 不在。`write_required` targetは同path、Read系tool拒否2件後に枯渇 |
| qwen35 profile 2 | inspection scriptは生成、`output/inspection.json` 不在。`write_required` targetは同path、Read系tool拒否2件後に枯渇 |
| gemma31 profile | 初回 `amount` header欠落＋`reported=10`をrepairし、schema PASS。5/5 phase完走 |
| qwen35 none 1 | inspection artifactは5キー非準拠だが工程検査は最終gate外。E1〜E4はPASS |
| qwen35 none 2 | inspection artifact不在 |
| gemma31 none | inspection artifactは生成したが固有schema。final check到達前にartifact follow-through停止 |

DATA-10クラスの再発は2/6だが、事前の記録対象であるgemma inspection非追従は0/1だった。

## イベント語彙（6 run統合）

```text
  16 artifact_stagnation_feedback
   3 bash_path_confinement_rejected
   3 context_truncation_suspected
   2 depth_profile
   1 escalation_carryover
   2 final_acceptance_deterministic_remedies
   6 host_env_contamination
   9 inspect_command_normalized
  23 loop_stop
   1 path_fallback_evaluated
  10 phase_verification_result
   6 plan_preset_resolved
   3 planner_error
   1 planner_plan_sanitized
  86 planner_quality_issue
   1 planner_quality_retry
   2 planner_quality_retry_degraded
  17 planner_raw_output_shape
  19 preset_step_converted
   3 preset_ultra_plan_used
   2 profile_behavior_probe
 119 provider_turn_duration
  11 read_only_stagnation_feedback
   4 read_only_tool_rejected
   6 recovery_prompt_saved
   6 run_start
   6 run_stop
  52 runtime_bash_policy
  24 step_obligation_scope
  42 step_prompt_contract
  19 step_short_circuited
   1 step_verify_failure
   1 step_verify_repair
   6 time_profile
   1 tool_args_path_normalized
   1 tool_args_path_salvaged
 133 tool_call_raw
 120 tool_execute
   7 tool_validation_error
   6 tui_command_stop
   6 ultra_context_initialized
   2 ultra_final_acceptance
   4 ultra_partial_artifact_summary
   8 ultra_phase_complete
  12 ultra_phase_context_attached
  12 ultra_phase_context_updated
   8 ultra_phase_execute_complete
   4 ultra_phase_failed
  12 ultra_phase_plan_validated
   8 ultra_phase_profile_check
  12 ultra_phase_scaffold_complete
  12 ultra_phase_start
   2 ultra_plan_complete
   3 ultra_plan_generation_attempt
   3 ultra_plan_generation_metadata_normalized
   3 ultra_plan_generation_succeeded
   3 ultra_plan_raw_output_shape
  19 verify_canonicalized
   7 verify_command_normalized_at_runtime
   8 verify_default_bound
   3 workspace_cd_stripped
```

## 成果物と不在資料

各runに `.anvil/` 一式、`data/sales.csv`、`data/sales.csv.sha256` を退避した。`pipeline/`、`output/`、`evidence/` は存在したrunのみ実物を退避し、存在しない資料は補完していない。

| run | pipeline | output | evidence | `.anvil` events/summary/repair/plan |
|---|---|---|---|---|
| qwen35 profile 1 | なし | なし | なし | あり |
| qwen35 profile 2 | なし | 空（追跡ファイルなし） | なし | あり |
| gemma31 profile | main.py | inspection/results/report | 7 JSON（E1〜E4、probe、assurance、inspection） | あり |
| qwen35 none 1 | main.py | inspection/results/report | 7 JSON（同上） | あり |
| qwen35 none 2 | なし | なし | なし | あり |
| gemma31 none | main.py | inspectionのみ | なし | あり |

退避後、6本のCSV SHA一致、全events JSONL各行、全evidence/output JSONのparse成功を検証した。
