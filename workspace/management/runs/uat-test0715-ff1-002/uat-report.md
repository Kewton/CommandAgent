# data UAT #4（uat-test0715-data-004）計測レポート

## 結論

指定された6 runを各1回だけ実行した。全runが分類済みの `run_stop` と回復資料を残して正直終端したが、成功runおよび `full` は0/6だった。

事前宣言した判定は、P0-a **PASS**、P0-b **PASS**、P0-c **FAIL**、P1-a **PASS**、P1-b **PASS** である。P0-c は `data4_gemma31_profile_001` が `data-inspection` で `model_stagnation:read_only_loop: write_required exhausted for output/inspection.json` に到達し、DATA-10型が1/6で再発したため不合格とした。

| 判定 | 結果 | 計測事実 |
|---|---:|---|
| P0-a: 6/6 正直終端 | PASS | 6/6に `run_stop`、分類済み `process_failure`、具体的な `stop_reason`、repair plan/promptがある。panic・分類不能終端・理由なき中断は0件 |
| P0-b: assurance契約§4準拠 | PASS | パイプライン未実行4 runはいずれも `static` または `failed`。`partial` / `full` の過大申告は0件 |
| P0-c: DATA-10型再発ゼロ | **FAIL** | Run 2で `data-inspection` の `write_required` が `output/inspection.json` を対象に枯渇。再発1件 |
| P1-a: noneアームのシェル構文即死減少＋テレメトリ | PASS | none 4 runのシェル構文起因の即死は0/4（前回 m4_003 は4/4）。noneで `verify_policy` の `original_command` / `violation_kind` を2件記録。runtime書き換えはprofile runで2件観測 |
| P1-b: `data_inspection_schema` 実戦発火 | PASS | profile 2 runで実行。Run 1はPASS、Run 2は5キー列挙付きFAIL |

## 計測条件

- 計測対象: `develop`、HEAD `a5bafcb3e17a098bb7b9edfcee21282f3fd4f634` (`a5bafcb Wire contract instrumentation repair guidance`)
- バイナリ: `commandagent 0.1.0 a5bafcb 2026-07-15T03:10:46Z`（`+dirty` なし）
- release binary SHA-256: `80da779f8346eb4879796b43da9f20cfeeeb270da3a36b8896d243f3c7756d16`（build/install先で一致）
- 計測ワークスペース: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0715_data_004`（実行前に不存在を確認して新規作成）
- 成果物先: ユーザーの後続指定により `workspace/management/runs/uat-test0715-ff1-002/`。UAT識別子は元タスクどおり `uat-test0715-data-004`
- 共通入力 SHA-256: `2f6c04e42b0ebdff85a7eb6b52a342610155be6796bd89e5729075d87c78d873`。6/6で一致
- 共通 planner: `qwen3.6:27b-coding-nvfp4` / `ollama`
- 共通 profile: `data`
- 共通 goal: `data/sales.csv を読み込み、月次×地域の売上集計と全体合計を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。`

### Preflight

| 項目 | 結果 |
|---|---|
| `git status --porcelain` | 空 |
| `cargo test` | exit 0。lib 1303 passed / 13 ignored、全integration/doc targetを含め失敗0 |
| `cargo build --release` | exit 0 |
| `install -m 755 target/release/commandagent ~/.local/bin/commandagent` | 成功 |
| `commandagent --version` | `commandagent 0.1.0 a5bafcb 2026-07-15T03:10:46Z` |

## Run行列

所要時間は各コマンドを `/usr/bin/time -p` で外側から計測した `real` 値である。再試行は行っていない。

| # | run / run id | executor / preset | exit | terminal / final acceptance | assurance | 失敗クラス（主要終端） | 所要時間 |
|---:|---|---|---:|---|---|---|---:|
| 1 | `data4_qwen35_profile_001`<br>`019f63c4-2752-7c01-9383-36604efd70c5` | qwen3.6:35b-a3b-coding-nvfp4 / profile | 1 | failed / not_checked | failed (`data_assurance_failed`) | `data-cleaning`: `model_stagnation:read_only_loop: write_required exhausted for pipeline/main.py`。`data_claims_binding`違反 | 1571.38 s |
| 2 | `data4_gemma31_profile_001`<br>`019f63dc-b091-7e80-891e-8db9d29ddd08` | gemma4:31b / profile | 1 | failed / not_checked | failed (`data_profile_script_not_generated`) | `data-inspection`: `model_stagnation:read_only_loop: write_required exhausted for output/inspection.json` | 306.99 s |
| 3 | `data4_qwen35_none_001`<br>`019f63e1-c4d1-76a0-8627-36a7f40e6fb6` | qwen3.6:35b-a3b-coding-nvfp4 / none | 1 | failed / not_checked | static (`data_profile_probe_not_run`) | `inspect-schema-and-rules`: `artifact_follow_through_exhausted`; missing `tests/smoke_test.py`, feedback 3 | 632.64 s |
| 4 | `data4_qwen35_none_002`<br>`019f63eb-d9e3-70a1-afd1-3e42649e3769` | qwen3.6:35b-a3b-coding-nvfp4 / none | 1 | failed / not_checked | failed (`data_assurance_failed`) | `load-and-validate-data`: `loop_progress_exhausted: model_stagnation:read_only_loop` | 939.76 s |
| 5 | `data4_gemma31_none_001`<br>`019f63fa-918e-7652-9a11-bfdcdf0cb3b7` | gemma4:31b / none | 1 | failed / not_checked | static (`data_profile_probe_not_run`) | `load-and-validate-data`: `artifact_follow_through_exhausted`; missing `output/results.json`, `output/report.md`, feedback 1 | 619.34 s |
| 6 | `data4_gemma31_none_002`<br>`019f6404-6b2b-76a1-8bde-0e65996e04dc` | gemma4:31b / none | 1 | failed / not_checked | static (`data_profile_probe_not_run`) | `inspect-and-define-schema`: `artifact_follow_through_exhausted`; missing `output/results.json`, `output/report.md`, feedback 1 | 479.71 s |

## DATA-10監査

| run | `data-inspection`フェーズ結果 | `inspection.json` | `data_inspection_schema` | inspection段 `write_required` |
|---|---|---|---|---|
| qwen35 profile | PASS (`phase_verification_result: ok=true`, `ultra_phase_complete`) | あり。5キーすべてあり、`input_row_count=60` | 初回FAIL: 5キー欠落 → repair 1 FAIL: `distinct_values_missing_categorical_columns:date` → repair 2 **PASS** | あり、3件。対象はすべて `output/inspection.json`。枯渇なし |
| gemma31 profile | **FAIL** | あり。内容は `{}` | **FAIL**: `missing_keys:column_names,input_row_count,type_summaries,distinct_values,sample_rows` | あり、2件。対象はすべて `output/inspection.json`。その後枯渇 |
| qwen35 none 1 | canonical `data-inspection` フェーズなし（dynamic phase `inspect-schema-and-rules`） | なし | 未実行 | canonical inspectionでは発火なし |
| qwen35 none 2 | canonical `data-inspection` フェーズなし（dynamic phase `load-and-validate-data`） | あり。キーは `total_input_rows`, `valid_rows`, `excluded_count`, `excluded_reasons`, `notes`。規定5キーは0/5 | 未実行 | canonical inspectionでは発火なし |
| gemma31 none 1 | canonical `data-inspection` フェーズなし（dynamic phase `load-and-validate-data`） | あり。キーは `columns`, `observations`。規定5キーは0/5 | 未実行 | canonical inspectionでは発火なし |
| gemma31 none 2 | canonical `data-inspection` フェーズなし（dynamic phase `inspect-and-define-schema`） | あり。内容は `{}` | 未実行 | canonical inspectionでは発火なし |

profile 2 runの変換後planでは、`data-inspection` の全stepについて次が一致した。

- `expected_paths`: `output/inspection.json` のみ
- `verify`: `anvil-catalog-check:data_inspection_schema` と `test -f output/inspection.json` のみ
- `pipeline/main.py`, `output/results.json`, `output/report.md` はinspection stepの `expected_paths` / `verify` に含まれない

したがって、inspection stepへの最終成果物の混入は今回のprofile 2 runでは観測されなかった。一方、Run 2は内容検証の失敗後に同じ対象への書き込みを完了できず、DATA-10判定条件に該当した。最終フェーズにはどのrunも到達していないため、最終フェーズでの全契約チェック束縛は本UATでは未観測である。

## 書き換えイベント

`verify_command_normalized_at_runtime` をrunごと・`normalization_kind` ごとに数えた。

| run | `shell_control_split` | `stderr_suppression_stripped` | `fallback_true_stripped` |
|---|---:|---:|---:|
| qwen35 profile | 1 | 1 | 0 |
| gemma31 profile | 0 | 0 | 0 |
| qwen35 none 1 | 0 | 0 | 0 |
| qwen35 none 2 | 0 | 0 | 0 |
| gemma31 none 1 | 0 | 0 | 0 |
| gemma31 none 2 | 0 | 0 | 0 |
| **合計** | **1** | **1** | **0** |

原文例1（`stderr_suppression_stripped`。同時に末尾pipeも縮約）:

```text
find /Users/maenokota/share/work/localwork/commandagent_mvp/01/test0715_data_004/data4_qwen35_profile_001 -type f 2>/dev/null | head -50
→ ["find /Users/maenokota/share/work/localwork/commandagent_mvp/01/test0715_data_004/data4_qwen35_profile_001 -type f"]
```

原文例2（`shell_control_split`）:

```text
ls -la /Users/maenokota/share/work/localwork/commandagent_mvp/01/test0715_data_004/data4_qwen35_profile_001/pipeline/ 2>/dev/null; echo "EXIT:$?"
→ ["ls -la /Users/maenokota/share/work/localwork/commandagent_mvp/01/test0715_data_004/data4_qwen35_profile_001/pipeline/", "echo \"EXIT:$?\""]
```

`fallback_true_stripped` の実戦発火例は存在しない。

## verify lint / runtime bash policy拒否

`runtime_bash_policy` イベントは40件あり、`blocked=true` の拒否は0件だった。plannerのverify policy拒否は2件、後続lint拒否は1件だった。

1. `data4_qwen35_none_002`, step `verify-results-schema`
   - `violation_kind`: `shell_control_syntax`
   - `normalized_commands`: `[]`
   - `original_command`:

```text
python -c "import json; d=json.load(open('output/results.json')); assert 'reconciliation' in d and 'values' in d; r=d['reconciliation']; assert r['input_rows']==r['used_rows']+sum(e['rows'] for e in r['excluded']); print('ok')"
```

2. `data4_gemma31_none_002`, step `verify-artifacts`
   - `violation_kind`: `shell_control_syntax`
   - `normalized_commands`: `[]`
   - `original_command`:

```text
python -c "import json, os; assert os.path.exists('output/report.md'); assert os.path.exists('output/inspection.json'); d=json.load(open('output/results.json')); assert 'reconciliation' in d and 'values' in d; assert d['reconciliation']['input_rows'] == d['reconciliation']['used_rows'] + sum(e['rows'] for e in d['reconciliation']['excluded'])"
```

3. `data4_qwen35_none_002`, planner lint repair attempt 2
   - message: `verify step requires at least one verify command`
   - このイベントには `original_command` / `violation_kind` フィールドが存在しない

参考として、verify policyとは別の `bash_path_confinement_rejected` は3件だった。Run 3で旧ワークスペース名 `test0715_data_001` を参照する `cat` / `ls` が2件、Run 4で解決後パスが `/Users/maenokuta/...` となるパス誤記が1件である。

## E系 evidence

| run | `pipeline-run.json` | `reconciliation.json` | `claims-binding.json` | `rerun-consistency.json` |
|---|---|---|---|---|
| qwen35 profile | あり / PASS。`python3 -B pipeline/main.py`, exit 0, 52 ms | あり / PASS。`60 = 57 + 3`; `invalid_date=1`, `missing_amount=1`, `missing_date=1` | あり / FAIL。30 violations | なし |
| gemma31 profile | なし | なし | なし | なし |
| qwen35 none 1 | なし | なし | なし | なし |
| qwen35 none 2 | あり / PASS。`python3 -B pipeline/main.py`, exit 0, 109 ms | あり / PASS。`60 = 56 + 4`; `invalid_date=1`, `missing_amount=1`, `missing_date=1`, `non_positive_amount=1` | あり / FAIL。19 violations | なし |
| gemma31 none 1 | なし | あり / FAIL。`output/results.json` 不在、input/usedはnull | あり / FAIL。`output/results.json` 不在 | なし |
| gemma31 none 2 | なし | あり / FAIL。`output/results.json` 不在、input/usedはnull | あり / FAIL。`output/results.json` 不在 | なし |

到達率は `pipeline-run` 2/6（PASS 2）、`reconciliation` 4/6（PASS 2）、`claims-binding` 4/6（PASS 0）、`rerun-consistency` 0/6である。事前期待 `60/56/4` と一致したのはRun 4のみ。Run 1は `60/57/3` で、負数行を除外していない内訳だった。

## assurance監査

契約§4との照合では、`full` はE1〜E4すべての成功、`partial` はパイプライン成功および所定evidenceの成功、`static` はスクリプト生成済みだが実行probe未完了、`failed` は実行・evidence・再現性の失敗を表すものとして照合した。

| run | assurance / 根拠 | pipeline実行 | evidence状態 | §4準拠判定 |
|---|---|---:|---|---:|
| qwen35 profile | failed / `data_assurance_failed` | あり | pipeline-run PASS、reconciliation PASS、claims-binding FAIL、rerunなし | 準拠（保守側） |
| gemma31 profile | failed / `data_profile_script_not_generated` | なし | inspection-schema FAILのみ | 準拠 |
| qwen35 none 1 | static / `data_profile_probe_not_run` | なし | E系なし | 準拠 |
| qwen35 none 2 | failed / `data_assurance_failed` | あり | pipeline-run PASS、reconciliation PASS、claims-binding FAIL、rerunなし | 準拠（保守側） |
| gemma31 none 1 | static / `data_profile_probe_not_run` | なし | results不在に対する失敗evidence | 準拠 |
| gemma31 none 2 | static / `data_profile_probe_not_run` | なし | results不在に対する失敗evidence | 準拠 |

パイプライン未実行runの `partial` 以上は0件、assuranceインフレは0件だった。

## イベント語彙（6 run統合）

```text
  19 artifact_stagnation_feedback
   3 bash_path_confinement_rejected
   9 empty_response_escalation
   5 empty_response_recovered
   7 escalation_carryover
   6 host_env_contamination
   8 inspect_command_normalized
  22 loop_stop
   1 path_fallback_evaluated
   1 phase_verification_result
   1 pipeline_error_extraction
   6 plan_preset_resolved
   3 planner_error
  49 planner_quality_issue
   2 planner_quality_retry
   3 planner_quality_warning
  12 planner_raw_output_shape
   9 preset_step_converted
   2 preset_ultra_plan_used
 118 provider_turn_duration
  10 read_only_stagnation_feedback
   6 read_only_tool_rejected
   8 recovery_prompt_saved
   6 run_start
   6 run_stop
  40 runtime_bash_policy
  22 step_obligation_scope
  18 step_prompt_contract
   3 step_short_circuited
   3 step_verify_failure
   5 step_verify_repair
   6 time_profile
   4 tool_args_path_normalized
   1 tool_args_path_salvaged
 135 tool_call_raw
 112 tool_execute
   3 tool_validation_error
   6 tui_command_stop
   6 ultra_context_initialized
   6 ultra_partial_artifact_summary
   1 ultra_phase_complete
   7 ultra_phase_context_attached
   7 ultra_phase_context_updated
   1 ultra_phase_execute_complete
   6 ultra_phase_failed
   7 ultra_phase_plan_validated
   1 ultra_phase_profile_check
   7 ultra_phase_scaffold_complete
   7 ultra_phase_start
   4 ultra_plan_generation_attempt
   4 ultra_plan_generation_metadata_normalized
   4 ultra_plan_generation_succeeded
   4 ultra_plan_raw_output_shape
  13 verify_canonicalized
   2 verify_command_normalized_at_runtime
   7 workspace_cd_stripped
```

## 記録項目

- full率: 0/6
- profile: 0/2成功、none: 0/4成功
- `output/results.json` と `output/report.md` の両方があるrun: 2/6（Run 1, Run 4）
- fullが存在しないため、「results.jsonのvaluesとレポート本文数値の照合例2〜3個」および「E1〜E4全pass確認」は該当なし
- 全runの `.anvil/`、存在した `pipeline/`、`output/`、`evidence/`、入力CSV、および入力SHA記録を `artifacts/` に退避した。存在しなかったディレクトリ／ファイルは補完していない
