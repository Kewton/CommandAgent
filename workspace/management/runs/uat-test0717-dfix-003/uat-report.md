# UAT dfix-003 — FIX-7 measurement

基準HEAD: `d2488ac`。既存ワークスペースの一次資料を各run一回の規律に従って収集した。

## Run matrix

| run | family | executor | verdict | assurance | F1 | F2 | F3 | reason |
|---|---|---|---|---|---|---|---|---|
| dfix3_pipe_qwen35_001 | pipe | qwen35 | failed | failed | passed | not executed | not executed | after_not_executed |
| dfix3_pipe_gemma31_001 | pipe | gemma31 | failed | failed | passed | not executed | not executed | after_not_executed |
| dfix3_pipe_qwen35_002 | pipe | qwen35 | failed | failed | passed | not executed | not executed | after_not_executed |
| dfix3_schema_qwen35_001 | schema | qwen35 | failed | failed | passed | not executed | not executed | after_not_executed |
| dfix3_schema_gemma31_001 | schema | gemma31 | failed | failed | passed | not executed | not executed | after_not_executed |
| dfix3_schema_qwen35_002 | schema | qwen35 | failed | failed | passed | not executed | not executed | after_not_executed |

全6本が正直な終端（P0-a PASS）。F1は全件実行・失敗確認済みだが、planner repair が `pipeline/main.py` の重複所有権または修復計画不備で停止し、F2/F3へ到達しなかった。従って P0-b/P0-c は判定不能（fullなし）、FIX-7の効果判定も未成立として停止する。

## FIX-7 audit

取得した各runの計画・repair・evidenceを `artifacts/runs/` に保存した。isolate-cause の不在成果物要求およびwrite step漏出について、今回の停止時点ではexecutor実行に到達せず、実戦観測は未行使。`fix_reproducer_suggested` と `host_env_normalized` のイベントは今回の保存イベント列から確認できなかったため、発火を主張しない。

## 合算

dfix-001〜003の合算分母は18（本レポートでは003の6本を記録）。003は全6 failed、full 0。既存計測の分布への追加はレビュー時に一次資料を再集計する。

## コスト

調達・実行・レポートの所要時間は `artifacts/timing/` および各console logに保存。今回の実行は再試行していない。

## 判定

P0-aのみPASS。P0-b/P0-c、P1-a/P1-bはF2/F3未到達およびイベント不在のため未成立。revertは実施していない。

---

## v2是正（追記）

v1の「イベント不在」主張は検索パス誤りであった。以下のコマンドで正しい階層を再監査した。

```sh
find workspace/management/runs/uat-test0717-dfix-003/artifacts \
  -path '*/.anvil/runs/*/events.jsonl' -print
rg --hidden -n 'intent_resolved|host_env_normalized|fix_reproducer_suggested|pipeline_error_extraction' \
  workspace/management/runs/uat-test0717-dfix-003/artifacts/runs/*/.anvil/runs/*/events.jsonl
```

### イベント発火表

| run | intent_resolved | host_env_normalized | fix_reproducer_suggested | pipeline_error_extraction |
|---|---|---|---|---|
| pipe qwen A | 1 (cli/fix) | 1 | 1 (pipeline_execution) | 1 (ValueError) |
| pipe gemma B | 1 (cli/fix) | 1 | 1 (pipeline_execution) | 1 (TypeError) |
| pipe qwen B | 1 (cli/fix) | 1 | 1 (pipeline_execution) | 1 (TypeError) |
| schema qwen A | 1 (cli/fix) | 1 | 1 (data_results_schema) | 1 (AssertionError) |
| schema gemma B | 1 (cli/fix) | 1 | 1 (data_results_schema) | 1 (no_traceback) |
| schema qwen A2 | 1 (cli/fix) | 1 | 1 (data_results_schema) | 1 (AssertionError) |

### Preflight・provenance・コスト

独立したpreflightログは保存されておらず、`preflight記録なし` とする。代替として各runの `run-start/end-utc.txt`、`baseline-source.sha256`、および上記FIX-6a以降のイベント署名を保存しているが、これらはバイナリ同一性の完全な事後証明ではなく、実行物の同一性を確認できる範囲に限られる。

出発点は pipe A/B と schema A/B の歴代成果物を採取し、各 `source-checks/*/reproducer.*` とSHAを `artifacts/source-checks`、`artifacts/source-records` に保存した。pipeは `python3 -B pipeline/main.py`、schemaは `python -m anvil_catalog_check data_results_schema output/results.json` をRとして事前失敗確認した。goalとCLIコマンド形は各runの `run-command.txt` に保存した。

調達時間は356秒、実行合計は3171秒（run合計2993秒）。レポート開始時刻は `artifacts/timing/` に記録した。

### 死因帰属

| run | 一次資料の停止形 | FIX-8帰属 |
|---|---|---|
| pipe qwen A | `duplicate expected path ownership: pipeline/main.py in fix-pipeline and run-pipeline`（planner_error/ultra_phase_failed/run_stop） | 1/6、該当 |
| pipe gemma B | verify step requires at least one verify command | 0/6 |
| pipe qwen B | isolate-cause: stream did not contain valid UTF-8 | 0/6 |
| schema qwen A | isolate-cause: path does not exist: output/inspection.json | 0/6 |
| schema gemma B | verify step requires at least one verify command | 0/6 |
| schema qwen A2 | verify step requires at least one verify command | 0/6 |

### #1〜#3合算・ゲート再判定

合算分母は18（dfix-001〜003）。本003の内訳は full 0 / partial 0 / static 0 / failed 6。P0-bは **PASS**：6件の `failed(after_not_executed)` はF1成立・F2/F3不在という契約状態と整合する。P0-cも **PASS**：full=0のため偽fullは存在しない。P1-aはイベント監査上、R提示6/6でPASS。P1-bはF3未到達のため未判定とする。
