# data UAT #9 — 第2族（時系列）再計測（uat-test0716-data-009）計測レポート

## 結論

指定された6 runを新規計測ワークスペースで、指定順 `1→4→2→5→3→6`、各1回だけ実行した。6/6が `run_stop`、`status=failed`、`failure_kind=process_failure` と具体的な停止理由を残して正直終端した。完走・最終受け入れ到達・fullはいずれも0/6だった。

事前宣言した判定は、P0-a **PASS**、P0-b **PASS**、P0-c **PASS**、P1-a **PASS（直接カバレッジ限定）**、P1-b **PASS** である。UAT #8を支配した `multiple_inputs` と引用payload内のセミコロンに対する `shell_control_syntax` 拒否は、ともに0件だった。引用内に `;` を含む `python -c` verifyは4件生成され、4件とも `verify_canonicalized` まで到達してpolicy拒否されなかった。

一方、`claims-binding.json` は6本すべてで存在せず、E2の%照合は今回も実戦未到達だった。数値付き%を含むreportはRun 6の1本、%クレームは5件で、対応する値は `results.json.values` に存在したが、E2自身による `percent=true`、`matched_key`、照合結果は記録されていない。第2族はUAT #8と合算してfull 0/12、E2 evidence 0/12である。

| 判定 | 結果 | 計測事実 |
|---|---:|---|
| P0-a: 6/6 正直終端 | **PASS** | 6/6に `run_stop`、`status=failed`、`failure_kind=process_failure`、具体的な `stop_reason`。panic・分類不能終端・理由なき中断は0 |
| P0-b: assurance契約§4準拠 | **PASS** | pipeline未生成3本はfailed、pipeline生成済み・final probe未実行3本はstatic。partial/fullは0で、インフレ・デフレとも0 |
| P0-c: 偽成功ゼロ | **PASS** | fullを名乗るrunは0。`ultra_final_acceptance` 0件、E1〜E4 evidence一式を持つrunも0 |
| P1-a: DATA-13型ゼロ | **PASS（直接カバレッジ限定）** | `multiple_inputs` 0件。inspection checkを実行した2本はいずれも `input_path=data/sales.csv`。派生 `output/cleaned.csv` を作ったRun 6はinspection check未実行のため、派生CSV共存下の入力選択evidenceは未取得 |
| P1-b: DATA-7b型ゼロ | **PASS** | `shell_control_syntax` / `verify_command_policy_error` 0件。引用内`;`を持つverify 4件はcanonical/advisoryとして受理。クォート外制御構文のverifyは生成されず、拒否維持の実戦観測はN/A |
| 記録: full率 | **0/6** | profile 0/3、none 0/3 |
| 記録: E2 %照合 | **未実戦** | report 1本に数値付き%クレーム5件、claims-binding evidence 0/6 |
| 記録: DATA-10残存分散 | **2/6** | Run 1は不正なinspection内容を修復できず、Run 3はinspection未生成のままwrite-required枯渇 |

## 計測条件

- 対象: `develop`、HEAD `2028eb40452e77ccb32f436cec81e81cbf6ef3ca`（`2028eb4 Make verify shell lint quote-aware`、計測開始時に `origin/develop` と一致）
- バイナリ: `commandagent 0.1.0 2028eb4 2026-07-16T04:43:54Z`（`+dirty` なし）
- release/install binary SHA-256: `7bfdf7107c7eacbd51c22681366786424d1952b86886ac7b83684448635566b1`（両者一致）
- 計測ワークスペース: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0716_data_009`。実行前に不存在を確認して新規作成
- 成果物先: `workspace/management/runs/uat-test0716-data-009/`
- 共通入力 SHA-256: `2f6c04e42b0ebdff85a7eb6b52a342610155be6796bd89e5729075d87c78d873`。生成直後と退避後の6/6で一致
- 共通 planner: `qwen3.6:27b-coding-nvfp4` / `ollama`
- 共通 profile/provider/context: `data` / `ollama` / `65536`
- 共通 goal: `data/sales.csv を読み込み、月次の売上合計・前月比（%）・3ヶ月移動平均を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。`
- 外側のcommandagent invocationは各run 1回、再試行0回。製品内のbounded repair/corrective planningはeventsにそのまま残した
- 実行順: Run 1 → 4 → 2 → 5 → 3 → 6
- 所要時間は各runの `time_profile.profile.total_ms`。6本合計は3302.206秒

### Preflight

計測前に存在した別タスクの未追跡ディレクトリ `workspace/management/runs/uat-test0715-ff1-001/` は、ユーザー確認後にstash commit `b1443249bcb0c48f7050455d694d881160333f94`（`On develop: isolate pre-existing uat-test0715-ff1-001 for data UAT 9`）へ一時隔離した。その後の `git status --porcelain` が空であることを確認した。このstashは本UATコミットに含めず、提出コミット後に復元する。

| 項目 | 結果 |
|---|---|
| `git status --porcelain` | 上記の一時隔離後に空 |
| `git log -1 --oneline` | `2028eb4 Make verify shell lint quote-aware` |
| `cargo test` | browser probeを実行可能な権限付き環境でexit 0。主要lib 1344 passed / 13 ignored、conformance 18 passed / 1 ignored、data profile 10/10、quote-aware integration 2/2を含むfull suite失敗0 |
| `cargo build --release` | exit 0 |
| install | `install -m 755 target/release/commandagent /Users/maenokota/.local/bin/commandagent` 成功 |
| `commandagent --version` | `commandagent 0.1.0 2028eb4 2026-07-16T04:43:54Z` |
| Ollama models | planner `qwen3.6:27b-coding-nvfp4`、executor `qwen3.6:35b-a3b-coding-nvfp4` / `gemma4:31b` の存在を確認 |

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
| 1 | `data9_ts_qwen35_profile_001`<br>`019f6940-466d-7912-9d32-6f4369fbfeeb` | qwen3.6:35b-a3b-coding-nvfp4 / profile | 1 | failed / not_checked | failed (`data_profile_script_not_generated`) | 0/5 | `data-inspection`: 24 vs 60行・`distinct_values.date`不足を修復できずwrite-required枯渇 | 552.440 s |
| 2 | `data9_ts_qwen35_profile_002`<br>`019f6951-e16e-7fc0-84a9-86f7657258ba` | qwen3.6:35b-a3b-coding-nvfp4 / profile | 1 | failed / not_checked | static (`data_profile_probe_not_run`) | 2/5 | `data-aggregation`: `pipeline/main.py`へのwrite-required枯渇 | 675.574 s |
| 3 | `data9_ts_gemma31_profile_001`<br>`019f696a-42fd-77f1-af4c-9bb5066c0490` | gemma4:31b / profile | 1 | failed / not_checked | failed (`data_profile_script_not_generated`) | 0/5 | `data-inspection`: `inspection.json`未生成、Read/Bash反復でwrite-required枯渇 | 283.641 s |
| 4 | `data9_ts_qwen35_none_001`<br>`019f6949-5bc7-7f61-b268-5368c68320b0` | qwen3.6:35b-a3b-coding-nvfp4 / none | 1 | failed / not_checked | static (`data_profile_probe_not_run`) | 0/3 | `data-validation-and-cleaning`: `results.json` / `report.md`のartifact follow-through枯渇 | 524.654 s |
| 5 | `data9_ts_qwen35_none_002`<br>`019f6961-6c4e-7020-a97b-5af570d660db` | qwen3.6:35b-a3b-coding-nvfp4 / none | 1 | failed / not_checked | failed (`data_profile_script_not_generated`) | 0/4 | `load-and-validate-data`: pipeline書き込み前にartifact recovery枯渇 | 530.102 s |
| 6 | `data9_ts_gemma31_none_001`<br>`019f696e-f39f-76d1-a408-f28fc56c028b` | gemma4:31b / none | 1 | failed / not_checked | static (`data_profile_probe_not_run`) | 1/4 | `metrics-calculation`: pipeline更新をせずRead/Bash反復、`no_progress_recorded` | 735.795 s |

完了phase深度の分布は0 phaseが4本、1 phaseが1本、2 phaseが1本。最終受け入れ到達、completed terminal、fullはいずれも0本だった。

### 終端理由の一次資料

各runの完全な `run_stop.stop_reason` は次のeventsに保存されている。

| run | events / `run_stop` 行 |
|---|---|
| Run 1 | `artifacts/data9_ts_qwen35_profile_001/.anvil/runs/019f6940-466d-7912-9d32-6f4369fbfeeb/events.jsonl:102` |
| Run 2 | `artifacts/data9_ts_qwen35_profile_002/.anvil/runs/019f6951-e16e-7fc0-84a9-86f7657258ba/events.jsonl:176` |
| Run 3 | `artifacts/data9_ts_gemma31_profile_001/.anvil/runs/019f696a-42fd-77f1-af4c-9bb5066c0490/events.jsonl:59` |
| Run 4 | `artifacts/data9_ts_qwen35_none_001/.anvil/runs/019f6949-5bc7-7f61-b268-5368c68320b0/events.jsonl:102` |
| Run 5 | `artifacts/data9_ts_qwen35_none_002/.anvil/runs/019f6961-6c4e-7020-a97b-5af570d660db/events.jsonl:56` |
| Run 6 | `artifacts/data9_ts_gemma31_none_001/.anvil/runs/019f696e-f39f-76d1-a408-f28fc56c028b/events.jsonl:133` |

## B-2k監査

### DATA-13: goal参照入力の優先

全events、inspection evidence、repair文書に `multiple_inputs` は0件だった。`data_inspection_schema` がevidenceを書いた2本の入力特定結果は次のとおり。

| run | schema結果 | evidenceの入力特定 | failure |
|---|---|---|---|
| Run 1 | FAIL | `input_path: data/sales.csv` | `input_row_count_mismatch:expected=60:reported=24`、`distinct_values_missing_categorical_columns:date` |
| Run 2 | PASS | `input_path: data/sales.csv` | なし |

原文はそれぞれ `artifacts/data9_ts_qwen35_profile_001/evidence/inspection-schema.json` と `artifacts/data9_ts_qwen35_profile_002/evidence/inspection-schema.json` に退避した。両方ともgoalが名指した実在パス `data/sales.csv` を入力として記録している。

派生表形式ファイルを生成したrunはRun 6だけで、`output/cleaned.csv` が存在する。Run 6では `data_inspection_schema` 自体に到達せず、`inspection-schema.json` は存在しない。このため「派生CSV共存時にもgoal参照入力を選んだ」という直接evidenceは本計測では取得できていない。ただしRun 6を含む全runで `multiple_inputs` terminalは0で、事前宣言したP1-aの再発ゼロ条件は満たした。

### DATA-7b: 引用payload内の制御文字

引用内に`;`を含むverifyは4件あった。全件が `verify_canonicalized` として記録され、`shell_control_syntax` / `verify_command_policy_error` は発生していない。

| run / step | original | 判定 |
|---|---|---|
| Run 2 / `verify-results-schema` | `python -c 'import json; d=json.load(open("output/results.json")); assert "reconciliation" in d and "values" in d; r=d["reconciliation"]; assert r["input_rows"]==r["used_rows"]+sum(e["rows"] for e in r["excluded"])'` | `verify_canonicalized`, `disposition=advisory`。policy拒否なし |
| Run 3 / `verify-inspection-schema` | `python -c "import json; d=json.load(open('output/inspection.json')); assert set(d.keys()) == {'column_names','input_row_count','type_summaries','distinct_values','sample_rows'}; assert len(d['sample_rows']) > 0; print('ok')"` | `replacement=anvil-catalog-check:data_inspection_schema`, `disposition=canonical`。policy拒否なし |
| Run 6 / `verify-results` | `python -c "import json; d=json.load(open('output/results.json')); r=d['reconciliation']; assert r['input_rows']==r['used_rows']+sum(e['rows'] for e in r['excluded']); assert 'values' in d"` | `verify_canonicalized`, `disposition=advisory`。policy拒否なし |
| Run 6 / `verify-pipeline` | `python -c "import json; d=json.load(open('output/results.json')); assert 'reconciliation' in d and 'values' in d"` | `verify_canonicalized`, `disposition=advisory`。policy拒否なし |

原文の所在はRun 2 events 126行目、Run 3 events 10行目、Run 6 events 14 / 67行目である。特にRun 6はUAT #8で同型の引用payloadが拒否されたexecutor/preset構成で、今回はPhase 1を完了している。

クォート外制御構文を含むverifyは本計測では生成されなかったため、拒否維持の実戦観測はN/Aである。Run 6のimplement stepでは次のruntime Bashがあったが、`step_kind=implement`、`verifier_policy_checked=false`、`deterministic_verifier_evidence=false` であり、verify lintの拒否維持根拠には数えない。

```text
ls -R data pipeline output 2>/dev/null || ls -R
```

一次資料はRun 6 events 84〜85行目にある。

## E2 %クレーム監査

`claims-binding.json` は6 runすべてで存在しない。数値付き%クレームを含むreportはRun 6の `output/report.md` だけで、次の5件がある。表の対応候補はreportとresultsを本レポート作成時に機械的に対応させたものであり、E2 evidenceの `matched_key` ではない。

| %クレーム原文 | `percent=true` | `results.json.values` 対応候補 | E2照合結果 |
|---|---|---|---|
| `18.34%` | 未記録 | `mom_2026-02=18.34417208604302` | 未実行 |
| `-12.37%` | 未記録 | `mom_2026-03=-12.37265925518874` | 未実行 |
| `-18.84%` | 未記録 | `mom_2026-04=-18.842257597684515` | 未実行 |
| `27.62%` | 未記録 | `mom_2026-05=27.61531145981931` | 未実行 |
| `-7.93%` | 未記録 | `mom_2026-06=-7.931998136935259` | 未実行 |

5件は表示桁への丸めでは対応候補と一致する。しかしE2未実行のため、quantity claimとして認識されたか、`percent=true` で正規化されたか、月ラベルが `date_label` へ分離されたか、照合がPASSしたかは判定できない。violationもPASSも記録されていないため、「偽陽性ゼロ」や「%照合成功」を主張せず、record-only結果を **未実戦** とした。

## 意味的正解との対比（record-only）

fullまたはE2 PASSのrunは0本なので、#8と同じ正式条件の対比対象は存在しない。参考として、report/resultsを生成したRun 6を独立参照値と比較する。これは合否およびassuranceに用いない。

| 項目 | Run 6 | 独立参照 | record-only結果 |
|---|---|---|---|
| reconciliation | input 60 / used 58 / excluded 2 | input 60 / used 56 / excluded 4 | 不一致 |
| monthly | Jan 19990、Feb 23657、Mar 20730、Apr 16824、May 21470、Jun 19767 | Jan 19990、Feb 18657、Mar 20730、Apr 17324、May 21470、Jun 19767 | 4/6一致。Feb +5000、Apr -500 |
| mom% | Feb 18.34、Mar -12.37、Apr -18.84、May 27.62、Jun -7.93 | Feb -6.67、Mar 11.11、Apr -16.43、May 23.93、Jun -7.93 | 1/5一致 |
| ma3 | Mar 21459.00、Apr 20403.67、May 19674.67、Jun 19353.67 | Mar 19792.33、Apr 18903.67、May 19841.33、Jun 19520.33 | 0/4一致。Run 6はJan/Febにも短い窓の値を出力 |

Run 6の `pipeline/main.py` は空値2件のみを除外し、不正日付 `2026-02-30` と負値 `-500` を採用した。これはdata契約§2/§7の意味的正しさのスコープ外であり、record-only観測として記録する。

## inspection監査

| run | inspection成果物 | `data_inspection_schema` | 入力行数 / 入力path | 最終状態 |
|---|---|---|---|---|
| Run 1 qwen35/profile | 規定5キー | FAIL | reported 24 / expected 60、`data/sales.csv` | `distinct_values.date`不足を含め修復できず枯渇 |
| Run 2 qwen35/profile | 規定5キー | PASS | 60 / `data/sales.csv` | inspection phase完了 |
| Run 3 gemma31/profile | なし | 未実行 | N/A | inspection書き込み前に枯渇 |
| Run 4 qwen35/none | 独自schema | 未実行 | `observations.total_rows=22` | final成果物不足でphase 1停止 |
| Run 5 qwen35/none | なし | 未実行 | N/A | pipeline書き込み前に停止 |
| Run 6 gemma31/none | 独自4キー | 未実行 | `total_input_rows=60` | phase 1完了、phase 2で停止 |

inspection evidenceは2/6、PASSは1/2である。Run 1のfailureは `multiple_inputs` ではなく、内容の行数・categorical distinct検査が機能した結果である。

## E系 evidence

| run | pipeline probe | E1 `reconciliation` | E2 `claims-binding` | E3 `rerun-consistency` | E4 `results-schema` / final-bound |
|---|---|---|---|---|---|
| qwen35 profile 1 | なし | なし | なし | なし | なし |
| qwen35 profile 2 | なし | なし | なし | なし | なし |
| gemma31 profile | なし | なし | なし | なし | なし |
| qwen35 none 1 | なし | なし | なし | なし | なし |
| qwen35 none 2 | なし | なし | なし | なし | なし |
| gemma31 none | なし | なし | なし | なし | なし |

`inspection-schema.json` はE1〜E4の最終受け入れ一式とは別の工程evidenceで、Run 1 / 2に存在する。E1〜E4、pipeline probe、`data-assurance.json`、`ultra_final_acceptance` の到達率はすべて0/6である。

## assurance監査

契約 `docs/data-profile-contract.md` §4に従い、fullはE1〜E4全PASS、partialはpipeline実行成功＋E1/E3 PASSでE2またはE4未達、staticはscript生成済みだがprobe未完、failedはscript未生成等として照合した。

| run | pipeline/main.py | final probe / E1〜E4 | terminal assurance / 根拠 | 契約§4判定 |
|---|---:|---|---|---:|
| qwen35 profile 1 | なし | 未実行 / 未到達 | failed / `data_profile_script_not_generated` | 準拠 |
| qwen35 profile 2 | あり | 未実行 / 未到達 | static / `data_profile_probe_not_run` | 準拠 |
| gemma31 profile | なし | 未実行 / 未到達 | failed / `data_profile_script_not_generated` | 準拠 |
| qwen35 none 1 | あり | 未実行 / 未到達 | static / `data_profile_probe_not_run` | 準拠 |
| qwen35 none 2 | なし | 未実行 / 未到達 | failed / `data_profile_script_not_generated` | 準拠 |
| gemma31 none | あり | 未実行 / 未到達 | static / `data_profile_probe_not_run` | 準拠 |

Run 6はpipelineを実行しresults/reportも生成したが、最終の隔離probe、E1、E3 evidenceが存在しないためpartial以上を名乗っていない。full相当evidenceを持つrunがないため、B-2jのfull方向投影を実戦確認する対象はなかった。インフレ・デフレはいずれも0件だった。

## 既知クラス再発監査

| クラス | 件数 | 事実 |
|---|---:|---|
| DATA-1〜6 | 0 | events / terminal stop reasonに既知の機械起因終端形なし |
| DATA-7b | 0 | 引用内`;`のpolicy拒否0。対象verify 4件はcanonical/advisory判定へ到達 |
| DATA-8〜9 | 0 | hidden-path block / pipeline traceback起因terminalなし |
| DATA-10残存分散（記録のみ） | 2 | Run 1はinspection内容修復非追従、Run 3はinspection未書き込み |
| DATA-11 | 0（未到達） | final acceptance到達0、E系PASS後のinspection誤束縛0。実行カバレッジなし |
| DATA-12 | 0 | `step_short_circuited` 4件は全て `pre_satisfied_verified`。事前充足verify可能stepのread-only枯渇なし |
| DATA-13 | 0 | `multiple_inputs` 0。inspection evidenceの入力pathは2/2でgoal参照 `data/sales.csv` |

`verify_default_bound` は5件、`step_short_circuited` は4件。今回の停止はinspection内容・モデル書き込み非追従・artifact follow-throughで、DATA-13 / DATA-7bの機械偽陽性はterminalを支配していない。

第1族およびUAT #8までの記録にない新規の機械起因クラスは、本計測では付番しない。Run 1 / 3のinspection非追従、Run 2の既存成果物へのwrite非追従、Run 4 / 5のartifact follow-through、Run 6のno-progressはいずれも既存の停止語彙と一次資料で分類できる。

## UAT #8との対比

| 指標 | UAT #8 | UAT #9 | #8+#9 |
|---|---:|---:|---:|
| run | 6 | 6 | 12 |
| completed / full | 0 / 0 | 0 / 0 | 0 / 0 |
| final acceptance到達 | 0 | 0 | 0 |
| E2 evidence | 0 | 0 | 0 |
| report上の数値付き%クレーム | 11 | 5 | 16 |
| `multiple_inputs` terminal | 1 | 0 | 1 |
| 引用内`;`の `shell_control_syntax` terminal | 1 | 0 | 1 |
| inspection非追従 | 2 | 2 | 4 |

UAT #9ではB-2k対象の2つの機械偽陽性終端は消滅したが、full率とE2到達率は0のままだった。第2族の族別バンド生成・台帳・bandファイルは指示どおり変更していない。

## イベント統合値

6本のevent語彙を統合した主要値は次のとおり。

```text
run_start                         6
run_stop                          6
time_profile                      6
ultra_phase_start                 9
ultra_phase_complete              3
ultra_phase_failed                6
ultra_final_acceptance            0
phase_verification_result         3
verify_canonicalized              8
verify_command_normalized_at_runtime 3
verify_default_bound              5
step_short_circuited              4
planner_error                     1
read_only_stagnation_feedback     6
read_only_tool_rejected           6
artifact_stagnation_feedback     13
runtime_bash_policy              36
provider_turn_duration           86
```

## 退避物と不変条件

- 各runの `.anvil/` 全体（plans / repairs / runs/events.jsonl / summary）、存在した `pipeline/`、`output/`、`evidence/`、`scripts/`、`data/`、入力SHA記録を `artifacts/<run名>/` に退避した。
- 退避後の `data/sales.csv` と `sales.csv.sha256` は6/6で指定SHAに一致した。
- 元ワークスペースはモデル実行で生成された状態を保持し、回収処理による変更・削除を行っていない。
- リポジトリの `src/`、`tests/`、`docs/` は変更していない。
- fullは0本のため、「fullが出た場合の全evidence完全転記」は対象なし。
