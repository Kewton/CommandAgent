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
