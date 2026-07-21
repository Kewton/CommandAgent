# bench report skeleton: dfix-synthesis-20260721-074830

This skeleton transfers mechanical observations only. A human reviewer must decide UAT pass/fail, failure class, retry consumption, and settlement.

## Preflight record

- HEAD: `4127673704a00b3a8d2ffef897deb4aa463cd704`
- minimum ancestor: `56d247f`
- NODE_ENV: `production`
- deviations: `0`

## Event search method

The harness recursively parses JSON lines from each run artifact using file glob `.anvil/runs/**/events.jsonl`, reads the exact `event` field, and applies these regular-expression patterns:

- `intent_resolved`: `^intent_resolved$`
- `host_env_normalized`: `^host_env_normalized$`
- `fix_reproducer_suggested`: `^fix_reproducer_suggested$`
- `*_plan_synthesized`: `^[a-z0-9_]+_plan_synthesized$`
- `*_adjudicated`: `^[a-z0-9_]+_adjudicated$`

## Run matrix (mechanical transfer)

| run | harness status | product exit | seconds | verdict transfer | assurance transfer |
|---|---|---:|---:|---|---|
| pipe_qwen35_001 | completed | 1 | 128 | failed | failed (after_not_executed) |
| pipe_gemma31_001 | completed | 1 | 143 | failed | failed (after_not_executed) |
| pipe_qwen35_002 | completed | 1 | 18 | failed | failed (after_not_executed) |
| schema_qwen35_001 | completed | 1 | 13 | failed | failed (after_not_executed) |
| schema_gemma31_001 | interrupted(environment) | — | 15 | interrupted | static (data_profile_probe_not_run) |
| schema_qwen35_002 | completed | 1 | 112 | failed | failed (after_not_executed) |

## Event firing counts

| run | intent_resolved | host_env_normalized | fix_reproducer_suggested | *_plan_synthesized | *_adjudicated |
|---|---:|---:|---:|---:|---:|
| pipe_qwen35_001 | 1 | 1 | 1 | 1 | 0 |
| pipe_gemma31_001 | 1 | 1 | 1 | 1 | 0 |
| pipe_qwen35_002 | 1 | 1 | 1 | 1 | 0 |
| schema_qwen35_001 | 1 | 1 | 1 | 1 | 0 |
| schema_gemma31_001 | 1 | 1 | 1 | 1 | 0 |
| schema_qwen35_002 | 1 | 1 | 1 | 1 | 0 |

## Terminal reasons (verbatim transfer)

### pipe_qwen35_001

````text
phase repair failed: model_stagnation:read_only_loop: write_required exhausted for pipeline/main.py; objective: Execute exactly one StepPlan step. Overall goal: data/sales.csv を処理する pipeline/main.py の実行がエラーで失敗します。原因を特定して修正してください。修正後もデータ契約の既存検証が通ることを確認してください。 Current step id: implement-fix Current step kind: implement Current step instruction: Repair the F1-diagnosed defect in `pipeline/main.py` using the isolated cause and the shared target resolver (traceback_mapped); preserve the existing data
````

### pipe_gemma31_001

````text
phase repair failed: model_stagnation:read_only_loop: write_required exhausted for pipeline/main.py; objective: Execute exactly one StepPlan step. Overall goal: data/sales.csv を処理する pipeline/main.py の実行がエラーで失敗します。原因を特定して修正してください。修正後もデータ契約の既存検証が通ることを確認してください。 Current step id: implement-fix Current step kind: implement Current step instruction: Repair the F1-diagnosed defect in `pipeline/main.py` using the isolated cause and the shared target resolver (traceback_mapped); preserve the existing data
````

### pipe_qwen35_002

````text
phase repair failed: model_stagnation:read_only_loop: write_required exhausted for pipeline/main.py; objective: Execute exactly one StepPlan step. Overall goal: data/sales.csv を処理する pipeline/main.py の実行がエラーで失敗します。原因を特定して修正してください。修正後もデータ契約の既存検証が通ることを確認してください。 Current step id: implement-fix Current step kind: implement Current step instruction: Repair the F1-diagnosed defect in `pipeline/main.py` using the isolated cause and the shared target resolver (traceback_mapped); preserve the existing data
````

### schema_qwen35_001

````text
phase repair failed: recoverable tool error repeated: missing_arg; incomplete; Recovery artifact check: prompt_parse_ok=true, yaml_parse_ok=true, command_targets_valid=true
````

### schema_gemma31_001

````text
interrupted by user
````

### schema_qwen35_002

````text
phase repair failed: model_stagnation:read_only_loop: write_required exhausted for pipeline/main.py; objective: Repair step `implement-fix`. Verification failed: data_results_schema:results.json missing required key `reconciliation`. Repair target: implementation. Fix the implementation files that should satisfy the requested behavior. Make the smallest bounded change, then stop. Overall goal: output/results.json がデータ契約のスキーマ検証に失敗します。パイプラインを修正して正しい results.json を再生成し、既存検証が通ることを確認してください。 Repair
````

## Interrupted runs requiring review

The following runs were not rerun: schema_gemma31_001.
A one-time rerun must use a new directory and requires review adjudication.

## Human review fields

- UAT pass/fail: 
- Failure class / attribution: 
- Retry-consumption decision: 
- Settlement comment: 

## Step 1 adjudication transfer

`schema_gemma31_001` の `interrupted(environment)` はrun非消費として扱い、
既裁定例外に従い新規ディレクトリ・新規コピーで1回だけ再実行した。再調達の
SHA/precheck、完全一致コマンド、前後epoch、実行中断記録は
`rerun-schema_gemma31_001-20260721-080000/` と `rerun-artifacts/` に保存した。
再実行もenvironment中断となり、追加再試行はしない。

## Six-run distribution and comparison

| run | verdict | assurance / stop class |
|---|---|---|
| pipe_qwen35_001 | failed | failed (after_not_executed) / read-only loop stagnation |
| pipe_gemma31_001 | failed | failed (after_not_executed) / read-only loop stagnation |
| pipe_qwen35_002 | failed | failed (after_not_executed) / read-only loop stagnation |
| schema_qwen35_001 | failed | failed (after_not_executed) / missing_arg recovery |
| schema_gemma31_001 | interrupted | static / environment interruption |
| schema_qwen35_002 | failed | failed (after_not_executed) / results schema contract |

dfix-005 v2との対比では、verdictの機械的分布はfailed 5・interrupted 1で、
pipe側read-only stagnationとschema側missing_arg/契約不備が観測された。機械
クラスの再発有無と停滞位置の確定裁定はレビュー側が行う。ハーネスは判定・分類・
清算を行わない。

## v0.2 improvement candidates (confirmed)

- `--report-root`を分離し、既定をリポジトリ内`runs/`として監査資産が自動でGit追跡に乗る形にする
- interrupted runの例外再実行をbench内でサポートし、両記録を自動保存する
- scrub結果をrun matrixへ表示する
- preflight自己出力許容件数と対象prefixをskeletonへ転記する
- resume中断runの原文と最終状態を一覧化する

## Human observations (dfix-006)

ハーネス検収: dfix-005 v2と突合し、preflightはclean判定、ancestor、cargo test、release build/install/version、NODE_ENVを実施し、自己出力許容件数もmetaに記録した。4 source setのSHA-256とprecheck結果、suiteから構築したwrapperなし6コマンド、各run直後の`.anvil`・copy対象・console退避、events.jsonlの再帰検索パターンは期待どおり転記された。退避後scrubは全件ok（findings 0）。今回の不足は、scrub結果のrun matrix列表示と、preflight自己出力許容の実際の混在ケースをskeletonへ要約する欄がない点である。

分布所見: product exitは5件が1、1件は`interrupted(environment)`。verdict転記はfailed 4件、interrupted 1件、failed 1件で、停止位置はpipe系のread-only loop、schema系のmissing_argまたはresults.json契約不備だった。dfix-005 v2との差分の合否・機械クラス再発判定・停滞帰属はレビュー側が確定する。ハーネスは判定・分類・清算を行わない。

v0.2改善候補:

- scrub findings/okをrun matrixへ機械表示する
- preflight自己出力許容件数と対象prefixをskeletonへ転記する
- resumeによる中断runの最終状態を、実行中断の原文とともに一覧化する
