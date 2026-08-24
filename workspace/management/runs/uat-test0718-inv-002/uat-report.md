# uat-test0718-inv-002: investigation intent 第2計測

実施日: 2026-07-18〜19 (JST)

契約: `docs/investigation-intent-contract.md` (fixed, `87d432a`)

INV-1基準: `3dea4e8` / `3302dd9`

計測HEAD: `3302dd9 Guide investigation diagnosis binding`

計測workspace: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0718_inv_002`

## 結論

6/6は製品exit 1で理由付きの正直なfailed終端となり、environment interruption、panic、再試行は0だった。I1は6/6で成立し、`stage=diagnosis`, `expected=failure`, `executed=true`, `epoch=1` と実失敗を全runの `evidence/investigation-run.json` で確認した。合成3段、`investigation_plan_synthesized`、`plan_preset_resolved.origin=default_investigate_data` も6/6である。

INV-1の主目的だった投影は回復した。summary表示はI2未到達4本が `failed(investigation_incomplete)`、I2違反2本が `failed(diagnosis_unbound)` で、契約assuranceと6/6一致した。`data_profile_probe_not_run` は再帰検索で0件。したがってP0-bはPASSである。

fullは0/6。4本はdiagnose段でdiagnosis.mdを作れずI2未実行、2本はI2まで到達したが、4件および3件の全抽出主張が不一致で `diagnosis_unbound` となった。照合器は虚偽・未束縛診断をfailedへ落としており、偽成功は0である。run 6型のコードブロック違反はinv-001の5件から本計測の2件へ減ったが、別の不正確なerror quote 1件もあり、形式逸脱は消滅していない。全6runの一次死因はモデル帰属で、機械一次死因は0/6だった。

| 基準 | 結果 | 根拠 |
|---|---|---|
| P0-a 6/6正直終端 | PASS | 全run product exit 1、理由付きfailed、environment interruption / panic / 再試行 0 |
| P0-b 契約§4準拠 | PASS | 契約assuranceとsummary表示が6/6一致。旧 `data_profile_probe_not_run` 0件 |
| P0-c 偽成功ゼロ | PASS | full/partial 0、I2違反2本はいずれもfailed |
| P1-a 合成6/6＋既定preset | PASS | synthesis event 6/6、3段plan 6/6、origin 6/6 |
| P1-b I1実行・失敗6/6 | PASS | investigation-run evidence 6/6 |

## 1. Preflight

全greenを確認してから製品runを開始した。

| 項目 | 結果 |
|---|---|
| `git status --porcelain` | 空 |
| `git log -1 --oneline` | `3302dd9 Guide investigation diagnosis binding` |
| `git merge-base --is-ancestor 3302dd9 HEAD` | exit 0 |
| 権限付き `cargo test` | exit 0。lib 1,447 passed / 15 ignored、全integration suite・doc test green |
| `cargo build --release` | exit 0 |
| install | `install -m 755 target/release/commandagent ~/.local/bin/commandagent` exit 0 |
| version | `commandagent 0.1.0 3302dd9 2026-07-18T14:49:41Z`（`+dirty`なし） |
| binary SHA-256 | build/installとも `e0995ca0c248f459969f98baed73b6e0a0be6676928d6385b6ed05046248d6b4` |
| host `NODE_ENV` | `production`。各runで `host_env_normalized` が発火 |

## 2. 出発点とprovenance

採取元は `workspace/management/runs/uat-test0717-dfix-002/artifacts/source-checks/`。各runには対応setの `pipeline/`, `output/`, `data/` のみを新規コピーした。採取元の `reproducer.*.log`, `catalog-helper-*.log`, `.git` の持込は0。runディレクトリ内の `preflight-r.*` はコピー後に本計測で生成した事前確認ログである。

| set | pipeline/main.py | output主要物 | data/sales.csv | R事前確認 |
|---|---|---|---|---|
| pipe-a | `4944322105b422f71338513405e3de57d2c698aab565f313fac794e4163ad1d3` | inspection `0e76d134335ffd290fb457321c2b0cd94412bd844f1f4732b9a089116aa5a6a7` | `2f6c04e42b0ebdff85a7eb6b52a342610155be6796bd89e5729075d87c78d873` | exit 1、`ValueError: invalid literal for int() with base 10: ''` |
| pipe-b | `b27e8aaffef74dac171ff19dffb89eb891c97ee2038045887dae8d43719a511b` | results `af452a62d26b5e377453bbd1daddc27282bd61c806aeff30d6616dd02afbf6f2`; inspection `6bd2ec134cbbc6fda43c8f256e941779849e63152cee8f36d4c4df2f6d1b4558` | 同上 | exit 1、`TypeError: list.append() takes exactly one argument (2 given)` |
| schema-a / schema-b | `5e2d7efe794c78a20dded6ca1b0b4e449293fe2609c8274c302b102ddb0c7c96` | results `a0e3a1dfd4a2378598efead1da29673d2d91e09c6fdebecb1f9b1db5e7dd07ca`; report `17af1dd02de623845341e4a427c0c7ec5b26254de6524030bc5842d295165f97` | 同上 | exit 1、`AssertionError` |

これらは `uat-test0717-dfix-002/analysis/source-provenance.json` の全対応hashと一致する。連鎖は「過去UAT原資料 → dfix-002 qualified source-check → inv-002独立fresh copy」で、壊れた出発点の合成・補完は0である。実行後も全6runの `pipeline/main.py` と `data/sales.csv` は採取元に対する `cmp` が0で、調査による修正は0だった。

R事前確認自体は各runで1回だけ実行し、上記のPython例外をstderrに得た。終了値保存用補助変数にzsh予約名 `status` を用いたため、補助記録処理が `zsh:2: read-only variable: status` で終了した。これは製品run前の記録補助だけの事故であり、生成済みstderrとPython例外のexit 1を確認して `preflight-r.exit-code.txt` を保存した。Rは再実行していない。この経緯は `artifacts/campaign/procurement.log` にも記録した。

## 3. 実行コマンド

各runでexecutorとgoalだけを置換し、次の形を1回だけ起動した。`timeout`, `env`, `nice`, `nohup` その他wrapperは不使用。標準shell redirectionで製品出力を `uat-console.log` に保存し、その前後に `date +%s` と `date` を記録した。

```text
commandagent --yes --intent investigate --context-budget 65536 \
  --model <executor> --provider ollama \
  --planner-model qwen3.6:27b-coding-nvfp4 --planner-provider ollama \
  --ultra-plan-run --profile data "<goal>"
```

## 4. Run行列

「契約assurance」は契約§4に従い、`investigation_adjudicated` があればその値、I1実行済みで裁定未到達なら `failed(investigation_incomplete)` とした。INV-1後の投影確認のためsummary表示を別列に保つ。

| # | run | set / executor | verdict | 契約assurance | summary表示 | I1 | I2 | 失敗クラス（帰属） | 秒 |
|---:|---|---|---|---|---|---|---|---|---:|
| 1 | inv2_pipe_qwen35_001 | pipe-a / qwen35 | failed (honest) | failed (`investigation_incomplete`) | failed (`investigation_incomplete`) | pass | 未実行 | diagnose `artifact_recovery_exhausted`、診断成果物未作成（モデル） | 198 |
| 2 | inv2_pipe_gemma31_001 | pipe-b / gemma31 | failed (honest) | failed (`investigation_incomplete`) | failed (`investigation_incomplete`) | pass | 未実行 | diagnose `artifact_follow_through_exhausted`、diagnosis.md不在（モデル） | 118 |
| 3 | inv2_pipe_qwen35_002 | pipe-b / qwen35 | failed (honest) | failed (`investigation_incomplete`) | failed (`investigation_incomplete`) | pass | 未実行 | diagnose `artifact_recovery_exhausted`、診断成果物未作成（モデル） | 106 |
| 4 | inv2_schema_qwen35_001 | schema-a / qwen35 | failed (honest) | failed (`diagnosis_unbound`) | failed (`diagnosis_unbound`) | pass | fail: 4/4違反 | path:lineに束縛されないcode block 4件（モデル）。照合拒否は機械の正常動作 | 124 |
| 5 | inv2_schema_gemma31_001 | schema-b / gemma31 | failed (honest) | failed (`investigation_incomplete`) | failed (`investigation_incomplete`) | pass | 未実行 | diagnose `artifact_follow_through_exhausted`、diagnosis.md不在（モデル） | 335 |
| 6 | inv2_schema_qwen35_002 | schema-a / qwen35 | failed (honest) | failed (`diagnosis_unbound`) | failed (`diagnosis_unbound`) | pass | fail: 3/3違反 | 架空 `ValueError` prefixと未束縛code block 2件（モデル）。照合拒否は機械の正常動作 | 36 |

計時epochは順に `1784386575→1784386773`, `1784386817→1784386935`, `1784386969→1784387075`, `1784387120→1784387244`, `1784387282→1784387617`, `1784387676→1784387712`。全て `product_exit=1`。環境中断0、再実行0、人手ターミナル切替0。

一次死因分布はモデル6/6、機械0/6、複合0/6。族別fullはpipe 0/3、schema 0/3。executor別fullはqwen35 0/4、gemma31 0/2。

## 5. INV-1投影監査

使用した検索コマンド:

```text
rg -n '^Assurance:|^Stop reason:|data_profile_probe_not_run' \
  workspace/management/runs/uat-test0718-inv-002/artifacts/*/.anvil/runs/*/summary.md
```

全6runのsummaryはrun行列の契約assuranceと一致した。`data_profile_probe_not_run` は同じ再帰検索で0 hit。I1実行済み・裁定未到達を `failed(investigation_incomplete)` とし、I2違反を `failed(diagnosis_unbound)` とするintent dispatchが実戦6/6で観測された。inv-001で6/6発生した旧data投影の再発は0である。

## 6. 合成計画・diagnoseガイダンス監査

全6runのcanonical `ultra-plan-*.yaml` に次の3段がこの順で実在した。

1. `reproduce-candidate`
2. `diagnose`
3. `bind-verify`

検索コマンド:

```text
rg -n 'id: "(reproduce-candidate|diagnose|bind-verify)"' \
  workspace/management/runs/uat-test0718-inv-002/artifacts/*/.anvil/plans/ultra-plan-*.yaml
```

結果は18 hit（3 phase × 6 run）。`investigation_plan_synthesized` はpipe族で `r_basis=goal_failure_kind:pipeline_execution`、schema族で `r_basis=goal_profile_contract:data_results_schema`。全runの `plan_preset_resolved` は `plan_preset=profile`, `origin=default_investigate_data`, `source=default_investigate_data` だった。

diagnose StepPlanのinstructionを次で再帰検索した。

```text
rg -n '診断の主張は次の形式|エラー引用:|位置: pipeline/main.py:53|修正案・例示コード|実在ファイル一覧|一覧にないファイル' \
  workspace/management/runs/uat-test0718-inv-002/artifacts/*/.anvil/plans/plan-*.yaml
```

6/6に次の字義例と制約が実在した。

````text
診断の主張は次の形式で書くこと（以下の値は形式例であり、必ず再現Rの実出力と実在ファイルから得た実観測値で置き換える）:
エラー引用: `ValueError: could not convert string to float: ''`
位置: pipeline/main.py:53
コード引用（実在コードのみ）:
```python
amount = float(row["amount"])
```
修正案・例示コードはコードブロックにせず、
『修正方針:』以下に文章で書くこと（照合対象外となる）。
...
一覧にないファイルを参照しないこと。
````

実在ファイル一覧はrun 1が8件、run 2/3が9件、run 4/5/6が10件で、記載された全pathの存在を各run成果物で確認した。不在 `output/inspection.json` を要求したinv-001 run 4型は本計測0件だった。

ガイダンスの効果は部分的である。inv-001 run 6の未束縛code block 5件に対し、対応するinv-002 run 6はcode block 2件へ減少した。ただしerror quoteに実出力にない `ValueError:` を付けた1件も加わり、合計3件が違反となった。またinv-002 run 4には未束縛code block 4件があり、キャンペーン全体のcode block違反はinv-001の8件からinv-002の6件へ減ったがゼロにはなっていない。

## 7. I1 reproducer_fails監査

| run | R原文 | stage / expected / epoch | 実結果 |
|---|---|---|---|
| inv2_pipe_qwen35_001 | `python3 -B pipeline/main.py` | diagnosis / failure / 1 | executed=true, failure: `ValueError: invalid literal for int() with base 10: ''` |
| inv2_pipe_gemma31_001 | `python3 -B pipeline/main.py` | diagnosis / failure / 1 | executed=true, failure: `TypeError: list.append() takes exactly one argument (2 given)` |
| inv2_pipe_qwen35_002 | `python3 -B pipeline/main.py` | diagnosis / failure / 1 | executed=true, failure: 同TypeError |
| inv2_schema_qwen35_001 | `anvil-catalog-check:data_results_schema` | diagnosis / failure / 1 | executed=true, failure: ``results.json missing required key `reconciliation` `` |
| inv2_schema_gemma31_001 | `anvil-catalog-check:data_results_schema` | diagnosis / failure / 1 | executed=true, failure: 同上 |
| inv2_schema_qwen35_002 | `anvil-catalog-check:data_results_schema` | diagnosis / failure / 1 | executed=true, failure: 同上 |

`reproducer_defect` は0/6、R再構築も0/6。I1 evidenceは各runの `artifacts/<run>/evidence/investigation-run.json` に保存した。

## 8. I2 diagnosis_bound監査

| run | I2状態 | error_quote | file_line | code_snippet | total | match | violation | claims_absent |
|---|---|---:|---:|---:|---:|---:|---:|---|
| inv2_pipe_qwen35_001 | 未実行（diagnose停止） | — | — | — | — | — | — | n/a |
| inv2_pipe_gemma31_001 | 未実行（diagnose停止） | — | — | — | — | — | — | n/a |
| inv2_pipe_qwen35_002 | 未実行（diagnose停止） | — | — | — | — | — | — | n/a |
| inv2_schema_qwen35_001 | 実行、failed | 0 | 0 | 4 | 4 | 0 | 4 | false |
| inv2_schema_gemma31_001 | 未実行（diagnose停止） | — | — | — | — | — | — | n/a |
| inv2_schema_qwen35_002 | 実行、failed | 1 | 0 | 2 | 3 | 0 | 3 | false |

I2未実行を `claims_absent` と数えていない。`claims_absent` はI2を実行し抽出0件だった場合だけの契約分類である。I2実行2本は全7件を違反として拒否した。

### I2違反原文全件: inv2_schema_qwen35_001

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
      "value": "results = {\n    \"aggregation\": { ... },\n    \"summary\": {\n        \"excluded_rows\": { \"invalid_format\": 0, \"missing_value\": 2 },\n        \"input_rows\": 60,\n        \"total_sales\": 122438.0,\n        \"used_rows\": 58\n    }\n}\nwith open(results_file, 'w') as f:\n    json.dump(results, f, indent=2)",
      "subject_path": null,
      "line": null,
      "matched": false,
      "nearest": "no referenced existing file"
    },
    {
      "kind": "code_snippet",
      "value": "{\n  \"aggregation\": { ... },\n  \"summary\": {\n    \"excluded_rows\": { \"invalid_format\": 0, \"missing_value\": 2 },\n    \"input_rows\": 60,\n    \"total_sales\": 122438.0,\n    \"used_rows\": 58\n  }\n}",
      "subject_path": null,
      "line": null,
      "matched": false,
      "nearest": "no referenced existing file"
    },
    {
      "kind": "code_snippet",
      "value": "{\n  \"aggregation\": { ... },\n  \"summary\": { ... },\n  \"reconciliation\": {\n    \"input_rows\": 60,\n    \"used_rows\": 58,\n    \"excluded\": [\n      {\"reason\": \"missing_value\", \"rows\": 2}\n    ]\n  }\n}",
      "subject_path": null,
      "line": null,
      "matched": false,
      "nearest": "no referenced existing file"
    },
    {
      "kind": "code_snippet",
      "value": "# Sales Summary Report\n- Total Input Rows: 60\n- Valid Rows Used: 58\n- Excluded Rows:\n  - invalid_format: 0\n  - missing_value: 2\n- Total Sales: 122438.00",
      "subject_path": null,
      "line": null,
      "matched": false,
      "nearest": "no referenced existing file"
    }
  ]
}
```

### I2違反原文全件: inv2_schema_qwen35_002

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
      "value": "ValueError: results.json missing required key 'reconciliation'",
      "subject_path": null,
      "line": null,
      "matched": false,
      "nearest": "results.json missing required key `reconciliation`"
    },
    {
      "kind": "code_snippet",
      "value": "{\n  \"aggregation\": {\n    \"名古屋\": {\"2026-03\": 20730.0, \"2026-06\": 19767.0},\n    \"大阪\": {\"2026-02\": 18657.0, \"2026-05\": 21470.0},\n    \"東京\": {\"2026-01\": 19990.0, \"2026-02\": 5000.0, \"2026-04\": 16824.0}\n  },\n  \"summary\": {\n    \"excluded_rows\": {\"invalid_format\": 0, \"missing_value\": 2},\n    \"input_rows\": 60,\n    \"total_sales\": 122438.0,\n    \"used_rows\": 58\n  }\n}",
      "subject_path": null,
      "line": null,
      "matched": false,
      "nearest": "no referenced existing file"
    },
    {
      "kind": "code_snippet",
      "value": "{\n  \"reconciliation\": {\n    \"input_rows\": N,\n    \"used_rows\": N,\n    \"excluded\": [{\"reason\": \"...\", \"rows\": N}]\n  },\n  \"values\": {\n    \"<claim_key>\": number\n  }\n}",
      "subject_path": null,
      "line": null,
      "matched": false,
      "nearest": "no referenced existing file"
    }
  ]
}
```

両runともdiagnosis.md原本は `artifacts/<run>/output/diagnosis.md` に保存した。run 4は位置を ``pipeline/main.py` (行末部分...)`` と書き、契約の `path:line` 形式にしなかったため、code blockの `subject_path` が全てnullとなった。run 6も位置を ``output/results.json` のトップレベルキー構成`` と書き、lineを束縛しなかった。内容が実ファイルに似ていても機械束縛を獲得していないためfailedが正しい。

## 9. イベント発火表

検索コマンド（不在主張にも同じ再帰検索を使用）:

```text
rg -n '"event":"(intent_resolved|plan_preset_resolved|investigation_plan_synthesized|investigation_adjudicated|host_env_normalized)"' \
  workspace/management/runs/uat-test0718-inv-002/artifacts/*/.anvil/runs/*/events.jsonl
```

| event | 発火 | 内容 |
|---|---:|---|
| intent_resolved | 6/6 | `origin=cli`, `value=investigate` |
| plan_preset_resolved | 6/6 | `profile`, `origin=default_investigate_data` |
| investigation_plan_synthesized | 6/6 | `profile=data`, `phase_count=3`, family別r_basis |
| host_env_normalized | 6/6 | inherited `NODE_ENV` をbounded process childでunset |
| investigation_adjudicated | 2/6 | run 4, 6。ともに `failed(diagnosis_unbound)` |

`investigation_adjudicated` がrun 1, 2, 3, 5に不在なのはbind-verify前にdiagnose段で終了したため。同じ検索で該当4runは0 hitで、I2 evidence不在と整合する。

## 10. inv-001＋inv-002合算12run

| campaign | run | family / executor | terminal class | 一次死因 | 帰属 |
|---|---|---|---|---|---|
| inv-001 | inv1_pipe_qwen35_001 | pipe / qwen35 | failed | diagnose read-only loop | モデル |
| inv-001 | inv1_pipe_gemma31_001 | pipe / gemma31 | failed (`diagnosis_unbound`) | 架空ValueError・コード5件 | モデル |
| inv-001 | inv1_pipe_qwen35_002 | pipe / qwen35 | failed | diagnose read-only loop | モデル |
| inv-001 | inv1_schema_qwen35_001 | schema / qwen35 | failed | 不在 `output/inspection.json` Read | モデル |
| inv-001 | inv1_schema_gemma31_001 | schema / gemma31 | failed | diagnose read-only loop | モデル |
| inv-001 | inv1_schema_qwen35_002 | schema / qwen35 | failed (`diagnosis_unbound`) | 未束縛code block 5件 | モデル |
| inv-002 | inv2_pipe_qwen35_001 | pipe / qwen35 | failed (`investigation_incomplete`) | artifact recovery exhausted | モデル |
| inv-002 | inv2_pipe_gemma31_001 | pipe / gemma31 | failed (`investigation_incomplete`) | artifact follow-through exhausted | モデル |
| inv-002 | inv2_pipe_qwen35_002 | pipe / qwen35 | failed (`investigation_incomplete`) | artifact recovery exhausted | モデル |
| inv-002 | inv2_schema_qwen35_001 | schema / qwen35 | failed (`diagnosis_unbound`) | 未束縛code block 4件 | モデル |
| inv-002 | inv2_schema_gemma31_001 | schema / gemma31 | failed (`investigation_incomplete`) | artifact follow-through exhausted | モデル |
| inv-002 | inv2_schema_qwen35_002 | schema / qwen35 | failed (`diagnosis_unbound`) | 架空error prefix＋未束縛code block 2件 | モデル |

| 指標 | inv-001 | inv-002 | 合算 |
|---|---:|---:|---:|
| 正直終端 | 6/6 | 6/6 | 12/12 |
| I1成立 | 6/6 | 6/6 | 12/12 |
| I2実行 | 2/6 | 2/6 | 4/12 |
| I2成立 | 0/6 | 0/6 | 0/12 |
| full | 0/6 | 0/6 | 0/12 |
| 一次死因: 機械 | 0 | 0 | 0 |
| 一次死因: モデル | 6 | 6 | 12 |
| 一次死因: 複合 | 0 | 0 | 0 |
| summary投影不整合（一次死因外） | 6 | 0 | 6 |

合算I2はerror_quote 3件、file_line 0件、code_snippet 14件の計17件を抽出し、match 0、violation 17だった。pipe族full 0/6、schema族full 0/6。qwen35 full 0/8、gemma31 full 0/4。I2到達はpipe 1/6、schema 3/6、qwen35 3/8、gemma31 1/4である。

INV-1は投影不整合と不在inspection参照を除去したが、diagnose成果物の完遂と機械照合可能な `path:line` 束縛は依然として残る壁である。一次死因の機械/モデル帰属では12/12がモデル側である一方、機械側はI1、投影、I2拒否を契約どおり実施した。

## 11. full evidence

fullは0/6のため、I1・I2・diagnosis.md全文を完全転記する対象runはない。I2まで到達したrun 4, 6のI1/I2 evidenceとdiagnosis.md原本はartifactsに保存し、違反全件は上記へ転記した。

## 12. コスト

| 区分 | epoch / 実測 | 所要 |
|---|---|---:|
| 調達・R事前確認・provenance | `1784386304→1784386575`（workspace birth epochから最初のrun前 `date +%s`） | 271秒 (4分31秒) |
| 製品実行（6本の純計） | 各 `uat-console.log` 前後epoch差の合計 | 917秒 (15分17秒) |
| 実行区間wall（run間退避を含む） | `1784386575→1784387712` | 1,137秒 (18分57秒) |
| レポート・監査 | `1784387747→1784388232` | 485秒 (8分05秒) |

分節用の人手ターミナル切替は不要だった。環境終了による非消費attemptも0。各run終端直後に `.anvil/`, `pipeline/`, `output/`, `data/`, `evidence/`, `uat-console.log` を退避した。R事前確認の補助記録事故と補正時間は調達コストに含む。
