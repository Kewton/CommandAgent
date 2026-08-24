# DATA-10 inspection 確定停滞・一次資料調査

- 調査日: 2026-07-15
- 対象 campaign: `uat-test0714-m4-004`
- 対象 Run: `data_agg_qwen27_plan_qwen35_exec_preset_profile_001`
- run id: `019f60f5-3299-7f21-9dac-fb760b5df37b`
- 比較 campaign: `uat-test0714-m4-001`
- 比較 Run 2: `data_agg_qwen27_plan_gemma31_exec_preset_profile_001`
- 比較 run id: `019f5f23-a347-7141-bab3-8515842684c2`

この文書は原文の確保と機械的な項目対応だけを記録する。原因解釈と修正案は含めない。

## 回収元と退避範囲

Run 1 回収元:

```text
/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0714_m4_004/data_agg_qwen27_plan_qwen35_exec_preset_profile_001
```

比較 Run 2 回収元:

```text
/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0714_m4_001/data_agg_qwen27_plan_gemma31_exec_preset_profile_001
```

退避先は `artifacts/run1/` と `artifacts/run2-m4001/`。指定された repair、recovery plan、`inspection.json`、実装ファイル、event 断片に加え、原文の参照元である phase plan、read-only recovery 文書、retained evidence を退避した。Run 2 は campaign の `uat-report.md` と `aggregate.json` の双方で ordinal `2` と記録されている。

`inspection-fragments.jsonl` は元の JSONL を再構成せず、そのまま連続抽出した。

| 退避ファイル | 元の範囲 | 行数 |
| --- | --- | ---: |
| `artifacts/run1/inspection-fragments.jsonl` | Run 1 `events.jsonl` 6–105行目 | 100 |
| `artifacts/run2-m4001/inspection-fragments.jsonl` | Run 2 `events.jsonl` 6–78行目 | 73 |

指定ファイルと退避コピーは `cmp` で一致した。両 event 断片と全 JSON は `jq -e` を通過した。Run 1 の回収元には `uat-console.log` または `*console*.log` は存在しない。比較 Run 2 の `uat-console.log` は退避した。

## Run 1: inspection step の原文

phase plan の `Phase task` 原文:

```text
Inspect the workspace CSV or TSV input for: data/sales.csv を読み込み、月次×地域の売上集計と全体合計を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。. Write output/inspection.json with column names, input row count, type summaries, distinct observed values for every categorical column (for example the actual region values), and sample rows. Derive later validation rules such as valid regions and date formats only from output/inspection.json observations; never invent an unobserved value set. Verify this phase without depending on pipeline/main.py, output/results.json, or output/report.md.
```

phase plan に保存された全 step 定義の原文:

```yaml
steps:
  - id: "inspect-workspace"
    kind: "inspect"
    expected_result: "pass"
    instruction: "Read data/sales.csv to assess structure and content."
    expected_paths:
    verify:
  - id: "create-inspection-script"
    kind: "verify"
    expected_result: "pass"
    instruction: "Verify the profile-owned data_manifest_artifact contract by running every declared check and report any exact failure."
    expected_paths:
      - "pipeline/main.py"
      - "output/inspection.json"
    verify:
      - "anvil-catalog-check:pipeline_probe"
      - "test -f output/inspection.json"
  - id: "run-inspection"
    kind: "verify"
    expected_result: "pass"
    instruction: "Verify the profile-owned data_manifest_artifact contract by running every declared check and report any exact failure."
    expected_paths:
      - "output/inspection.json"
      - "pipeline/main.py"
      - "output/results.json"
      - "output/report.md"
    verify:
      - "anvil-catalog-check:pipeline_probe"
      - "test -f output/inspection.json"
      - "anvil-catalog-check:data_results_schema"
      - "anvil-catalog-check:data_reconciliation"
      - "anvil-catalog-check:data_claims_binding"
  - id: "verify-inspection-output"
    kind: "verify"
    expected_result: "pass"
    instruction: "Verify the profile-owned data_manifest_artifact contract by running every declared check and report any exact failure."
    expected_paths:
      - "output/inspection.json"
    verify:
      - "test -f output/inspection.json"
```

`events.jsonl` では step 定義全体を持つイベントは存在しない。保存されている対応イベントは次のとおり。

- `planner_raw_output_shape` は planner 出力の先頭500文字だけを `preview` に保持する。
- `preset_step_converted` は `step_id` と `ownership=data_manifest_artifact` を保持する。
- `step_prompt_contract` は `step_id`、`step_kind`、`has_verify_commands` などの有無を保持する。
- `step_obligation_scope` は `effective_required_paths`、`explicit_required_paths`、`initially_missing_paths` を保持する。
- instruction と verify command の全文は退避した phase plan に存在する。

## Run 1: verify 実行・失敗出力の原文

### StepPlan verify の実行記録

失敗した `run-inspection` の終了イベント原文:

```json
{"event":"loop_stop","last_blocking_reason":"model_stagnation:read_only_loop","last_provider_error":null,"phase_scope":"data-inspection","read_only_streak":0,"reason":"model_stagnation:read_only_loop","recovery_prompt_path":".anvil/repairs/repair-read-only-stagnation-019f60fa-a0d4-7620-abda-d7afbe19c613.md","recovery_ultra_plan_path":".anvil/plans/recovery-ultra-plan-read-only-stagnation-019f60fa-a0d4-7620-abda-d7ba6ef938a0.yaml","recovery_yaml_missing":false,"schema_version":"1","selected_targets":["output/inspection.json","pipeline/main.py","output/results.json","output/report.md"],"selection_reason":"required_path","session_scope":"plan-run-step","step_kind":"verify","tool_calls":8,"verify_attempts":0,"write_required_no_write_attempts":2,"write_required_no_write_limit":2,"write_required_target_path":"output/inspection.json"}
```

このイベントの `verify_attempts` は `0`。したがって、phase plan に宣言された次の5 command について、StepPlan verify の command 実行イベント、stderr、command exit code は存在しない。

```text
anvil-catalog-check:pipeline_probe
test -f output/inspection.json
anvil-catalog-check:data_results_schema
anvil-catalog-check:data_reconciliation
anvil-catalog-check:data_claims_binding
```

Run 1 の `events.jsonl` 全109行には `stderr`、`stdout`、`exit_code`、`original_command` の各 key が存在しない。`tool_execute` は tool 名と `status` だけを保持する。

### verify 文脈で拒否された Bash

`create-inspection-script` 実行中、`step_kind=verify` で保存された拒否イベントの原文:

```json
{"bash_policy_purpose":"deterministic_verifier_evidence","blocked":true,"command_summary":"ls -la output/ 2>/dev/null || echo \"output directory does not exist\"","deterministic_verifier_evidence":false,"event":"runtime_bash_policy","normalization_kind":"","normalized_command_summary":"","policy_error_kind":"verify_command_policy_error","reason":"verify command may not create or write files with shell redirects; create files with the Write tool; keep verify to one deterministic command. For python-cli behavior probes, fixture CSVs already exist when required; python-cli behavior-probe fixture CSVs already exist; verify should run the deterministic python command against those fixtures.","schema_version":"1","step_kind":"verify","tool_name":"Bash","verifier_policy_checked":true,"verifier_policy_ok":false,"verify_command_violation_kind":"shell_control_syntax"}
```

続く2イベントの原文:

```json
{"bash_policy_purpose":"deterministic_verifier_evidence","deterministic_verifier_evidence":false,"event":"tool_policy_error","name":"Bash","policy_error_kind":"verify_command_policy_error","repeat_count":1,"schema_version":"1","step_kind":"verify","verify_command_violation_kind":"shell_control_syntax"}
{"error_kind":"verify_command_policy_error","event":"tool_validation_error","missing_arg":null,"name":"Bash","repeat_count":1,"schema_version":"1"}
```

B-2d telemetry 名で指定された key の保存状況は次のとおり。

| 指定項目 | Run 1 の保存値 |
| --- | --- |
| `original_command` | key 自体が存在しない |
| command 本文に相当する保存 key | `command_summary` |
| `violation_kind` | key 自体は存在しない |
| violation の保存 key/value | `verify_command_violation_kind="shell_control_syntax"` |
| policy 判定 | `policy_error_kind="verify_command_policy_error"` |
| stderr | key 自体が存在しない |
| exit code | key 自体が存在しない |

### phase 終端理由

`ultra_phase_failed.reason` の保存原文。文字列は一次資料上この位置で終わる。

```text
model_stagnation:read_only_loop: write_required exhausted for output/inspection.json; objective: Execute exactly one StepPlan step. Overall goal: data/sales.csv を読み込み、月次×地域の売上集計と全体合計を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。 Current step id: run-inspection Current step kind: verify Current step instruction: Verify the profile-owned data_manifest_artifact contract by running every declared check and report any exact failure. Required final artifacts: - pipeline/main.py - output/inspection.json 
```

Run 1 の回収元に commandagent の process exit code を記録した console log は存在しない。terminal event は `ok=false`、`status="failed"`、`task_status="failed"` を保持する。

### retained evidence の値

回収元には StepPlan event とは別に次の evidence が存在した。`loop_stop.verify_attempts=0` のため、これらを StepPlan verify 実行回数へ読み替えない。

| capability | status | ok | exit/stderr または failure |
| --- | --- | --- | --- |
| `pipeline_probe` | `pass` | `true` | `exit_code=0`; `stderr.text=""` |
| `data_results_schema` | `pass` | `true` | failure なし |
| `data_reconciliation` | `pass` | `true` | `failure_kinds=[]` |
| `data_claims_binding` | `failed` | `false` | `failure_kinds` 全文は `artifacts/run1/evidence/claims-binding.json` に退避 |

`pipeline-run.json` に保存された command 原文:

```json
[
  "python3",
  "-B",
  "pipeline/main.py"
]
```

同 evidence の stderr と exit code 原文:

```json
"exit_code": 0,
"stderr": {
  "text": "",
  "captured_bytes": 0,
  "total_bytes": 0,
  "truncated": false
}
```

## Run 1: 現存する inspection.json 全文

```json
{
  "total_input_rows": 60,
  "valid_rows": 57,
  "excluded": [
    {
      "reason": "invalid_date",
      "rows": 1
    },
    {
      "reason": "missing_date",
      "rows": 1
    },
    {
      "reason": "missing_amount",
      "rows": 1
    }
  ],
  "regions": [
    "名古屋",
    "大阪",
    "東京"
  ],
  "months": [
    [
      2026,
      1
    ],
    [
      2026,
      2
    ],
    [
      2026,
      3
    ],
    [
      2026,
      4
    ],
    [
      2026,
      5
    ],
    [
      2026,
      6
    ]
  ]
}
```

現存ファイルの top-level 構造:

| key | JSON type | value/要素数 |
| --- | --- | ---: |
| `total_input_rows` | number | `60` |
| `valid_rows` | number | `57` |
| `excluded` | array | `3` |
| `regions` | array | `3` |
| `months` | array | `6` |

### 現存ファイルの版に関する一次資料

- `events.jsonl` の Write event は `output/inspection.json` への本文を `preview="<omitted>"`、`string_len=851` と記録する。
- 現存する回収元 `output/inspection.json` は533 bytes。
- `pipeline-run.json` は `pipeline/main.py` 実行後の `output/inspection.json` を `bytes=533`、`fnv1a64="137ca5d1c0ccabd5"` と記録する。
- 回収元に、Write event の省略された851文字本文を保持する別ファイルは存在しない。
- 本調査で退避した `inspection.json` は現存する533-byte file と byte-for-byte 同一。

## Run 1: 要求と inspection.json の機械的差分

### 宣言 verify command と JSON の対応

| 宣言 verify | inspection.json に対する明示的要求 | 保存事実 |
| --- | --- | --- |
| `anvil-catalog-check:pipeline_probe` | inspection key 名の指定なし | retained evidence は `pass`, `exit_code=0` |
| `test -f output/inspection.json` | file の存在 | file は存在。先行 step の `loop_stop.reason` は `required_artifacts_satisfied_after_tool` |
| `anvil-catalog-check:data_results_schema` | `output/results.json` の schema | retained evidence は `pass` |
| `anvil-catalog-check:data_reconciliation` | results reconciliation | retained evidence は `pass` |
| `anvil-catalog-check:data_claims_binding` | results/report claims binding | retained evidence は `failed` |

phase plan の宣言 verify 一覧には、`output/inspection.json` の content key または content schema を指定する command はない。

### Phase task の記載項目と JSON の対応

| Phase task の記載 | 現存 JSON の同名 key/構造 | 現存 JSON にある値 |
| --- | --- | --- |
| `column names` | `column_names` key なし | column 名 `date` / `region` / `amount` の配列なし |
| `input row count` | `input_row_count` key なし | `total_input_rows: 60` は存在 |
| `type summaries` | `type_summaries` key なし | column ごとの type 値なし |
| `distinct observed values for every categorical column` | `distinct_values` object なし | `regions: ["名古屋","大阪","東京"]` と `months: [[2026,1],...,[2026,6]]` は存在。元 column 名ごとの object ではない |
| `sample rows` | `sample_rows` key なし | row object の array なし |

値単位の保存状況:

- 地域値は3件ある。
- 生の date 文字列一覧はない。
- amount の observed value 一覧はない。
- sample row の `date`、`region`、`amount` の組はない。
- `months` の各要素は2個の number からなる array。
- `excluded` の各要素は `reason` string と `rows` number からなる object。

## Run 1: repair 文言の原文

`repair-phase-data-inspection-019f60fa-a0d7-77f3-be47-c87d91e53ada.md` 全文:

```text
Recover this failed run by producing and executing a focused ultra plan.

Original goal:
data/sales.csv を読み込み、月次×地域の売上集計と全体合計を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。

Profile: data

Failure scope:
- phase: data-inspection
- step: unknown
- kind: phase_execute_error

Failure evidence:
- model_stagnation:read_only_loop: write_required exhausted for output/inspection.json; objective: Execute exactly one StepPlan step. Overall goal: data/sales.csv を読み込み、月次×地域の売上集計と全体合計を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。 Current step id: run-inspection Current step kind: verify Current step instruction: Verify the profile-owned data_manifest_artifact contract by running every declared check and report any exact failure. Required final artifacts: - pipeline/main.py - output/inspection.json 

Missing paths:
- none

Missing capabilities:
- none

Verification commands:
- none

Changed paths:
- none

Repair targets:
- none

Required recovery action:
- Inspect the current workspace state first.
- Preserve already useful artifacts.
- Create or repair the missing implementation artifacts.
- Use deterministic verification.
- Do not treat scaffold-only or build-only output as complete.
```

`repair-read-only-stagnation-019f60fa-a0d4-7620-abda-d7afbe19c613.md` 全文:

```text
Recover this failed run by producing and executing a focused ultra plan.

Original goal:
Execute exactly one StepPlan step. Overall goal: data/sales.csv を読み込み、月次×地域の売上集計と全体合計を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。 Current step id: run-inspection Current step kind: verify Current step instruction: Verify the profile-owned data_manifest_artifact contract by running every declared check and report any exact failure. Required final artifacts: - pipeline/main.py - output/inspection.json - output/results.json - output/report.md Required final capabilities: - data_reconciliation - dat

Profile: data

Failure scope:
- phase: data-inspection
- step: verify
- kind: model_stagnation:read_only_loop

Failure evidence:
- read_only_stagnation: write_required reached after read_only_streak=0
- write_required exhausted without Write/Edit to output/inspection.json: attempts=2/2
- write_required selected_targets=output/inspection.json,pipeline/main.py,output/results.json,output/report.md; selection_reason=required_path

Missing paths:
- none

Missing capabilities:
- none

Verification commands:
- none

Changed paths:
- none

Repair targets:
- output/inspection.json
- pipeline/main.py
- output/results.json
- output/report.md

Required recovery action:
- Inspect the current workspace state first.
- Preserve already useful artifacts.
- Create or repair the missing implementation artifacts.
- Use deterministic verification.
- Do not treat scaffold-only or build-only output as complete.
```

各 step の prompt 本文は event 上 `prompt_body_saved=false`。上記2 repair file 以外の完全な repair prompt 本文は退避元に存在しない。

## Run 1: write_required 前後の時系列（10行以内）

| # | 元 event 行 | モデル応答／tool 結果 |
| ---: | --- | --- |
| 1 | 71–73 | `Write output/inspection.json` が `status=ok`。`required_artifacts_satisfied_after_tool` で先行 step 終了。部分書き込みは `write_required` 発火前。 |
| 2 | 77–82 | `Read pipeline/main.py`、`Read output/inspection.json`、`Read data/sales.csv` を実行し、3件とも `status=ok`。 |
| 3 | 84 | tool call のない応答に対して `empty_response_escalation`, `stage=nudge_1`。 |
| 4 | 86–90 | `Read pipeline/main.py` と `Read data/sales.csv`。`empty_response_recovered`, `after_empty_responses=1`。両 Read は `status=ok`。 |
| 5 | 92–93 | `Read data/sales.csv` を再実行し `status=ok`。 |
| 6 | 94 | `read_only_stagnation_feedback`, `stage=write_required`, `target_path=output/inspection.json`, `selection_reason=required_path`。selected targets は4件。 |
| 7 | 96–97 | モデル応答は `Read data/sales.csv`。`read_only_tool_rejected`, `attempts=1/2`。 |
| 8 | 99–100 | モデル応答は `Read pipeline/main.py`。`read_only_tool_rejected`, `attempts=2/2`。 |
| 9 | 101–104 | recovery 保存後、`verify_attempts=0` で loop stop。phase は `write_required exhausted for output/inspection.json` で failed。`write_required` 後の Write/Edit は0件。 |

## 比較 Run 2（m4_001）

### instruction と verify command の原文

失敗 step `generate-inspection-report` の原文:

```yaml
  - id: "generate-inspection-report"
    kind: "verify"
    expected_result: "pass"
    instruction: "Verify the profile-owned data_manifest_artifact contract by running every declared check and report any exact failure."
    expected_paths:
      - "output/inspection.json"
      - "pipeline/main.py"
      - "output/results.json"
      - "output/report.md"
    verify:
      - "anvil-catalog-check:pipeline_probe"
      - "test -f output/inspection.json"
      - "anvil-catalog-check:data_results_schema"
      - "anvil-catalog-check:data_reconciliation"
      - "anvil-catalog-check:data_claims_binding"
```

inspection 生成 step の instruction 原文:

```text
Create scripts/inspect_data.py that reads data/sales.csv, computes column names, input row count, type summaries, distinct observed values for every categorical column, and sample rows, then writes the results to output/inspection.json.
```

### verify 実行と出力の保存状況

`generate-inspection-report` 内で保存された Bash command と結果:

| 元 event 行 | command 原文 (`command_summary`) | 結果 |
| ---: | --- | --- |
| 56–57 | `python3 scripts/inspect_data.py` | `tool_execute.status="ok"` |
| 60–61 | `python3 scripts/inspect_data.py && test -f output/inspection.json && echo "SUCCESS"` | `tool_execute.status="ok"`; `normalization_kind="shell_control_split"` |
| 64–65 | `python3 scripts/inspect_data.py` | `tool_execute.status="ok"` |
| 69–70 | `python3 scripts/inspect_data.py` | `read_only_tool_rejected`, attempts `1/2` |
| 72–73 | `python3 scripts/inspect_data.py` | `read_only_tool_rejected`, attempts `2/2` |

Run 2 の event にも command stdout、stderr、command exit code、`original_command` key は存在しない。command は `command_summary` に保存されている。`uat-console.log` は実行済み3 command を `✓ Bash ok` と記録し、campaign harness の最終行を `exit_code=1` と記録する。

Run 2 の loop stop 原文:

```json
{"event":"loop_stop","last_blocking_reason":"model_stagnation:read_only_loop","last_provider_error":null,"phase_scope":"data-inspection","read_only_streak":0,"reason":"model_stagnation:read_only_loop","recovery_prompt_path":".anvil/repairs/repair-read-only-stagnation-019f5f25-cccd-7e91-a8cc-80e4d646e7d5.md","recovery_ultra_plan_path":".anvil/plans/recovery-ultra-plan-read-only-stagnation-019f5f25-cccd-7e91-a8cc-80fba38d17b6.yaml","recovery_yaml_missing":false,"schema_version":"1","selected_targets":["output/inspection.json","pipeline/main.py","output/results.json","output/report.md"],"selection_reason":"required_path","session_scope":"plan-run-step","step_kind":"verify","tool_calls":5,"verify_attempts":0,"write_required_no_write_attempts":2,"write_required_no_write_limit":2,"write_required_target_path":"output/inspection.json"}
```

Run 2 の `ultra_phase_failed.reason` 原文。文字列は一次資料上この位置で終わる。

```text
model_stagnation:read_only_loop: write_required exhausted for output/inspection.json; objective: Execute exactly one StepPlan step. Overall goal: data/sales.csv を読み込み、月次×地域の売上集計と全体合計を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。 Current step id: generate-inspection-report Current step kind: verify Current step instruction: Verify the profile-owned data_manifest_artifact contract by running every declared check and report any exact failure. Required final artifacts: - pipeline/main.py - Paths: - re
```

retained evidence の失敗出力原文:

```text
data_results_schema.error = failed to read /Users/maenokota/share/work/localwork/commandagent_mvp/01/test0714_m4_001/data_agg_qwen27_plan_gemma31_exec_preset_profile_001/output/results.json: No such file or directory (os error 2)
data_reconciliation.failure_kinds[0] = reconciliation_violation:invalid_results_schema:failed to read /Users/maenokota/share/work/localwork/commandagent_mvp/01/test0714_m4_001/data_agg_qwen27_plan_gemma31_exec_preset_profile_001/output/results.json: No such file or directory (os error 2)
data_claims_binding.failure_kinds[0] = claims_binding_violation:invalid_results_schema:failed to read /Users/maenokota/share/work/localwork/commandagent_mvp/01/test0714_m4_001/data_agg_qwen27_plan_gemma31_exec_preset_profile_001/output/results.json: No such file or directory (os error 2)
```

これら3 evidence は回収元に存在するが、StepPlan event の `verify_attempts` は `0`。

### 生成された inspection 成果物

`output/inspection.json` は存在し、全文を `artifacts/run2-m4001/inspection.json` に退避した。top-level 構造は次のとおり。

| key | JSON type | value/要素数 |
| --- | --- | ---: |
| `column_names` | array | `3` |
| `input_row_count` | number | `60` |
| `type_summaries` | object | `3` |
| `distinct_values` | object | `2` |
| `sample_rows` | array | `5` |

`column_names` は `date`、`region`、`amount`。`type_summaries` は3 column 分、`distinct_values` は `date` と `region` の2 key、`sample_rows` は5 row object を持つ。

## 2例の異同

| 比較項目 | m4_004 Run 1 | m4_001 Run 2 | 同一性 |
| --- | --- | --- | --- |
| phase | `data-inspection` | `data-inspection` | 同一 |
| executor | `qwen3.6:35b-a3b-coding-nvfp4` | `gemma4:31b-cloud` | 異なる |
| 失敗 step id | `run-inspection` | `generate-inspection-report` | 異なる |
| step instruction | `Verify the profile-owned...` | 同じ文字列 | 同一 |
| failing step の verify 5件 | pipeline probe / file existence / results schema / reconciliation / claims binding | 同じ5件、同じ順序 | 同一 |
| failing step 開始時の `initially_missing_paths` | `[]` | `pipeline/main.py`, `output/results.json`, `output/report.md` | 異なる |
| `test -f output/inspection.json` に対応する file | 存在 | 存在 | 同一 |
| `verify_attempts` | `0` | `0` | 同一 |
| write-required target | `output/inspection.json` | `output/inspection.json` | 同一 |
| selected targets / reason | 同じ4 path / `required_path` | 同じ4 path / `required_path` | 同一 |
| write-required 拒否回数 | `2/2` | `2/2` | 同一 |
| 拒否された tool | `Read` 2回 | `Bash` 2回 | 異なる |
| empty-response event | `nudge_1` 1回、recovered 1回 | なし | 異なる |
| `original_command` key | なし | なし | 同一 |
| `verify_command_violation_kind` | `shell_control_syntax` 1件（先行 step） | 該当 event なし | 異なる |
| inspection の phase-task 5項目 | 同名 top-level key 5件すべてなし | 5件すべてあり | 同一不足ではない |
| pipeline implementation | `pipeline/main.py` あり | `pipeline/main.py` なし | 異なる |
| terminal | `write_required exhausted for output/inspection.json` | 同じ terminal | 同一 |

宣言 verify command は2例で同一。現存 `inspection.json` の top-level 不足は同一ではない。Run 1 は `column_names`、`input_row_count`、`type_summaries`、`distinct_values`、`sample_rows` がなく、Run 2 は5 key すべてを持つ。

## 退避ファイル一覧

Run 1:

```text
artifacts/run1/evidence/claims-binding.json
artifacts/run1/evidence/pipeline-run.json
artifacts/run1/evidence/reconciliation.json
artifacts/run1/evidence/results-schema.json
artifacts/run1/inspection-fragments.jsonl
artifacts/run1/inspection.json
artifacts/run1/main.py
artifacts/run1/plan-019f60f7-883c-74d3-8f6f-ccb284069f06.yaml
artifacts/run1/recovery-ultra-plan-phase-data-inspection-019f60fa-a0d7-77f3-be47-c8882ab0b6ee.yaml
artifacts/run1/recovery-ultra-plan-read-only-stagnation-019f60fa-a0d4-7620-abda-d7ba6ef938a0.yaml
artifacts/run1/repair-phase-data-inspection-019f60fa-a0d7-77f3-be47-c87d91e53ada.md
artifacts/run1/repair-read-only-stagnation-019f60fa-a0d4-7620-abda-d7afbe19c613.md
```

比較 Run 2:

```text
artifacts/run2-m4001/evidence/claims-binding.json
artifacts/run2-m4001/evidence/reconciliation.json
artifacts/run2-m4001/evidence/results-schema.json
artifacts/run2-m4001/inspect_data.py
artifacts/run2-m4001/inspection-fragments.jsonl
artifacts/run2-m4001/inspection.json
artifacts/run2-m4001/plan-019f5f25-969f-7892-9a39-83d4157f2bd3.yaml
artifacts/run2-m4001/recovery-ultra-plan-phase-data-inspection-019f5f25-cccf-77f2-81b4-21f26acfdb89.yaml
artifacts/run2-m4001/recovery-ultra-plan-read-only-stagnation-019f5f25-cccd-7e91-a8cc-80fba38d17b6.yaml
artifacts/run2-m4001/repair-phase-data-inspection-019f5f25-ccce-7f41-b284-1dcc87de8270.md
artifacts/run2-m4001/repair-read-only-stagnation-019f5f25-cccd-7e91-a8cc-80e4d646e7d5.md
artifacts/run2-m4001/uat-console.log
```
