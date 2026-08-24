# data UAT #5（uat-test0715-data-005）計測レポート

## 結論

指定された6 runを新規計測ワークスペースで各1回だけ実行した。6/6が分類済みの `run_stop` と具体的な停止理由を残して正直終端したが、成功runおよび `full` は0/6だった。

事前宣言した判定は、P0-a **PASS**、P0-b **PASS**、P0-c **PASS**、P1-a **FAIL**、P1-b **PASS** である。E2の偽陽性は0件になり、Run 3で `rerun-consistency` が初めて実戦実行されてPASSした。一方、Run 2は `data-inspection` で `write_required exhausted for output/inspection.json` に再到達し、DATA-10型が1/6で残った。

| 判定 | 結果 | 計測事実 |
|---|---:|---|
| P0-a: 6/6 正直終端 | PASS | 6/6に `run_stop`、`failure_kind=process_failure`、具体的な `stop_reason`、repair prompt/planがある。panic・分類不能終端・理由なき中断は0件 |
| P0-b: assurance契約§4準拠 | PASS | パイプライン実行プローブ未完の4 runは `static` または `failed`。Run 3の `partial` はpipeline-run/E1/E2/E3がPASS、E4の `data_inspection_schema` がFAILであり§4と一致。インフレ0件 |
| P0-c: E2偽陽性ゼロ | PASS | `claim_kind=date_label` 36件はすべて `ok=true`。`reconciliation.*` への照合11件もすべて成功。残る3違反はレポートの地域合計に対応する `values` キーが実在しないため、偽陽性には数えない |
| P1-a: DATA-10型ゼロ＋profileの字義例スキーマ確認 | **FAIL** | qwen35 profileは5キー構造でPASSしたが、gemma31 profileは3キー欠落後に `output/inspection.json` への `write_required` が枯渇。DATA-10型1件 |
| P1-b: E3を1本以上で実行 | PASS | Run 3で `rerun-consistency.json` が生成され、`pipeline_run_ok=true`、baselineとrerunが完全一致、PASS |

## 計測条件

- 対象: `develop`、HEAD `0103ae55b156ee2c58df7126fa5537fe35d2c297`（`0103ae5 Add E2 calibration fixtures`、`origin/develop` と一致）
- バイナリ: `commandagent 0.1.0 0103ae5 2026-07-15T06:26:12Z`（`+dirty` なし）
- release/install binary SHA-256: `1fdff9756a82e7db625f24aac91c84318539631ec4080db4dfa31e52d31713fd`（両者一致）
- 計測ワークスペース: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0715_data_005`。実行前に不存在を確認して新規作成
- 成果物先: `workspace/management/runs/uat-test0715-data-005/`
- 共通入力 SHA-256: `2f6c04e42b0ebdff85a7eb6b52a342610155be6796bd89e5729075d87c78d873`。生成直後と退避後の6/6で一致
- 共通 planner: `qwen3.6:27b-coding-nvfp4` / `ollama`
- 共通 profile/provider/context: `data` / `ollama` / `65536`
- 共通 goal: `data/sales.csv を読み込み、月次×地域の売上集計と全体合計を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。`
- 外側のcommandagent invocationは各run 1回、再試行0回。planner/executorが同一run内で行ったbounded repair/replanはeventsにそのまま残した

### Preflight

計測前に存在した別タスクの未追跡ディレクトリ `workspace/management/runs/uat-test0715-ff1-001/` は、ユーザー確認後に対象限定stash `356a9af305040a18def87fe862857e0d37af6208`（`Preserve uat-test0715-ff1-001 during data UAT #5 preflight`）へ一時退避した。その後の `git status --porcelain` が空であることを確認してpreflightを開始した。このstashは本UATコミットに含めず、提出コミット後に復元する。

| 項目 | 結果 |
|---|---|
| `git status --porcelain` | 上記の対象限定一時退避後に空 |
| `git log -1 --oneline` | `0103ae5 Add E2 calibration fixtures`（指定された `0103ae5` 以降） |
| `cargo test --quiet` | exit 0。主要lib 1311 passed / 13 ignored、全integration/doc targetを含め失敗0 |
| `cargo build --release` | exit 0 |
| install | `install -m 755 target/release/commandagent /Users/maenokota/.local/bin/commandagent` 成功 |
| `commandagent --version` | `commandagent 0.1.0 0103ae5 2026-07-15T06:26:12Z` |

## Run行列

所要時間は各コマンドを `/usr/bin/time -p` で外側から計測した `real` 値である。

| # | run / run id | executor / preset | exit | terminal / final acceptance | assurance | 失敗クラス（主要終端） | 所要時間 |
|---:|---|---|---:|---|---|---|---:|
| 1 | `data5_qwen35_profile_001`<br>`019f6476-d96f-7e20-abda-0094031f600e` | qwen3.6:35b-a3b-coding-nvfp4 / profile | 1 | failed / not_checked | failed (`data_assurance_failed`) | `data-cleaning`: `model_stagnation:read_only_loop: write_required exhausted for output/results.json` | 732.80 s |
| 2 | `data5_gemma31_profile_001`<br>`019f6482-ec38-7331-8626-3940bca992b1` | gemma4:31b / profile | 1 | failed / not_checked | failed (`data_profile_script_not_generated`) | `data-inspection`: `model_stagnation:read_only_loop: write_required exhausted for output/inspection.json` | 406.41 s |
| 3 | `data5_qwen35_none_001`<br>`019f6489-9c41-7273-ab72-2c375324935b` | qwen3.6:35b-a3b-coding-nvfp4 / none | 1 | failed / not_checked | partial (`data_assurance_partial`) | 最終phase `final-verification-and-cleanup`: `data_inspection_schema` 5キー欠落、bounded repair 2回で不変 | 1560.84 s |
| 4 | `data5_qwen35_none_002`<br>`019f64a1-ff3d-7c00-adc1-89851d981cc6` | qwen3.6:35b-a3b-coding-nvfp4 / none | 1 | failed / not_checked | static (`data_profile_probe_not_run`) | `data-ingestion-and-schema-inspection`: `artifact_follow_through_exhausted`; missing `output/results.json`, `output/report.md`, feedback 2 | 786.91 s |
| 5 | `data5_gemma31_none_001`<br>`019f64af-22e9-7ec3-b72b-96601f2aad09` | gemma4:31b / none | 1 | failed / not_checked | static (`data_profile_probe_not_run`) | `load-and-validate-data`: `data_results_schema` が不在の `output/results.json` を読めず、bounded repair 2回で不変 | 504.06 s |
| 6 | `data5_gemma31_none_002`<br>`019f64b7-4bd8-7332-9eed-d539f8449ea7` | gemma4:31b / none | 1 | failed / not_checked | static (`data_profile_probe_not_run`) | `load-and-define-validation-rules`: `artifact_follow_through_exhausted`; missing `output/results.json`, `output/report.md`, feedback 1 | 686.38 s |

## E2監査（B-2g較正の答え合わせ）

`claims` は `claims-binding.json` の配列要素数、`ok` と `violations` は各要素の `ok` で集計した。results/report不在によるevidence失敗は数値claim違反と混同せず記載した。

| run | claims | ok | violations | `claim_kind=date_label` | `reconciliation.*` 照合でok | `nearest_miss` | evidence結果 |
|---|---:|---:|---:|---:|---:|---|---|
| qwen35 profile | 39 | 36 | 3 | 12（全ok） | 5 | あり、3件 | FAIL。地域別合計3件のみ違反 |
| gemma31 profile | — | — | — | — | — | — | `claims-binding.json` 不在 |
| qwen35 none 1 | 46 | 46 | 0 | 24（全ok） | 6 | なし | **PASS** |
| qwen35 none 2 | 0 | 0 | 0 | 0 | 0 | なし | FAIL。`output/results.json` 不在のためclaim抽出前に停止 |
| gemma31 none 1 | — | — | — | — | — | — | `claims-binding.json` 不在 |
| gemma31 none 2 | 0 | 0 | 0 | 0 | 0 | なし | FAIL。`output/results.json` 不在のためclaim抽出前に停止 |
| **有効なreport/results 2 run計** | **85** | **82** | **3** | **36（全ok）** | **11** | **3** | 数値claim偽陽性0件 |

### 数値claim違反の全件原文

3件とも Run 1 の `output/report.md`「地域別合計」表から抽出された。`matched_key`、`matched_result_value`、`rounded_result_value` はすべて `null` である。

| failure_kind原文 | report文脈 | claim record原文（要点） |
|---|---|---|
| `claims_binding_violation:output/report.md:651:40497.00` | `\| 名古屋 \| 40497.00 \|` | `raw="40497.00"`, `normalized_value=40497.0`, `printed_precision=2`, `nearest_miss={"key":"2026-05_大阪","result_value":21470.0,"rounded_result_value":21470.0,"absolute_difference":19027.0}` |
| `claims_binding_violation:output/report.md:673:40127.00` | `\| 大阪 \| 40127.00 \|` | `raw="40127.00"`, `normalized_value=40127.0`, `printed_precision=2`, `nearest_miss={"key":"2026-05_大阪","result_value":21470.0,"rounded_result_value":21470.0,"absolute_difference":18657.0}` |
| `claims_binding_violation:output/report.md:695:36814.00` | `\| 東京 \| 36814.00 \|` | `raw="36814.00"`, `normalized_value=36814.0`, `printed_precision=2`, `nearest_miss={"key":"2026-05_大阪","result_value":21470.0,"rounded_result_value":21470.0,"absolute_difference":15344.0}` |

Run 1の `results.json.values` には月×地域18キーと `overall_total` だけがあり、`region_total_名古屋`、`region_total_大阪`、`region_total_東京` または同値のキーはない。3値はそれぞれ `20730+19767=40497`、`18657+21470=40127`、`19990+16824=36814` と月別値から算出できるが、対応値そのものは `values` / `reconciliation` のどのキーにも存在しない。このため3件は表記・日付・照合域による偽陽性ではない。

claim配列を作れずに失敗した2件のfailure原文は次のとおりである。

```text
data5_qwen35_none_002:
claims_binding_violation:invalid_results_schema:failed to read /Users/maenokota/share/work/localwork/commandagent_mvp/01/test0715_data_005/data5_qwen35_none_002/output/results.json: No such file or directory (os error 2)

data5_gemma31_none_002:
claims_binding_violation:invalid_results_schema:failed to read /Users/maenokota/share/work/localwork/commandagent_mvp/01/test0715_data_005/data5_gemma31_none_002/output/results.json: No such file or directory (os error 2)
```

### 較正機能の実測

- ISO月ラベルから抽出された内部token 36件は、evidenceに `claim_kind="date_label"`, `matched_key=null`, `nearest_miss=null`, `ok=true` と明示された。日付tokenの黙示的削除は0件
- reconciliation照合は Run 1で `input_rows`、`used_rows`、`excluded_rows_total`、`excluded[0].rows`、`excluded[1].rows` の5件、Run 3で6件が `matched_key="reconciliation...."` として成功
- Run 1の真の照合不能3件にはすべて `nearest_miss` が記録された。違反がないRun 3とclaim抽出前失敗runには `nearest_miss` はない

## inspection監査

「字義例スキーマ準拠」は、`column_names` / `input_row_count` / `type_summaries` / `distinct_values` / `sample_rows` の5キー構造を持つかで機械的に判定した。値が実測と一致するかは別欄に事実を記録する。

| run | `data_inspection_schema` | 欠落キー / failure | 修復回数 | `inspection.json` と字義例5キー構造 |
|---|---|---|---|---|
| qwen35 profile | 初回FAIL → **PASS** | 初回 `column_names,input_row_count,type_summaries,distinct_values` 欠落 | schema repair 1回、`step_verify_repair ok=true` | **準拠（5/5）**。ただし `input_row_count=24` で実入力60と不一致。その他の例値は東京/大阪/名古屋および先頭行形 |
| gemma31 profile | **FAIL** | `column_names,input_row_count,type_summaries` 欠落 | schema failure後repair turn 2回、書き込みなし。`step_verify_repair` 完了イベント0 | 非準拠（2/5）。`distinct_values` と `sample_rows` のみあり、値は `North/South/East/West`, 2023年で入力と不一致 |
| qwen35 none 1 | 最終phaseで **FAIL** | 5キーすべて欠落 | bounded repair 2回、いずれもno change | 非準拠（0/5）。独自キー `total_rows`, `valid_rows`, `invalid_rows`, `excluded_reasons`, `valid_regions` |
| qwen35 none 2 | 未実行 | evidenceなし | 0 | 非準拠（0/5）。独自キー `columns`, `total_rows`, `valid_rows` 等。`total_rows=24` |
| gemma31 none 1 | **FAIL**（複合verify内） | `inspection_path:path does not exist: output/inspection.json` | 複合verifyのbounded repair 2回、主失敗はresults不在 | 成果物なし |
| gemma31 none 2 | 未実行 | evidenceなし | 0 | 非準拠（0/5）。内容 `{}` |

profile 2 runの `data-inspection` について、inspection段の `read_only_stagnation_feedback stage=write_required` は qwen35で1件、gemma31で2件だった。qwen35は `output/inspection.json` を書き直してphaseを完了した。gemma31は同じ対象へのBashが3回 `read_only_tool_rejected` となり、`write_required exhausted for output/inspection.json` で停止した。したがってDATA-10型は1/6で再発している。

## E系 evidence

| run | `pipeline-run.json` | `reconciliation.json` | `claims-binding.json` | `rerun-consistency.json` |
|---|---|---|---|---|
| qwen35 profile | あり / PASS。`python3 -B pipeline/main.py`, exit 0, 86 ms | あり / PASS。`60 = 57 + 3`; `invalid_date=2`, `missing_amount=1` | あり / FAIL。36/39 ok、真の対応値欠落3件 | なし |
| gemma31 profile | なし | なし | なし | なし |
| qwen35 none 1 | あり / PASS。`python3 -B pipeline/main.py`, exit 0, 57 ms | あり / PASS。`60 = 57 + 3`; `empty_date=1`, `invalid_date=1`, `non_numeric_amount=1` | あり / **PASS**。46/46 ok | あり / **PASS** |
| qwen35 none 2 | なし | あり / FAIL。`output/results.json` 不在 | あり / FAIL。`output/results.json` 不在 | なし |
| gemma31 none 1 | なし | あり / FAIL。`output/results.json` 不在 | なし | なし |
| gemma31 none 2 | なし | あり / FAIL。`output/results.json` 不在 | あり / FAIL。`output/results.json` 不在 | なし |

到達率は `pipeline-run` 2/6（PASS 2）、`reconciliation` 5/6（PASS 2）、`claims-binding` 4/6（PASS 1）、`rerun-consistency` 1/6（PASS 1）である。成功したreconciliation 2件はいずれも勘定式 `60=57+3` を満たすが、事前期待 `60/56/4` には一致せず、負数行 `-500` を除外内訳に含めていない。

### rerun-consistency初実戦発火

Run 3の `rerun-consistency.json` は次の事実を記録した。

```text
status=pass
ok=true
pipeline_run_ok=true
entry=pipeline/main.py
failure_kinds=[]
baseline_results == rerun_results: true
values key count: 16
reconciliation: input_rows=60, used_rows=57,
  excluded=[empty_date:1, invalid_date:1, non_numeric_amount:1]
grand_total=117438.0
month totals: 2026-01=19990.0, 2026-02=18657.0, 2026-03=20730.0,
  2026-04=16824.0, 2026-05=21470.0, 2026-06=19767.0
region totals: 名古屋=40497.0, 大阪=40127.0, 東京=36814.0
```

baselineとrerunはreconciliationおよび16個のvaluesを含むJSON値として完全一致した。最終phase到達の証拠は `ultra_phase_start phase_index=4/4` と、このPASS evidenceの両方にある。

## assurance監査

契約 `docs/data-profile-contract.md` §4に従い、`full` はE1〜E4全pass、`partial` はpipeline実行成功かつE1/E3 passでE2またはE4未達、`static` はスクリプト生成済みだが実行probe未完、`failed` は実行/E1/再現性等の失敗として照合した。

| run | assurance / 根拠 | pipeline実行probe | 契約§4照合 | 準拠判定 |
|---|---|---:|---|---:|
| qwen35 profile | failed / `data_assurance_failed` | あり / PASS | E1 PASS、E2 FAIL、E3未到達、E4 inspection/results PASS | 準拠（partialを名乗らない保守側） |
| gemma31 profile | failed / `data_profile_script_not_generated` | なし | pipelineなし、inspection FAIL | 準拠 |
| qwen35 none 1 | partial / `data_assurance_partial` | あり / PASS | E1/E2/E3 PASS、E4 results PASS・inspection FAIL | 準拠 |
| qwen35 none 2 | static / `data_profile_probe_not_run` | なし | pipelineは生成、results/reportおよびprobeなし | 準拠 |
| gemma31 none 1 | static / `data_profile_probe_not_run` | 実行stepはあったがprobe evidenceなし、results未生成 | pipeline script生成、probe未完 | 準拠 |
| gemma31 none 2 | static / `data_profile_probe_not_run` | なし | pipelineは生成、results/reportおよびprobeなし | 準拠 |

パイプライン実行probe未完runの `partial` / `full` は0件、assuranceインフレは0件だった。

## 正準化・書き換え・拒否イベント

| run | `verify_canonicalized` | planner safe-shell split | runtime `workspace_cd_normalized` | runtime `success_failure_echo_stripped` | `workspace_cd_stripped` | `inspect_command_normalized` | verify-policy拒否 | runtime拒否 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| qwen35 profile | 4 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| gemma31 profile | 2 | 0 | 0 | 0 | 0 | 3 | 0 | 0 |
| qwen35 none 1 | 16 | 1 | 4 | 1 | 2 | 0 | 2 | 0 |
| qwen35 none 2 | 3 | 0 | 0 | 0 | 1 | 0 | 0 | 0 |
| gemma31 none 1 | 3 | 0 | 0 | 0 | 0 | 1 | 0 | 0 |
| gemma31 none 2 | 1 | 0 | 0 | 0 | 0 | 2 | 1 | 0 |
| **合計** | **29** | **1** | **4** | **1** | **3** | **6** | **3** | **0** |

DATA-7段2で事前に名付けたkindのうち、`normalization_kind=stderr_suppression_stripped` と `fallback_true_stripped` は0件、`normalization_kind=shell_control_split` という名前のruntimeイベントも0件だった。代わりにplan sanitizeの `planner_verify_command_normalized` 1件が `contains_safe_shell_split=true` を記録し、runtimeのecho分岐縮約1件は `success_failure_echo_stripped` というkindで記録された。

原文例:

```text
planner safe-shell split:
ls -d pipeline output | test -f pipeline/main.py && test -f smoke_check.py | python3 smoke_check.py
→ ["ls -d pipeline output", "test -f pipeline/main.py", "test -f smoke_check.py", "python3 smoke_check.py"]

runtime success/failure echo strip:
grep -q "月次" output/report.md && echo "PASS" || echo "FAIL"
→ ["grep -q \"月次\" output/report.md"]

runtime workspace cd normalization:
cd /Users/maenokota/share/work/localwork/commandagent_mvp/01/test0715_data_005/data5_qwen35_none_001 && python verify_checks.py
→ ["python verify_checks.py"]
```

B-2fのpython `-c` 契約アサーション正準化は実戦で観測された。代表例は次のとおりである。

```text
python -c "import json; d=json.load(open('output/results.json')); assert 'reconciliation' in d and 'values' in d; r=d['reconciliation']; assert r['input_rows'] == r['used_rows'] + sum(e['rows'] for e in r['excluded'])"
→ anvil-catalog-check:data_results_schema
→ anvil-catalog-check:data_reconciliation

python -c "import json,sys;r=json.load(open('output/results.json'));i=json.load(open('output/inspection.json'));assert 'reconciliation' in r and 'values' in r;rc=r['reconciliation'];assert rc['input_rows']==rc['used_rows']+sum(e['rows'] for e in rc['excluded']);assert 'columns' in i;assert len(open('output/report.md').read())>0;print('pass')"
→ anvil-catalog-check:data_results_schema
→ anvil-catalog-check:data_reconciliation
```

### original_command付き拒否の全件

`runtime_bash_policy` は39件、verify文脈15件、`blocked=true` は0件だった。planner verify policyの拒否3件はすべて `original_command`、`violation_kind=shell_control_syntax`、`normalized_commands=[]` を記録した。

1. Run 3、step `verify-output-artifacts`

```text
python -c "import os; assert os.path.exists('output/report.md') and os.path.getsize('output/report.md') > 0; print('OK')"
```

2. Run 3、step `verify-report-consistency`

```text
python -c "import json; r=json.load(open('output/results.json')); md=open('output/report.md').read(); assert all(str(v) in md for v in r['values'].values())"
```

3. Run 6、step `verify-inspection-output`

```text
python -c "import json; assert 'schema' in json.load(open('output/inspection.json'))"
```

## UAT #4との対比

| 指標 / 死因クラス | UAT #4 | UAT #5 | 変化の事実 |
|---|---:|---:|---|
| 正直終端 | 6/6 | 6/6 | 不変 |
| full | 0/6 | 0/6 | 不変 |
| E2 raw violations（有効report/results） | 49 | 3 | 46減。#5の3件はvalues対応キー不在 |
| E2偽陽性 | 49（日付分割36＋reconciliation照合域13） | 0 | 49→0 |
| date_labelがviolation | 36 | 0（36件を監査記録し全ok） | 36→0 |
| reconciliation数値がviolation | 13 | 0（11件照合成功） | 13→0 |
| claims-binding PASS | 0/4存在 | 1/4存在 | Run 3が初PASS |
| profile inspection完走 | 1/2 | 1/2 | 不変。qwen PASS、gemma FAIL |
| DATA-10型 | 1/6 | 1/6 | gemma profileで同じ `output/inspection.json` 対象の枯渇 |
| noneで最終phase到達 | 0/4 | 1/4 | Run 3が4/4 phaseへ到達 |
| rerun-consistency | 0/6 | 1/6 PASS | 初実戦発火 |
| `60/56/4` 一致 | 1/2 successful reconciliation | 0/2 | #5の2件はいずれも `60/57/3` |

フェーズ深度は、profileでは両回ともqwenがinspection完了後の第2phase、gemmaがinspection内で停止し、UAT #4と同じだった。noneではRun 3だけが第1〜3phaseを完了して最終第4phaseに到達し、E1/E2/E3をPASSした。他のnone 3 runは第1phaseで停止した。

## イベント語彙（6 run統合）

```text
  18 artifact_stagnation_feedback
   1 dependency_build_lifecycle
   6 escalation_carryover
   6 host_env_contamination
   6 inspect_command_normalized
  34 loop_stop
   2 path_fallback_evaluated
   4 phase_verification_result
   6 plan_preset_resolved
   4 planner_error
   2 planner_plan_sanitized
  63 planner_quality_issue
   2 planner_quality_retry
   1 planner_quality_retry_degraded
  17 planner_raw_output_shape
   1 planner_verify_command_normalized
   8 preset_step_converted
   2 preset_ultra_plan_used
 112 provider_turn_duration
  10 read_only_stagnation_feedback
   5 read_only_tool_rejected
  10 recovery_prompt_saved
   6 run_start
   6 run_stop
  39 runtime_bash_policy
  32 step_obligation_scope
  33 step_prompt_contract
   7 step_short_circuited
   4 step_verify_failure
   5 step_verify_repair
   6 time_profile
   6 tool_args_path_normalized
   2 tool_args_path_salvaged
 111 tool_call_raw
 106 tool_execute
   6 tui_command_stop
   6 ultra_context_initialized
   6 ultra_partial_artifact_summary
   4 ultra_phase_complete
  10 ultra_phase_context_attached
  10 ultra_phase_context_updated
   4 ultra_phase_execute_complete
   6 ultra_phase_failed
  10 ultra_phase_plan_validated
   4 ultra_phase_profile_check
  10 ultra_phase_scaffold_complete
  10 ultra_phase_start
   4 ultra_plan_generation_attempt
   4 ultra_plan_generation_metadata_normalized
   4 ultra_plan_generation_succeeded
   4 ultra_plan_raw_output_shape
  29 verify_canonicalized
   5 verify_command_normalized_at_runtime
   3 workspace_cd_stripped
```

## 記録項目

- full率: 0/6。data初fullは発生しなかったため、full時に要求された全evidence完全転記は該当なし
- profile: 0/2成功、none: 0/4成功
- `output/results.json` と `output/report.md` の両方があるrun: 2/6（Run 1, Run 3）
- E1/E2/E3の同時PASS: 1/6（Run 3）。E4のinspection schemaがFAILしたため `partial`
- 全runの `.anvil/`、存在した `pipeline/`、`output/`、`evidence/`、入力CSV、入力SHA記録を `artifacts/` に退避した。存在しなかった資料は補完していない
- 計測中断は0件。6本の外側commandはいずれも製品自身の分類済み終端まで待機した
