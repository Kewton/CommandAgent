# data UAT #8 — 第2シナリオ族（時系列）（uat-test0716-data-008）計測レポート

## 結論

指定された6 runを新規計測ワークスペースで、指定順 1→4→2→5→3→6、各1回だけ実行した。6/6が分類済みの `run_stop` と具体的な停止理由を残して正直終端したが、最終受け入れへ到達したrunはなく、fullは0/6だった。

事前宣言した判定は、P0-a **PASS**、P0-b **PASS**、P0-c **PASS**、P1-a **FAIL（未実戦）**、P1-b **FAIL** である。`output/report.md` に数値付き%クレームを含むrunは2本、クレームは計11件あったが、両runともE2到達前に停止し、`claims-binding.json` は6本すべてで存在しない。そのため `%` の `percent=true` 正規化、date labelとの区別、機械照合結果は実戦確認できなかった。

第1族の固定コードUAT #7では未観測だった停止形を2件記録した。Run 5では生成した派生CSVを一時的に `data/` 配下へ置いたため `data_inspection_schema` が3入力を検出し、Run 6では `output/monthly_metrics.csv` を読む `python -c` の複数assertが `shell_control_syntax` として補正再計画後も拒否され、phase scaffoldで停止した。後者はDATA-7で観測された「verifyのshell control拒否による停止」と同じ終端形を持つため、コマンド形が時系列固有の新しい派生メトリクス検証であることを併記したうえで、P1-bを厳格にFAILとした。

| 判定 | 結果 | 計測事実 |
|---|---:|---|
| P0-a: 6/6 正直終端 | **PASS** | 6/6に `run_stop`、`status=failed`、`failure_kind=process_failure`、具体的な `stop_reason`。panic・分類不能終端・理由なき中断は0 |
| P0-b: assurance契約§4準拠 | **PASS** | pipeline生成済み・probe未実行の4本はstatic、pipeline未生成の2本はfailed。partial/fullは0で、インフレ・デフレとも0 |
| P0-c: 偽成功ゼロ | **PASS** | fullを名乗るrunは0。`ultra_final_acceptance` 0件、E1〜E4一式を持つrunも0で、false-full 0 |
| P1-a: E2の%正規化 | **FAIL（未実戦）** | 数値付き%クレーム11件を含むreport 2本は生成されたが、E2 evidence 0/6。`percent=true`、対応キー、照合結果は未記録 |
| P1-b: DATA-1〜9,11,12再発ゼロ | **FAIL** | Run 6が `verify_command_policy_error / shell_control_syntax` の補正枯渇でphase scaffold停止。DATA-7隣接の新しい派生メトリクス検証形として一次資料を転記。他クラスの既知終端形は0 |
| 記録: full率 | **0/6** | profile 0/3、none 0/3 |
| 記録: DATA-10残存分散 | **2/6** | Run 3はinspection未書き込み、Run 5はmultiple-input失敗後のinspection書き込み非追従。いずれもwrite-required枯渇 |

## 計測条件

- 対象: `develop`、HEAD `fcb9ac8b505f1725db481f26c8ab14fd3d4d15ec`（`fcb9ac8 Record Phase B settlement`、計測開始時に `origin/develop` と一致）
- バイナリ: `commandagent 0.1.0 fcb9ac8 2026-07-16T00:25:36Z`（`+dirty` なし）
- release/install binary SHA-256: `240a24f4a261a75f5e0bc08528de61d8e8afda43f4971ae415371d1a24ec997b`（両者一致）
- 計測ワークスペース: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0716_data_008`。実行前に不存在を確認して新規作成
- 成果物先: `workspace/management/runs/uat-test0716-data-008/`
- 共通入力 SHA-256: `2f6c04e42b0ebdff85a7eb6b52a342610155be6796bd89e5729075d87c78d873`。生成直後と退避後の6/6で一致
- 共通 planner: `qwen3.6:27b-coding-nvfp4` / `ollama`
- 共通 profile/provider/context: `data` / `ollama` / `65536`
- 共通 goal: `data/sales.csv を読み込み、月次の売上合計・前月比（%）・3ヶ月移動平均を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。`
- 外側のcommandagent invocationは各run 1回、再試行0回。planner/executorが同一run内で行ったbounded repair/replanはeventsにそのまま残した
- 実行順: Run 1 → 4 → 2 → 5 → 3 → 6
- 所要時間は各runの `time_profile.profile.total_ms`。6本合計は4598.693秒

### Preflight

計測前に存在した別タスクの未追跡ディレクトリ `workspace/management/runs/uat-test0715-ff1-001/` は、ユーザー確認後にstash commit `3ed51b6005ed24fcae01f5c91503cefa4f68d9e1`（`On develop: isolate uat-test0715-ff1-001 for data UAT 8`）へ一時隔離した。その後の `git status --porcelain` が空であることを確認した。このstashは本UATコミットに含めず、提出コミット後に復元する。

| 項目 | 結果 |
|---|---|
| `git status --porcelain` | 上記の一時隔離後に空 |
| `git log -1 --oneline` | `fcb9ac8 Record Phase B settlement` |
| `cargo test` | browser probeを実行可能な権限付き環境でexit 0。主要lib 1337 passed / 13 ignored、conformance 18 passed / 1 ignored、data profile 10/10を含むfull suite失敗0 |
| `cargo build --release` | exit 0 |
| install | `install -m 755 target/release/commandagent /Users/maenokota/.local/bin/commandagent` 成功 |
| `commandagent --version` | `commandagent 0.1.0 fcb9ac8 2026-07-16T00:25:36Z` |

## 独立参照値

指定された参照スクリプトを入力生成後に独立して1回実行した原文は次のとおり。

```text
monthly: {'2026-01': 19990.0, '2026-02': 18657.0, '2026-03': 20730.0, '2026-04': 17324.0, '2026-05': 21470.0, '2026-06': 19767.0}
mom%: {'2026-02': -6.67, '2026-03': 11.11, '2026-04': -16.43, '2026-05': 23.93, '2026-06': -7.93}
ma3: {'2026-03': 19792.33, '2026-04': 18903.67, '2026-05': 19841.33, '2026-06': 19520.33}
input: 60 used: 56
```

この参照値は指示どおりrecord-onlyであり、P0/P1判定およびdata契約§4のassurance判定には用いていない。

## Run行列

| # | run / run id | executor / preset | exit | terminal / final acceptance | assurance | 到達phase | 失敗クラス（主要終端） | 所要 |
|---:|---|---|---:|---|---|---:|---|---:|
| 1 | `data8_ts_qwen35_profile_001`<br>`019f6852-7dae-7440-96e5-f1e90e5fd023` | qwen3.6:35b-a3b-coding-nvfp4 / profile | 1 | failed / not_checked | static (`data_profile_probe_not_run`) | 1/5 | `data-cleaning`: `artifact recovery exhausted`、不足 `output/results.json` / `output/report.md` | 1236.318 s |
| 2 | `data8_ts_qwen35_profile_002`<br>`019f689a-9e66-78a2-991f-a670c83ac650` | qwen3.6:35b-a3b-coding-nvfp4 / profile | 1 | failed / not_checked | static (`data_profile_probe_not_run`) | 1/5 | `data-cleaning`: `artifact recovery exhausted`、不足 `output/results.json` / `output/report.md` | 749.648 s |
| 3 | `data8_ts_gemma31_profile_001`<br>`019f68af-af65-7983-a0ff-ef1de0153391` | gemma4:31b / profile | 1 | failed / not_checked | failed (`data_profile_script_not_generated`) | 0/5 | `data-inspection`: `model_stagnation:read_only_loop: write_required exhausted for output/inspection.json` | 302.147 s |
| 4 | `data8_ts_qwen35_none_001`<br>`019f688e-d973-7480-94b2-85f71b247231` | qwen3.6:35b-a3b-coding-nvfp4 / none | 1 | failed / not_checked | static (`data_profile_probe_not_run`) | 1/4 | `compute-metrics`: `loop_progress_exhausted: model_stagnation:read_only_loop` | 738.501 s |
| 5 | `data8_ts_qwen35_none_002`<br>`019f68a6-75f6-7cd2-9339-0c04a5b6117f` | qwen3.6:35b-a3b-coding-nvfp4 / none | 1 | failed / not_checked | failed (`data_profile_script_not_generated`) | 0/3 | `load-and-validate-data`: inspection `multiple_inputs`後にwrite-required枯渇 | 560.330 s |
| 6 | `data8_ts_gemma31_none_001`<br>`019f68b4-c21f-7bc0-a015-3d9de0ab6b61` | gemma4:31b / none | 1 | failed / not_checked | static (`data_profile_probe_not_run`) | 1/3 | `monthly-metrics-calculation`: shell control verifyの補正枯渇による `phase_scaffold_error` | 1011.749 s |

完了phase深度の分布は0 phaseが2本、1 phaseが4本。最終受け入れ到達、completed terminal、fullはいずれも0本だった。

## E2 %クレーム監査

### evidence到達

`claims-binding.json` は6 runすべてで存在しない。reportが存在するRun 4 / 6について、report原文とresultsの対応候補を機械的に並べた。以下の「対応候補」は月ラベルと `values` キーの機械的対応であり、E2が記録した `matched_key` ではない。

| run | reportの数値付き%クレーム原文 | `percent=true` | resultsの対応候補 | E2照合結果 |
|---|---|---|---|---|
| Run 4 | `| 2026-02 | 18657.00 | -6.67% | N/A |` | 未記録 | `mom_2026-02=-6.67` | 未実行 |
| Run 4 | `| 2026-03 | 20730.00 | +11.11% | 19792.33 |` | 未記録 | `mom_2026-03=11.11` | 未実行 |
| Run 4 | `| 2026-04 | 16824.00 | -18.84% | 18737.00 |` | 未記録 | `mom_2026-04=-18.84` | 未実行 |
| Run 4 | `| 2026-05 | 21470.00 | +27.62% | 19674.67 |` | 未記録 | `mom_2026-05=27.62` | 未実行 |
| Run 4 | `| 2026-06 | 19767.00 | -7.93% | 19353.67 |` | 未記録 | `mom_2026-06=-7.93` | 未実行 |
| Run 6 | `| 2026-01 | 19,990.00 | 0.00% | 19,990.00 |` | 未記録 | `mom_pct_2026-01=0.0` | 未実行 |
| Run 6 | `| 2026-02 | 18,657.00 | -6.67% | 19,323.50 |` | 未記録 | `mom_pct_2026-02=-6.668334167083542` | 未実行 |
| Run 6 | `| 2026-03 | 20,730.00 | 11.11% | 19,792.33 |` | 未記録 | `mom_pct_2026-03=11.11111111111111` | 未実行 |
| Run 6 | `| 2026-04 | 16,824.00 | -18.84% | 18,737.00 |` | 未記録 | `mom_pct_2026-04=-18.842257597684515` | 未実行 |
| Run 6 | `| 2026-05 | 21,470.00 | 27.62% | 19,674.67 |` | 未記録 | `mom_pct_2026-05=27.61531145981931` | 未実行 |
| Run 6 | `| 2026-06 | 19,767.00 | -7.93% | 19,353.67 |` | 未記録 | `mom_pct_2026-06=-7.931998136935259` | 未実行 |

見出しの `MoM %` / `MoM Change (%)` は数値付きクレームに数えていない。E2未実行のため、11件がquantity claimとして認識されたか、`percent=true` で正規化されたか、ISO月ラベルが `date_label` へ分離されたか、丸め照合がPASSしたかはevents/artifactsから判定できない。violationもPASSも記録されていないため、偽陽性ゼロを主張せずP1-aをFAIL（未実戦）とした。

## 意味的正解との対比（record-only）

fullまたはE2 PASSのrunは0本なので、事前指定された正式な対比対象は存在しない。参考として、report/resultsまで生成したがE2未実行のRun 4 / 6を独立参照値と比較する。これは合否およびassuranceに用いない。

| run | reconciliation | monthly | mom% | ma3 | record-only結果 |
|---|---|---|---|---|---|
| Run 4 | report/results `60 / 57 / 3`、参照 `60 / 56 / 4` | 5/6一致。`2026-04`: 16824.00 vs 17324.0 | 3/5一致。`2026-04`: -18.84 vs -16.43、`2026-05`: 27.62 vs 23.93 | 1/4一致。`2026-04`: 18737.00 vs 18903.67、`2026-05`: 19674.67 vs 19841.33、`2026-06`: 19353.67 vs 19520.33 | 不一致 |
| Run 6 | report/results `60 / 57 / 3`、参照 `60 / 56 / 4` | Run 4と同じ5/6一致 | Run 4と同じ3/5一致。初月0.00%は参照定義に値なし | Run 4と同じ1/4一致。初2ヶ月の値は参照定義に値なし | 不一致 |

両pipelineの原文では、空値・不正日付・非数値等は除外するが、負値を除外する分岐は存在しない。入力の `2026-04-01,東京,-500` を採用した結果、usedが57となり、4月合計が参照値より500小さい。これはdata契約§2/§7の意味的正しさのスコープ外であり、record-only観測として記録する。

## inspection監査

| run | inspection成果物 | `data_inspection_schema` | 修復 | 最終状態 |
|---|---|---|---:|---|
| qwen35 profile 1 | 5キー、`input_row_count=60` | PASS | 3回 | inspection phase完了 |
| qwen35 profile 2 | 5キー、`input_row_count=60` | PASS | 2回 | inspection phase完了 |
| gemma31 profile | なし | 未実行 | 0回 | inspection書き込み前にwrite-required枯渇 |
| qwen35 none 1 | 5キー、`input_row_count=60` | PASS | 1回 | phase 1完了 |
| qwen35 none 2 | 5キー、`input_row_count=60` | **FAIL** | 2回 | `multiple_inputs:data/sales.csv,data/sales_clean.csv,data/validation_log.csv` |
| gemma31 none | 独自6キー（規定5キーなし） | 未実行 | 0回 | phase 1はpipeline存在checkのみで完了 |

Run 1は初回5キー欠落、repair 1/2で `input_row_count_mismatch:expected=60:reported=24` と `distinct_values_missing_categorical_columns:date`、repair 3でPASSした。Run 2は初回4キー欠落、repair 1で同じ24/60行数不一致、repair 2でPASSした。Run 4は初回5キー欠落後、repair 1でPASSした。

## 第1族で未観測だった停止形

### TS8-1: 派生CSVによるinspection input discovery汚染（Run 5）

Run 5の `scripts/validate_sales.py` は一時的に `data/sales_clean.csv` と `data/validation_log.csv` を生成した。`data_inspection_schema` は次の原文で3入力を検出した。

```text
data_inspection_schema:inspection_schema_violation:multiple_inputs:data/sales.csv,data/sales_clean.csv,data/validation_log.csv
```

モデルはrepair 1 / 2で `output/inspection.json` を書き直したが同じfailureが残り、repair 3ではBashを2回拒否され、次の `run_stop.stop_reason` で終了した。

```text
phase load-and-validate-data failed: model_stagnation:read_only_loop: write_required exhausted for output/inspection.json; objective: Repair step `verify-data-cleaning`. Verification failed: data_inspection_schema:inspection_schema_violation:multiple_inputs:data/sales.csv,data/sales_clean.csv,data/validation_log.csv. Repair target: implementation. Fix the implementation files that should satisfy the requested behavior. Make the smallest bounded change, then stop. Overall goal: data/sales.csv
Paths:
- repair prompt saved: .anvil/repairs/repair-phase-load-and-validate-data-019f68af-059a-7c63-9cb5-d3423b06d710.md
- Recovery UltraPlan YAML saved: .anvil/plans/recovery-ultra-plan-phase-load-and-validate-data-019f68af-059a-7c63-9cb5-d35677487947.yaml
Commands:
- suggested command: /ultra-plan-run --profile data "$(cat .anvil/repairs/repair-phase-load-and-validate-data-019f68af-059a-7c63-9cb5-d3423b06d710.md)"
- suggested YAML command: /run-ultra-plan .anvil/plans/recovery-ultra-plan-phase-load-and-validate-data-019f68af-059a-7c63-9cb5-d35677487947.yaml
```

一次資料は `artifacts/data8_ts_qwen35_none_002/.anvil/runs/019f68a6-75f6-7cd2-9339-0c04a5b6117f/events.jsonl` の88、94、99、119、134行目、`evidence/inspection-schema.json`、repair md、recovery planに退避した。終端時点ではモデルが派生CSVを `output/` へ移動しており、退避物もその最終状態をそのまま保存している。

### TS8-2 / DATA-7隣接: 派生メトリクスverifyのscaffold枯渇（Run 6）

Run 6はphase 1/3を完了した。phase 2の補正再計画では、attempt 1 / 2がverify 0本となって `verify step requires at least one verify command`、attempt 3が次の実測コマンドでpolicy拒否された。

```text
original_command: python -c "import pandas as pd; df = pd.read_csv('output/monthly_metrics.csv'); assert 'month' in df.columns; assert 'total_sales' in df.columns; assert 'mom_pct' in df.columns; assert 'moving_avg' in df.columns; print('Verification passed')"
violation_kind: shell_control_syntax
normalized_commands: []
step_id: verify-metrics-output
```

`results.json` / `inspection.json` の契約キーassertではなく、派生物 `output/monthly_metrics.csv` の列assertである。最終原文は次のとおり。

```text
phase scaffold failed: invalid StepPlan after corrective retries: verify command may not use shell control syntax; allowed alternatives: use one deterministic command such as `npm run build`, `cargo test`, `python -m compileall -q src`, or `test -f relative/path`; split multiple checks into separate verify commands; incomplete; Recovery artifact check: prompt_parse_ok=true, yaml_parse_ok=true, command_targets_valid=true
Paths:
- repair prompt saved: .anvil/repairs/repair-phase-monthly-metrics-calculation-019f68c4-332a-73b0-b788-dcf1896350c1.md
- Recovery UltraPlan YAML saved: .anvil/plans/recovery-ultra-plan-phase-monthly-metrics-calculation-019f68c4-332a-73b0-b788-dd05d48dac5f.yaml
Commands:
- suggested command: /ultra-plan-run --profile data "$(cat .anvil/repairs/repair-phase-monthly-metrics-calculation-019f68c4-332a-73b0-b788-dcf1896350c1.md)"
- suggested YAML command: /run-ultra-plan .anvil/plans/recovery-ultra-plan-phase-monthly-metrics-calculation-019f68c4-332a-73b0-b788-dd05d48dac5f.yaml
```

一次資料は `artifacts/data8_ts_gemma31_none_001/.anvil/runs/019f68b4-c21f-7bc0-a015-3d9de0ab6b61/events.jsonl` の85、88、92、94、96〜101行目、repair md、recovery planに退避した。同runのphase 1でも別の `python -c` 存在確認が同じpolicyで拒否されたが、同一run内の補正でphase 1は完了している。

## E系 evidence

| run | pipeline probe | E1 `reconciliation` | E2 `claims-binding` | E3 `rerun-consistency` | E4 `results-schema` / final-bound |
|---|---|---|---|---|---|
| qwen35 profile 1 | なし | なし | なし | なし | なし |
| qwen35 profile 2 | なし | なし | なし | なし | なし |
| gemma31 profile | なし | なし | なし | なし | なし |
| qwen35 none 1 | なし | なし | なし | なし | なし |
| qwen35 none 2 | なし | なし | なし | なし | なし |
| gemma31 none | なし | なし | なし | なし | なし |

`inspection-schema.json` はE1〜E4の最終受け入れ一式とは別の工程evidenceで、Run 1 / 2 / 4 / 5に存在する。E1〜E4、pipeline probe、`data-assurance.json`、`ultra_final_acceptance` の到達率はすべて0/6である。

## assurance監査

契約 `docs/data-profile-contract.md` §4に従い、fullはE1〜E4全PASS、partialはpipeline実行成功＋E1/E3 PASSでE2またはE4未達、staticはscript生成済みだがprobe未完、failedは実行失敗等として照合した。

| run | pipeline/main.py | final probe / E1〜E4 | terminal assurance / 根拠 | 契約§4判定 |
|---|---:|---|---|---:|
| qwen35 profile 1 | あり | 未実行 / 未到達 | static / `data_profile_probe_not_run` | 準拠 |
| qwen35 profile 2 | あり | 未実行 / 未到達 | static / `data_profile_probe_not_run` | 準拠 |
| gemma31 profile | なし | 未実行 / 未到達 | failed / `data_profile_script_not_generated` | 準拠 |
| qwen35 none 1 | あり | 未実行 / 未到達 | static / `data_profile_probe_not_run` | 準拠 |
| qwen35 none 2 | なし | 未実行 / 未到達 | failed / `data_profile_script_not_generated` | 準拠 |
| gemma31 none | あり | 未実行 / 未到達 | static / `data_profile_probe_not_run` | 準拠 |

Run 4 / 6は生成過程でpipelineを実行しresults/reportも作成したが、最終の隔離probe、E1、E3 evidenceが存在しないためpartial以上を名乗っていない。これは保守的な過少投影ではなく契約§4どおりのstaticである。full方向のB-2j投影を実戦確認するrunはなかったが、インフレ・デフレは0件だった。

## 既知クラス再発監査

| クラス | 件数 | 事実 |
|---|---:|---|
| DATA-1〜6 | 0 | events / terminal stop reasonに既知の機械起因終端形なし |
| DATA-7 | **1（隣接形）** | Run 6が `shell_control_syntax` verify拒否の補正枯渇でterminal。派生 `monthly_metrics.csv` の列assertという第2族固有形 |
| DATA-8 | 0 | `.anvil` hidden-path blockによるterminalなし |
| DATA-9 | 0 | pipeline traceback / `pipeline_exit_nonzero` によるterminalなし |
| DATA-10残存分散（記録のみ） | 2 | Run 3はinspection未生成、Run 5はmultiple-input失敗後に、どちらも `write_required exhausted for output/inspection.json` |
| DATA-11 | 0（未到達） | final phase到達0、E系PASS後のinspection誤束縛0。実行カバレッジなし |
| DATA-12 | 0 | `step_short_circuited` 5件は全て `pre_satisfied_verified` / verify PASS。expected paths全存在＋実verify PASS可能stepでのread-only枯渇0 |

`verify_default_bound` は6件、`step_short_circuited` は5件。短絡5件の `verification_summary.failure_count` は全て0だった。Run 4のread-only停止stepはverify commandを持たないimplement stepであり、DATA-12の「実verifyを実行してPASS可能」という条件を満たさない。Run 6の空verify attempt 1 / 2では当該動的phaseに束縛可能なcheckがなく、無条件空passにはならずlint errorを維持した。

## 第1族との対比

| 指標 | 第1族 UAT #7 | 第2族 UAT #8 |
|---|---:|---:|
| run | 6 | 6 |
| completed / full | 2 / 2 | 0 / 0 |
| final acceptance到達 | 2 | 0 |
| E2 evidence | 2本、72/72 PASS | 0本 |
| `%` claim実戦 | 対象外 | report上11件、E2未到達 |
| 主な停止 | inspection非追従2、tool引数1、artifact不足1 | artifact recovery 2、read-only 2、multiple-input 1、verify scaffold 1 |
| scenario固有のrecord-only意味差 | 未計測 | report生成2本とも負値を採用し参照値と不一致 |

UAT #8は第2シナリオ族の分布として0/6 fullを記録する。第1族の既存バンド値は変更せず、族別バンドへの反映は指示どおりレビュー後の別タスクに委ねる。

## 退避物と不変条件

- 各runの `.anvil/` 全体（plans / repairs / runs/events.jsonl / summary）、存在した `pipeline/`、`output/`、`evidence/`、`scripts/`、run直下の補助check、入力CSVとSHA記録を `artifacts/<run名>/` に退避した。
- 元ワークスペースは読み取りと実行時のモデル生成以外に、回収処理による変更・削除を行っていない。
- リポジトリの `src/`、`tests/`、`docs/` は変更していない。
- イベント統合値は `run_start=6`、`run_stop=6`、`time_profile=6`、`ultra_final_acceptance=0`、`planner_error=6`、`step_short_circuited=5`、`verify_default_bound=6`、`read_only_stagnation_feedback=17`、`read_only_tool_rejected=5` だった。
