# uat-test0718-inv-001: investigation intent 初計測

実施日: 2026-07-18 (JST)  
契約: `docs/investigation-intent-contract.md` (fixed, `87d432a`)  
実装基準: `1f50e4f Cover investigation intent conformance`  
計測workspace: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0718_inv_001`

## 結論

6/6は製品が理由付きで正直終端し、環境中断・panic・再試行は0だった。I1は6/6で成立し、`stage=diagnosis`, `expected=failure`, `executed=true`, `epoch=1` と実失敗が全runの `evidence/investigation-run.json` に記録された。`investigation_plan_synthesized`、3段合成計画、`plan_preset_resolved.origin=default_investigate_data` も6/6で確認した。

fullは0/6。4本はdiagnose段で停止しI2未実行、2本はdiagnosis.mdを生成してI2まで実行したが、全抽出主張が不一致となり `diagnosis_unbound` でfailedへ落ちた。虚偽診断をpartial/fullへ昇格させたrunは0であり、照合器は実戦で拒否を行った。

ただし、全runの汎用summaryが契約固有evidenceとは別に `Assurance: static (data_profile_probe_not_run)` を表示した。I1実行済みの実態、ならびに2本の `investigation_adjudicated.assurance_level=failed` と矛盾する。契約§4のassurance投影としては不適合なので、P0-bはfailと裁定する。これはrunの一次死因ではないが、新規の機械側表示/投影クラスである。

| 基準 | 結果 | 根拠 |
|---|---|---|
| P0-a 6/6正直終端 | PASS | 全run product exit 1、理由付きfailed、environment interruption 0 |
| P0-b 契約§4準拠 | **FAIL** | 全summaryが旧data理由のstaticを表示。I1実行済み、かつ2本の契約固有failed裁定と不整合 |
| P0-c 偽成功ゼロ | PASS | full/partial 0、I2違反2本はいずれもfailed |
| P1-a 合成6/6＋既定preset | PASS | synthesis event 6/6、3段plan 6/6、origin 6/6 |
| P1-b I1実行・失敗6/6 | PASS | investigation-run evidence 6/6 |

## 1. Preflight

全greenを確認してから製品runを開始した。

| 項目 | 結果 |
|---|---|
| `git status --porcelain` | 空 |
| `git log -1 --oneline` | `1f50e4f Cover investigation intent conformance` |
| `git merge-base --is-ancestor 1f50e4f HEAD` | exit 0 |
| 権限付き `cargo test --quiet` | exit 0。lib 1,442 passed / 15 ignored、後続integration suiteも全green |
| `cargo build --release` | exit 0 |
| install | `install -m 755 target/release/commandagent ~/.local/bin/commandagent` exit 0 |
| version | `commandagent 0.1.0 1f50e4f 2026-07-18T13:54:21Z`（`+dirty`なし） |
| binary SHA-256 | build/installとも `4fb852dd5a0845f2df1cd90c2e2146437ca3c373bb5fdb86c5b5c1d8e9dc7e1e` |
| host `NODE_ENV` | `production`。各runで `host_env_normalized` が発火 |

## 2. 出発点とprovenance

採取元は `workspace/management/runs/uat-test0717-dfix-002/artifacts/source-checks/`。各runには対応setの `pipeline/`, `output/`, `data/` のみを新規コピーした。採取元の `reproducer.*.log`, `catalog-helper-*.log`, `.git` の持込は0。runディレクトリ内の `preflight-r.*` はコピー後に本計測で新規生成した事前確認ログである。

| set | pipeline/main.py | output主要物 | data/sales.csv | R事前確認 |
|---|---|---|---|---|
| pipe-a | `4944322105b422f71338513405e3de57d2c698aab565f313fac794e4163ad1d3` | inspection `0e76d134335ffd290fb457321c2b0cd94412bd844f1f4732b9a089116aa5a6a7` | `2f6c04e42b0ebdff85a7eb6b52a342610155be6796bd89e5729075d87c78d873` | exit 1、`ValueError: invalid literal for int() with base 10: ''` |
| pipe-b | `b27e8aaffef74dac171ff19dffb89eb891c97ee2038045887dae8d43719a511b` | results `af452a62d26b5e377453bbd1daddc27282bd61c806aeff30d6616dd02afbf6f2`; inspection `6bd2ec134cbbc6fda43c8f256e941779849e63152cee8f36d4c4df2f6d1b4558` | 同上 | exit 1、`TypeError: list.append() takes exactly one argument (2 given)` |
| schema-a / schema-b | `5e2d7efe794c78a20dded6ca1b0b4e449293fe2609c8274c302b102ddb0c7c96` | results `a0e3a1dfd4a2378598efead1da29673d2d91e09c6fdebecb1f9b1db5e7dd07ca`; report `17af1dd02de623845341e4a427c0c7ec5b26254de6524030bc5842d295165f97` | 同上 | exit 1、`AssertionError` |

hashは `uat-test0717-dfix-002/analysis/source-provenance.json` の `pipeline_sha256`, `results_sha256`, `inspection_sha256`, `sales_csv_sha256` と一致する。したがって連鎖は「過去UAT原資料 → dfix-002 qualified source-check → inv-001独立fresh copy」であり、壊れた出発点の合成・補完は0である。

実行後にも全6runの `pipeline/main.py` と `data/sales.csv` を採取元に対して `cmp` し、6/6 byte一致を確認した。investigationによる修正は0である。

## 3. 実行コマンド

各runでexecutorとgoalのみを置換し、次の形を1回だけ起動した。`timeout`, `env`, `nice`, `nohup` その他wrapperは不使用。前後の `date +%s` と `date` は各 `uat-console.log` に保存した。

```text
commandagent --yes --intent investigate --context-budget 65536 \
  --model <executor> --provider ollama \
  --planner-model qwen3.6:27b-coding-nvfp4 --planner-provider ollama \
  --ultra-plan-run --profile data "<goal>"
```

## 4. Run行列

「契約assurance」は `investigation_adjudicated` が存在する場合はその値、未到達は `not_adjudicated` とした。「summary表示」は第三者が実出力の不整合を追えるよう別列にした。

| # | run | set / executor | verdict | 契約assurance | summary表示 | I1 | I2 | 失敗クラス（帰属） | 秒 |
|---:|---|---|---|---|---|---|---|---|---:|
| 1 | inv1_pipe_qwen35_001 | pipe-a / qwen35 | failed (honest) | not_adjudicated | static (`data_profile_probe_not_run`) | pass | 未実行 | diagnose `model_stagnation:read_only_loop`（モデル） | 29 |
| 2 | inv1_pipe_gemma31_001 | pipe-b / gemma31 | failed (honest) | failed (`diagnosis_unbound`) | static (`data_profile_probe_not_run`) | pass | fail: 5/5違反 | 虚偽ValueError・架空コードを引用（モデル）。照合拒否は機械の正常動作 | 59 |
| 3 | inv1_pipe_qwen35_002 | pipe-b / qwen35 | failed (honest) | not_adjudicated | static (`data_profile_probe_not_run`) | pass | 未実行 | diagnose `model_stagnation:read_only_loop`（モデル） | 31 |
| 4 | inv1_schema_qwen35_001 | schema-a / qwen35 | failed (honest) | not_adjudicated | static (`data_profile_probe_not_run`) | pass | 未実行 | executorが不在 `output/inspection.json` をRead（モデル） | 4 |
| 5 | inv1_schema_gemma31_001 | schema-b / gemma31 | failed (honest) | not_adjudicated | static (`data_profile_probe_not_run`) | pass | 未実行 | diagnose `model_stagnation:read_only_loop`（モデル） | 276 |
| 6 | inv1_schema_qwen35_002 | schema-a / qwen35 | failed (honest) | failed (`diagnosis_unbound`) | static (`data_profile_probe_not_run`) | pass | fail: 5/5違反 | 契約形式に束縛されないcode blockを5件提出（モデル）。照合拒否は機械の正常動作 | 38 |

計時epochは順に `1784383028→1784383057`, `1784383104→1784383163`, `1784383200→1784383231`, `1784383261→1784383265`, `1784383316→1784383592`, `1784383630→1784383668`。全て `product_exit=1`。環境中断0、再実行0。

一次死因分布はモデル6/6、機械0/6、複合0/6。ただし一次死因と独立した機械側のassurance投影不整合が6/6にある。族別fullはpipe 0/3、schema 0/3。executor別fullはqwen35 0/4、gemma31 0/2。

## 5. 合成計画・preset監査

全6runの `investigation_plan_synthesized` は `profile=data`, `phase_count=3`。pipe族の `r_basis` は `goal_failure_kind:pipeline_execution`、schema族は `goal_profile_contract:data_results_schema`。全runのcanonical `ultra-plan-*.yaml` に次の3段がこの順で実在する。

1. `reproduce-candidate`
2. `diagnose`
3. `bind-verify`

全runの `plan_preset_resolved` は `plan_preset=profile`, `origin=default_investigate_data`, `source=default_investigate_data`。CLIで `--plan-preset` を指定しない実戦経路の既定解決を6/6で確認した。

計画実物の検索コマンド:

```text
rg -n 'id: "(reproduce-candidate|diagnose|bind-verify)"' \
  workspace/management/runs/uat-test0718-inv-001/artifacts/*/.anvil/plans/ultra-plan-*.yaml
```

結果は18 hit（3 phase × 6 run）。

## 6. I1 reproducer_fails監査

| run | R原文 | stage / expected / epoch | 実結果 |
|---|---|---|---|
| inv1_pipe_qwen35_001 | `python3 -B pipeline/main.py` | diagnosis / failure / 1 | executed=true, failure: `ValueError: invalid literal for int() with base 10: ''` |
| inv1_pipe_gemma31_001 | `python3 -B pipeline/main.py` | diagnosis / failure / 1 | executed=true, failure: `TypeError: list.append() takes exactly one argument (2 given)` |
| inv1_pipe_qwen35_002 | `python3 -B pipeline/main.py` | diagnosis / failure / 1 | executed=true, failure: 同TypeError |
| inv1_schema_qwen35_001 | `anvil-catalog-check:data_results_schema` | diagnosis / failure / 1 | executed=true, failure: ``results.json missing required key `reconciliation` `` |
| inv1_schema_gemma31_001 | `anvil-catalog-check:data_results_schema` | diagnosis / failure / 1 | executed=true, failure: 同上 |
| inv1_schema_qwen35_002 | `anvil-catalog-check:data_results_schema` | diagnosis / failure / 1 | executed=true, failure: 同上 |

`reproducer_defect` は0/6、R再構築も0/6。I1 evidenceは各runの `artifacts/<run>/evidence/investigation-run.json` に保存した。

## 7. I2 diagnosis_bound監査

| run | I2状態 | error_quote | file_line | code_snippet | total | match | violation | claims_absent |
|---|---|---:|---:|---:|---:|---:|---:|---|
| inv1_pipe_qwen35_001 | 未実行（diagnose停止） | — | — | — | — | — | — | n/a |
| inv1_pipe_gemma31_001 | 実行、failed | 2 | 0 | 3 | 5 | 0 | 5 | false |
| inv1_pipe_qwen35_002 | 未実行（diagnose停止） | — | — | — | — | — | — | n/a |
| inv1_schema_qwen35_001 | 未実行（不在path read） | — | — | — | — | — | — | n/a |
| inv1_schema_gemma31_001 | 未実行（diagnose停止） | — | — | — | — | — | — | n/a |
| inv1_schema_qwen35_002 | 実行、failed | 0 | 0 | 5 | 5 | 0 | 5 | false |

I2未実行を `claims_absent` と数えていない。`claims_absent` はI2を実行し抽出0件だった場合だけの契約分類である。

### I2違反原文全件: inv1_pipe_gemma31_001

```json
{
  "schema_version": "1",
  "intent": "investigate",
  "contract_version": "v0",
  "contract_ref": "docs/investigation-intent-contract.md",
  "requirement_id": "diagnosis_bound",
  "claims": [
    {
      "kind": "error_quote",
      "value": "ValueError",
      "subject_path": null,
      "line": null,
      "matched": false,
      "nearest": "outcome: CommandFailed status: exit status: 1 elapsed_ms: 30 summary: TypeError: list.append() takes exactly one argument (2 given) stdout: stderr: Traceback (most recent call last): File \"/Users/<user>/share/work/localwork/commandagent_mvp/01/test0718_inv_001/inv1_pipe_gemma31_001/pipeline/main.py\", line 181, in <module> run() File \"/Users/<user>/share/work/localwork/commandagent_mvp/01/test0718_inv_001/inv1_pipe_gemma31_001/pipeline/main.py\", line 164, in run report_lines.append(\"## 月次×地域別売上\","
    },
    {
      "kind": "code_snippet",
      "value": "python3 pipeline/main.py",
      "subject_path": null,
      "line": null,
      "matched": false,
      "nearest": "no referenced existing file"
    },
    {
      "kind": "code_snippet",
      "value": "Traceback (most recent call last):\n  File \"pipeline/main.py\", line 45, in <module>\n    process_sales(\"data/sales.csv\")\n  File \"pipeline/main.py\", line 36, in process_sales\n    total_revenue += float(row['price']) * int(row['quantity'])\nValueError: could not convert string to float: '1,200'",
      "subject_path": null,
      "line": null,
      "matched": false,
      "nearest": "no referenced existing file"
    },
    {
      "kind": "code_snippet",
      "value": "total_revenue += float(row['price']) * int(row['quantity'])",
      "subject_path": null,
      "line": null,
      "matched": false,
      "nearest": "no referenced existing file"
    },
    {
      "kind": "error_quote",
      "value": "ValueError",
      "subject_path": null,
      "line": null,
      "matched": false,
      "nearest": "outcome: CommandFailed status: exit status: 1 elapsed_ms: 30 summary: TypeError: list.append() takes exactly one argument (2 given) stdout: stderr: Traceback (most recent call last): File \"/Users/<user>/share/work/localwork/commandagent_mvp/01/test0718_inv_001/inv1_pipe_gemma31_001/pipeline/main.py\", line 181, in <module> run() File \"/Users/<user>/share/work/localwork/commandagent_mvp/01/test0718_inv_001/inv1_pipe_gemma31_001/pipeline/main.py\", line 164, in run report_lines.append(\"## 月次×地域別売上\","
    }
  ]
}
```

実RはTypeErrorだったが、診断はValueError、`pipeline/main.py:36/45`、実在しない処理断片を主張した。照合器はerror outputの最近傍をTypeError原文として残し、5件全てを拒否した。

### I2違反原文全件: inv1_schema_qwen35_002

```json
{
  "schema_version": "1",
  "intent": "investigate",
  "contract_version": "v0",
  "contract_ref": "docs/investigation-intent-contract.md",
  "requirement_id": "diagnosis_bound",
  "claims": [
    {
      "kind": "code_snippet",
      "value": "{\n  \"aggregation\": {\n    \"名古屋\": { \"2026-03\": 20730.0, \"2026-06\": 19767.0 },\n    \"大阪\": { \"2026-02\": 18657.0, \"2026-05\": 21470.0 },\n    \"東京\": { \"2026-01\": 19990.0, \"2026-02\": 5000.0, \"2026-04\": 16824.0 }\n  },\n  \"summary\": {\n    \"excluded_rows\": { \"invalid_format\": 0, \"missing_value\": 2 },\n    \"input_rows\": 60,\n    \"total_sales\": 122438.0,\n    \"used_rows\": 58\n  }\n}",
      "subject_path": null,
      "line": null,
      "matched": false,
      "nearest": "no referenced existing file"
    },
    {
      "kind": "code_snippet",
      "value": "{\n  \"capability_id\": \"data_results_schema\",\n  \"status\": \"failed\",\n  \"ok\": false,\n  \"results_path\": \"output/results.json\",\n  \"error\": \"results.json missing required key `reconciliation`\"\n}",
      "subject_path": null,
      "line": null,
      "matched": false,
      "nearest": "no referenced existing file"
    },
    {
      "kind": "code_snippet",
      "value": "{\n  \"reconciliation\": {\n    \"input_rows\": N,\n    \"used_rows\": N,\n    \"excluded\": [\n      { \"reason\": \"...\", \"rows\": N }\n    ]\n  }\n}",
      "subject_path": null,
      "line": null,
      "matched": false,
      "nearest": "no referenced existing file"
    },
    {
      "kind": "code_snippet",
      "value": "import csv\nimport json\nfrom collections import defaultdict\nfrom statistics import fsum\n\ndef main():\n    input_file = 'data/sales.csv'\n    results_file = 'output/results.json'\n    report_file = 'output/report.md'\n    # Deterministic state: no randomness used, stable iteration order via sorted keys\n    input_rows = 0\n    used_rows = 0\n    excluded_rows = {\n        'missing_value': 0,\n        'invalid_format': 0\n    }\n    # aggregation: region -> month -> total_sales\n    aggregation = defaultdict(lambda: defaultdict(float))\n    total_sum = ...",
      "subject_path": null,
      "line": null,
      "matched": false,
      "nearest": "no referenced existing file"
    },
    {
      "kind": "code_snippet",
      "value": "{\n  \"reconciliation\": {\n    \"input_rows\": 60,\n    \"used_rows\": 58,\n    \"excluded\": [\n      { \"reason\": \"missing_value\", \"rows\": 1 },\n      { \"reason\": \"invalid_format\", \"rows\": 1 }\n    ]\n  },\n  \"values\": {\n    \"regional_名古屋\": 40497.0,\n    \"regional_大阪\": 40127.0,\n    \"regional_東京\": 41814.0\n  }\n}",
      "subject_path": null,
      "line": null,
      "matched": false,
      "nearest": "no referenced existing file"
    }
  ]
}
```

診断本文はpathを散文・見出し内に書いたが、フェンスcode blockの `subject_path` は全てnullだった。契約が要求する機械照合可能な `path:line` 形式で束縛されず、5件全て `nearest=no referenced existing file` となった。内容の一部が実ファイルと似ていても、機械束縛を獲得していないためfailedが正しい。

## 8. イベント発火表

検索コマンド（不在の根拠にも同じ再帰検索を使用）:

```text
rg -n '"event":"(intent_resolved|investigation_plan_synthesized|investigation_adjudicated|host_env_normalized|plan_preset_resolved)"' \
  workspace/management/runs/uat-test0718-inv-001/artifacts/*/.anvil/runs/*/events.jsonl
```

| event | 発火 | 内容 |
|---|---:|---|
| intent_resolved | 6/6 | `origin=cli`, `value=investigate` |
| plan_preset_resolved | 6/6 | `profile`, `origin=default_investigate_data` |
| investigation_plan_synthesized | 6/6 | `profile=data`, `phase_count=3`, family別r_basis |
| host_env_normalized | 6/6 | inherited `NODE_ENV` をbounded process childでunset |
| investigation_adjudicated | 2/6 | run 2, 6のみ。ともに `assurance_level=failed`, `assurance_reason=diagnosis_unbound` |

`investigation_adjudicated` がrun 1, 3, 4, 5に不在なのは、これらがbind-verify前のdiagnose段で終了したためである。同じ検索で該当4runは0 hit。event不在とI2未実行は整合する。

## 9. Assurance投影不整合

確認コマンド:

```text
rg -n '^Assurance:|^Stop reason:' \
  workspace/management/runs/uat-test0718-inv-001/artifacts/*/.anvil/runs/*/summary.md
```

全runのsummaryに `Assurance: static (data_profile_probe_not_run)` がある。一方:

- investigation-run evidenceは全6runでR実行済みなので、契約§4のstatic（診断は書かれたがR未実行）には該当しない。
- run 1, 3, 4, 5はdiagnosis.md未生成なので、やはり契約staticの必要条件を満たさない。
- run 2, 6は `investigation_adjudicated=failed(diagnosis_unbound)` であり、summaryのstaticと直接矛盾する。

したがって契約固有裁定自体は虚偽診断を正しくfailedへ落とすが、terminal summaryへのassurance投影が旧data profileの理由を参照している。runの一次死因ではないものの、第三者がsummaryだけを読むと契約§4と異なる階層を観測するためP0-bはgreenにできない。

## 10. full evidence

fullは0/6のため、I1・I2・diagnosis.md全文を転記する対象runはない。diagnosis.mdを生成したrun 2, 6の原本とbinding evidenceは各artifactsに保存した。

## 11. コスト

| 区分 | epoch / 実測 | 所要 |
|---|---|---:|
| 調達・R事前確認・provenance | `1784382916→1784383008` | 92秒 |
| 製品実行（6本の純計） | 各 `uat-console.log` 前後epoch差の合計 | 437秒 (7分17秒) |
| 実行区間wall（run間退避を含む） | `1784383028→1784383668` | 640秒 (10分40秒) |
| レポート・監査 | `1784383668→1784384050` | 382秒 (6分22秒) |

分節用の人手ターミナル切替は不要だった。環境終了による非消費attemptも0。各run終端直後に `.anvil/`, `pipeline/`, `output/`, `data/`, `evidence/`, `uat-console.log` を退避した。
