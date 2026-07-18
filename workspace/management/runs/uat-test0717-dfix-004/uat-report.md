# UAT dfix-004 — blocked before execution

基準HEAD: `64e2616`。権限付き環境で `cargo test -q` full suite は成功した（browser probeを含む）。

しかし、指定された計測ワークスペース `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0717_dfix_004/` が存在せず、採取済みの出発点・provenance・artifactsも確認できなかった。合成禁止かつ各run最大1回の規律のため、出発点を推測・合成して実行していない。

従って本計測の6 run、イベント監査、F系evidence、FIX-8/9再発判定、#1〜#4合算更新は未実施。run再実行も行っていない。D-2クローズ判定には使用しない。

Preflight: cargo test PASS。調達・実行・レポート所要: 調達/実行 0（workspace不在で開始前停止）、本報告作成時間のみ。

## v2: 再発行による実施

再発行HEADは `64e2616` 以降（実際のHEAD `2066ef2`）。ホスト `NODE_ENV=production` を記録した。権限付き `cargo test -q` はfull suite greenだった。

ただし、指定された採取元のうちローカル `test0713_data_001` が存在せず、リポジトリ側 `uat-test0713-data-001` にも成果物artifactsがなく、schema/A・schema/Bの合成禁止調達を満たせなかった。pipe系の候補（`test0714_m4_001`、`test0715_data_005`）は存在するが、4セットが揃わないため、各run最大1回の実行規律に従い6本とも開始していない。

従ってevents.jsonlの再帰監査、F系evidence、FIX-8/9再発判定、24run合算更新は未実施。実行を捏造せず、D-2クローズ判定には使用しない。

## v3: 採取元をリポジトリ内スナップショットに変更して実施

HEAD `85a02c7` 以降で実施。指定された `uat-test0717-dfix-002/artifacts/source-checks/{pipe-a,pipe-b,schema-a,schema-b}` をコピーし、4セットのR事前確認と `sales.csv` SHA-256（全件 `2f6c04e42b0ebdff85a7eb6b52a342610155be6796bd89e5729075d87c78d873`）を確認した。権限付き `cargo test -q` と release build/install/version は成功し、ホスト `NODE_ENV=production` を記録した。

6本は各1回試行したが、実行ラッパーに `timeout` コマンドが存在せず、全件がcommand実行前に終了コード127となった。各runの `uat-console.log` に `run: command not found: timeout`、`outer-exit-code.txt` に127を保存している。再試行は行わない。

| run | 終端 | F1/F2/F3 | 帰属 |
|---|---|---|---|
| dfix4_pipe_qwen35_001 | failed(127) | not executed | 計測ハーネス |
| dfix4_pipe_gemma31_001 | failed(127) | not executed | 計測ハーネス |
| dfix4_pipe_qwen35_002 | failed(127) | not executed | 計測ハーネス |
| dfix4_schema_qwen35_001 | failed(127) | not executed | 計測ハーネス |
| dfix4_schema_gemma31_001 | failed(127) | not executed | 計測ハーネス |
| dfix4_schema_qwen35_002 | failed(127) | not executed | 計測ハーネス |

従ってevents.jsonl、FIX-8/9再発監査、full/F evidenceは生成されず、P0/P1は未判定。調達は完了、実行はハーネス欠陥で停止、コスト詳細はv3 workspaceとartifacts-v3に保存した。D-2クローズ判定には使用しない。

## v4: 完全一致コマンド再発行 — preflightで停止

レビュー裁定を転記する: **v3の6試行はcommandagent起動ゼロのためrun消費に数えない。** v4では独自のtimeout・env・niceその他wrapperを一切追加せず、指定された完全一致コマンドだけを各1回起動する方針を事前固定した。

しかし、全green必須のpreflightで `cargo test` が終了コード101となった。HEADは `2d357047573ac2967a1acbf44db649861fbe2d13`（`2d35704` 以降の条件を満たす）で、`git fetch origin develop` 後も `HEAD == origin/develop == FETCH_HEAD` だった。作業ツリーに追跡済み・未追跡ファイルはなくstash対象なし、ホスト `NODE_ENV=production` も記録した。

full suiteコマンドはlibrary suiteの1432 pass / 0 fail / 15 ignoredを通過後、`tests/generality_guardrails.rs` の `runner_chokepoints_do_not_grow_past_interim_budget` で停止した。実行済み範囲の失敗はこれ1件である。原文は次のとおりで、focused再確認も同じ終了コード101だった。

```text
src/planner/fix_runtime/data_isolate.rs production code grew to 160 lines; production code baseline is 149, allowed max is 152. Move new subsystems to new modules or land a shrinking refactor first.
```

本タスクでは `src/`、`tests/`、`docs/` の変更が禁止され、guardrail baseline引上げも禁止されている。従ってpreflightを偽ってgreenにせず、release build/install、出発点調達、6本の `commandagent` 起動を行わなかった。新規計測workspaceも作成していない。v4の製品run消費は0、再試行も0である。証跡は `artifacts-v4/preflight.log` に保存した。

### v4判定

| 項目 | 結果 | 根拠 |
|---|---|---|
| preflight | failed | full `cargo test` exit 101、focused確認もexit 101 |
| P0-a 6/6正直終端 | 未判定 | v4 run起動ゼロ |
| P0-b 契約§4準拠 | 未判定 | F evidence未生成 |
| P0-c 偽成功ゼロ | 未判定 | v4 run起動ゼロ |
| P1-a FIX-8/9 4クラス再発ゼロ | 未判定 | events/run成果物未生成 |
| full率・族/executor差・残存クラス | 未集計 | v4 6 runが存在せず24run母集団を構成できない |

イベント発火表、F系evidence表、FIX-8/9の所有権重複lint・空verify scaffold error・不在成果物要求・UTF-8 phase死の再帰監査、および#1〜#4合算分布は、対象となるv4 `.anvil/runs/*/events.jsonl` と `evidence/fix-*.json` が存在しないため実施不能である。検索結果やevidenceを捏造してゼロ件とは判定しない。

### D-2コスト会計

| 発行 | 調達 | 製品実行 | レポート | 空振り要因 |
|---|---:|---:|---:|---|
| v1 | 0 | 0 | あり | 指定workspace不在 |
| v2 | 採取元探索のみ | 0 | あり | schema採取元不在 |
| v3 | 4セット調達完了 | 0 | あり | 存在しないtimeout wrapperで6件exit 127 |
| v4 | 0 | 0 | あり | full suite guardrail failureでpreflight停止 |

v4の実時計測は、preflight・上流確認・focused再確認が259秒（12:08:50〜12:13:09 JST）、調達0秒、製品実行0秒、初稿作成が84秒（12:13:09〜12:14:33 JST）だった。v1〜v3の調達・preflight・報告作成、およびv3の6回のハーネス空振りはD-2の空振りコストとして残る。ただしレビュー裁定どおりv3は製品run消費0であり、v4も製品run消費0である。D-2クローズ判定には使用しない。

## v5: 予算分離後の正式実行

実施日: 2026-07-18<br>
基準HEAD: `75b6a10` (`Record FIX-9 budget guardrail follow-up`)<br>
製品version: `commandagent 0.1.0 75b6a10 2026-07-18T05:12:18Z`<br>
契約: `docs/fix-intent-contract.md` v0 fixed<br>
計測workspace: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0717_dfix_004_v5`

### レビュー裁定の転記

**v3の6試行はtimeout wrapper事故によりcommandagent起動ゼロのためrun消費に数えない。v4はpreflight guardrail失敗により製品run消費0。本v5がdfix-004の正式実行である。**

本v5では外部`timeout`、`env`、`nice`、`time`、pipe、出力redirectその他wrapperを製品起動へ一切加えなかった。各`run-command.txt`のコマンドをそのまま1回だけ起動した。`uat-console.log`には製品起動と別操作で直前・直後の`date +%s`と`date`だけを記録し、製品の構造化terminal記録は各`.anvil/runs/*/{events.jsonl,summary.md}`へ保存した。外部run再試行は0である。

### 結果

**Overall: FAIL.** 6本はすべて製品を起動し、F1を実行したうえで分類済み`run_stop`へ正直に終端したためP0-aはPASS。6本とも`failed / failed(after_not_executed)`でF1成立・F2/F3未実行と矛盾せず、§4より高いassuranceを主張していないためP0-bはPASS。fullは0で偽fullも0のためP0-cはPASS。一方、FIX-8の所有権重複がpipe 3/3、FIX-9bの不在成果物要求がschema 2/3で再発したためP1-aはFAILである。

| Gate | Result | Evidence |
|---|---|---|
| P0-a: 6/6正直終端 | **PASS** | `run_stop` 6/6、outer exit 1が6/6、panic 0、分類不能0 |
| P0-b: 契約§4準拠 | **PASS** | 6/6 `failed(after_not_executed)`、F1 pass、F2/F3 not_executed |
| P0-c: 偽成功ゼロ | **PASS** | full 0/6、partial 0/6、static 0/6、false full 0 |
| P1-a: FIX-8/9 4クラス再発ゼロ | **FAIL** | 所有権重複3run、不在成果物要求2run、空verify 0、UTF-8死0 |

### Preflight

| Check | Result |
|---|---|
| Clean tree | `git status --porcelain`空。別作業・未追跡なし、stashなし |
| Revision | `git log -1 --oneline` = `75b6a10 ...`; `git merge-base --is-ancestor 75b6a10 HEAD` exit 0 |
| Full suite | 権限付き`cargo test` exit 0。library 1432 passed / 15 ignored、adjudication 6/6、fix conformance 9/9、guardrail 7/7、全integration/doc test green |
| Release build/install | `cargo build --release`、指定`install -m 755 ...`ともexit 0 |
| Version / dirty | `75b6a10`、`+dirty`なし |
| Binary SHA-256 | build/installedとも`dd105d719ef436674f3cff688a711b9102e2ed84e483263cc63d2ccb86718f01` |
| Host environment | `NODE_ENV=production` |
| Models | 権限付きread-only確認でplanner、qwen35 executor、gemma31 executorすべて実在 |

preflightは1784351424〜1784351581（157秒）。詳細は`artifacts-v5/preflight.log`に保存した。

### 出発点とprovenance

dfix-002の`analysis/source-provenance.json`から`artifacts/source-checks/{pipe-a,pipe-b,schema-a,schema-b}`へ至る既存chainを継承し、そこから各runへ`pipeline/`、`output/`、`data/`だけをfresh copyした。`.git`、採取元`reproducer.*.log`、`catalog-helper-*.log`の持込は0。合成した壊れ成果物は0である。

| Set | Principal SHA-256 | R事前確認 |
|---|---|---|
| pipe-a | `pipeline/main.py` `49443221…` | exit 1、採取元と同じline 53 `ValueError` |
| pipe-b | `pipeline/main.py` `b27e8aaf…` | 2 copyともexit 1、採取元と同じline 164 `TypeError` |
| schema-a | `results.json` `a0e3a1df…` | 2 copyとも指定one-liner exit 1、`AssertionError` |
| schema-b | `results.json` `a0e3a1df…` | exit 1、`AssertionError` |

`data/sales.csv`はR前後とも6/6で`2f6c04e42b0ebdff85a7eb6b52a342610155be6796bd89e5729075d87c78d873`。全hash、copy policy、dfix-002 chainは`artifacts-v5/provenance.md`へ記録した。調達は1784351618〜1784351825（207秒）。

### 完全一致コマンド

各runはexecutorとgoalだけを表どおり置換し、次の形をそのまま起動した。

```text
commandagent --yes --intent fix --context-budget 65536 \
  --model <executor> --provider ollama \
  --planner-model qwen3.6:27b-coding-nvfp4 --planner-provider ollama \
  --plan-preset none --ultra-plan-run --profile data "<goal>"
```

実際の非省略コマンドは各runの`run-command.txt`に保存した。

### Run matrix

| # | Run / event run id | Family / set / executor | Verdict / assurance | F1 / F2 / F3 | Terminal class / attribution | Wall |
|---:|---|---|---|---|---|---:|
| 1 | `dfix4_pipe_qwen35_001`<br>`019f73a9-c26c-78e0-adc6-d1aa9c1b6943` | pipe / A / qwen35 | failed / failed (`after_not_executed`) | pass / not executed / not executed | duplicate ownership: `inspect-prior-evidence` vs `implement-fix` / **machine (FIX-8)** | 718 s |
| 2 | `dfix4_pipe_gemma31_001`<br>`019f73b5-4179-71f0-92cc-946dd4b6a3d2` | pipe / B / gemma31 | failed / failed (`after_not_executed`) | pass / not executed / not executed | duplicate ownership: `fix-append-error` vs `run-pipeline` / **machine (FIX-8)** | 627 s |
| 3 | `dfix4_pipe_qwen35_002`<br>`019f73bf-7250-73f2-8bda-7cd4057c8725` | pipe / B / qwen35 | failed / failed (`after_not_executed`) | pass / not executed / not executed | duplicate ownership: `synthesize-cause-isolation` vs `implement-fix` / **machine (FIX-8)** | 248 s |
| 4 | `dfix4_schema_qwen35_001`<br>`019f73c4-1371-7c40-b5d4-7554d0ed4fcf` | schema / A / qwen35 | failed / failed (`after_not_executed`) | pass / not executed / not executed | repair read of absent `output/inspection.json` / **machine (FIX-9b)** | 909 s |
| 5 | `dfix4_schema_gemma31_001`<br>`019f73d2-7ad8-78f1-905b-a26e446fa3b5` | schema / B / gemma31 | failed / failed (`after_not_executed`) | pass / not executed / not executed | `model_stagnation:no_progress_recorded` at `execute-pipeline` / **model** | 1125 s |
| 6 | `dfix4_schema_qwen35_002`<br>`019f73e4-b9fc-7752-bf9f-9cfa1d36ec24` | schema / A / qwen35 | failed / failed (`after_not_executed`) | pass / not executed / not executed | absent `output/uat-console.log`、同turnでabsent inspectionもread / **machine+model** | 733 s |

run wall合計は4360秒（72分40秒）。Run 1開始からRun 6終了までのcampaign wallは4603秒（76分43秒、各run後の即時退避を含む）。族別full率はpipe 0/3、schema 0/3。executor別はqwen35 0/4、gemma31 0/2である。

### F系evidence監査

全runに`evidence/fix-*-before.json`と`fix-*-adjudication.json`が実在する。全F1は`stage=before, expected=failure, executed=true, outcome=failure, epoch=1`。F2は`after=null`、F3は`regressions=[]`で、adjudication statusはいずれも`not_executed`である。

| Run | F1 before_fails（R原文 / lineage / failure） | F2 after_passes | F3 no_regression |
|---:|---|---|---|
| 1 | `python3 -B pipeline/main.py` / `reproducer:d6587ef46e829187` / exit 1 `ValueError` | not executed; afterなし | 5 bindings frozen; 0 executed |
| 2 | `python3 -B pipeline/main.py` / `reproducer:d6587ef46e829187` / exit 1 `TypeError` | not executed; afterなし | 5 bindings frozen; 0 executed |
| 3 | `python3 -B pipeline/main.py` / `reproducer:d6587ef46e829187` / exit 1 `TypeError` | not executed; afterなし | 5 bindings frozen; 0 executed |
| 4 | `python -c "import json; d=json.load(open('output/results.json')); assert set(d.keys()) == {'reconciliation', 'values'} and set(d['reconciliation'].keys()) == {'input_rows', 'used_rows', 'excluded'}"` / `reproducer:ccd667d99df848de` / `AssertionError` | not executed; same-lineage/epoch ordering未行使 | 5 bindings frozen; 0 executed |
| 5 | `python -c "import json,sys; d=json.load(open('output/results.json')); r=d['reconciliation']; assert all(k in r for k in ('input_rows','used_rows','excluded')); assert isinstance(r['excluded'], list); assert r['input_rows'] == r['used_rows'] + sum(e['rows'] for e in r['excluded']); assert all(isinstance(v, (int, float)) for v in d.get('values', {}).values())"` / `reproducer:92dd10f5107760cd` / `KeyError`系schema failure | not executed; same-lineage/epoch ordering未行使 | 5 bindings frozen; 0 executed |
| 6 | `python -c "import json, sys; d=json.load(open('output/results.json')); assert 'reconciliation' in d and 'values' in d and 'input_rows' in d['reconciliation'] and 'used_rows' in d['reconciliation'] and 'excluded' in d['reconciliation']"` / `reproducer:f98d8f7c558485a7` / `AssertionError` | not executed; same-lineage/epoch ordering未行使 | 5 bindings frozen; 0 executed |

凍結F3集合は6/6で同じ順序だった。

1. `pipeline_probe` — `regression:539f12e6adea8590`
2. `data_reconciliation` — `regression:aad7a27c9f14260b`
3. `data_claims_binding` — `regression:5ee07e4968be4199`
4. `data_rerun_consistency` — `regression:09f9be875c51316d`
5. `data_results_schema` — `regression:292fa5e5cb8065da`

binding shrinkは0。fullが0件なので「full時のF1〜F3 evidence完全転記」は非該当である。

### イベント発火表

| Run | intent_resolved | host_env_normalized | fix_reproducer_suggested |
|---|---:|---:|---:|
| dfix4_pipe_qwen35_001 | 1 (`cli/fix`) | 1 (`NODE_ENV`, unset) | 1 (`pipeline_execution`) |
| dfix4_pipe_gemma31_001 | 1 (`cli/fix`) | 1 (`NODE_ENV`, unset) | 1 (`pipeline_execution`) |
| dfix4_pipe_qwen35_002 | 1 (`cli/fix`) | 1 (`NODE_ENV`, unset) | 1 (`pipeline_execution`) |
| dfix4_schema_qwen35_001 | 1 (`cli/fix`) | 1 (`NODE_ENV`, unset) | 1 (`data_results_schema`) |
| dfix4_schema_gemma31_001 | 1 (`cli/fix`) | 1 (`NODE_ENV`, unset) | 1 (`data_results_schema`) |
| dfix4_schema_qwen35_002 | 1 (`cli/fix`) | 1 (`NODE_ENV`, unset) | 1 (`data_results_schema`) |

再帰検索に使用したコマンドは次のとおり。

```sh
find workspace/management/runs/uat-test0717-dfix-004/artifacts-v5 \
  -path '*/.anvil/runs/*/events.jsonl' -print

rg --hidden -n -g 'events.jsonl' \
  '"event":"(intent_resolved|host_env_normalized|fix_reproducer_suggested)"' \
  workspace/management/runs/uat-test0717-dfix-004/artifacts-v5
```

検索原文とper-run countは`artifacts-v5/search-audit.md`にも保存した。

### FIX-8/9再発監査

検索対象は6つの`events.jsonl`、`summary.md`、plans、repairs、probe logsである。

```sh
rg --hidden -n -g 'events.jsonl' -g 'summary.md' -g '*.yaml' -g '*.md' \
  'duplicate expected path ownership|verify step requires at least one verify command|path does not exist' \
  workspace/management/runs/uat-test0717-dfix-004/artifacts-v5

rg --hidden -n -g 'events.jsonl' -g 'summary.md' -g '*.log' -g '*.md' \
  'stream did not contain valid UTF-8' \
  workspace/management/runs/uat-test0717-dfix-004/artifacts-v5
```

| Class | Count | Result |
|---|---:|---|
| 所有権重複lint | 3 run | **再発** |
| 空verify scaffold error | 0 | 再発なし |
| 不在成果物要求 | 2 terminal run | **再発** |
| UTF-8 phase死 | 0 | 再発なし |

発生原文全件:

1. Run 1: `duplicate expected path ownership: pipeline/main.py in inspect-prior-evidence and implement-fix`
2. Run 2: `duplicate expected path ownership: pipeline/main.py in fix-append-error and run-pipeline`
3. Run 3: `duplicate expected path ownership: pipeline/main.py in synthesize-cause-isolation and implement-fix`
4. Run 4: `path does not exist: output/inspection.json`
5. Run 6: `path does not exist: output/uat-console.log`
6. Run 6（同一executor turnの追加raw要求）: `"event":"tool_call_raw","name":"Read"` / `"preview":"output/inspection.json"`

Run 6の同一executor turnには`Read output/uat-console.log`と`Read output/inspection.json`が連続して記録され、先に返ったuat-console failureがterminal reasonとなった。従って不在要求はterminal 2run、raw `Read`要求はRun 4の1件とRun 6の2件、対象pathは2種類である。空verifyとUTF-8の0件主張は上記再帰検索に基づく。

### dfix-001〜003 + 本v5: 24run合算

| Family | Executor | Full | Partial | Static | Failed | Denominator | Full rate |
|---|---|---:|---:|---:|---:|---:|---:|
| pipe | qwen35 | 0 | 0 | 0 | 8 | 8 | 0% |
| pipe | gemma31 | 0 | 0 | 0 | 4 | 4 | 0% |
| schema | qwen35 | 0 | 0 | 0 | 8 | 8 | 0% |
| schema | gemma31 | 0 | 0 | 0 | 4 | 4 | 0% |
| **Total** | — | **0** | **0** | **0** | **24** | **24** | **0%** |

engine判定ではF1 pass 24/24、F2 not_executed 24/24、F3 not_executed 24/24、`failed(after_not_executed)` 24/24。dfix-001 Run 6のF1はengine passだが、既存報告どおりsubject読込前SyntaxErrorというUAT relevance caveatを維持する。

全runの一次死因と帰属を閉じた一覧にする。`複合`は、モデルが誘発した誤step/pathと、それを封じなかった機械policy gapの双方がterminalに必要だった場合であり、どちらかへ恣意的に丸めていない。

| Campaign / run | Primary death | Attribution |
|---|---|---|
| dfix-001 / pipe qwen A | workspaceにないprofile contract docをinspect | 複合（model path選択 + machine confinement gap） |
| dfix-001 / pipe gemma B | repair read-only exhaustion | model |
| dfix-001 / pipe qwen B | isolate-causeにwrite role漏出、未修復後exhaustion | 複合（machine role gap + model non-repair） |
| dfix-001 / schema qwen A | absent inspection prerequisite | machine |
| dfix-001 / schema gemma B | absent inspection prerequisite | machine |
| dfix-001 / schema qwen A2 | irrelevant SyntaxError R受理後、absent inspection | machine |
| dfix-002 / pipe qwen A | repair write-pressure exhaustion | model |
| dfix-002 / pipe gemma B | isolate implement/read-only mismatch | 複合 |
| dfix-002 / pipe qwen B | isolate implement/read-only mismatch | 複合 |
| dfix-002 / schema qwen A | absent inspection prerequisite | machine |
| dfix-002 / schema gemma B | bounded repair後schema verify failure | model |
| dfix-002 / schema qwen A2 | absent inspection prerequisite | machine |
| dfix-003 / pipe qwen A | duplicate expected path ownership | machine |
| dfix-003 / pipe gemma B | empty verify scaffold | machine |
| dfix-003 / pipe qwen B | invalid UTF-8 phase death | machine |
| dfix-003 / schema qwen A | absent inspection prerequisite | machine |
| dfix-003 / schema gemma B | empty verify scaffold | machine |
| dfix-003 / schema qwen A2 | empty verify scaffold | machine |
| dfix-004 v5 / pipe qwen A | duplicate expected path ownership | machine |
| dfix-004 v5 / pipe gemma B | duplicate expected path ownership | machine |
| dfix-004 v5 / pipe qwen B | duplicate expected path ownership | machine |
| dfix-004 v5 / schema qwen A | absent inspection read | machine |
| dfix-004 v5 / schema gemma B | `model_stagnation:no_progress_recorded` | model |
| dfix-004 v5 / schema qwen A2 | invented absent output paths not contained | 複合（model path + machine guard gap） |

帰属集計はmachine-only 15、model-only 4、複合5。dfix-003はレビュー前提どおりmachine 6/6。本v5はmachine-only 4、model-only 1、複合1で、残存はモデルだけには帰属できない。

### D-2コスト会計

| Scope | Acquisition / preflight | Execution | Reporting | Auditable total |
|---|---:|---:|---:|---:|
| dfix-001 | acquisition 756 s（preflight除外） | 4156 s | 395 s | 5307 s |
| dfix-002 | acquisition 280 s（preflight除外） | 3396 s | 545 s | 4221 s |
| dfix-003 | acquisition 356 s（preflight記録なし） | 3171 s | elapsed未保存 | **3527 s + unmeasured** |
| dfix-004 v5 | preflight 157 s + acquisition 207 s | campaign 4603 s | 1108 s（初稿・artifact監査まで） | 6075 s |

v5 reportingは最終run終了epoch 1784356510（2026-07-18 15:35:10 JST）から最終artifact manifest検証後epoch 1784357618（15:53:38 JST）までの1108秒として計上した。各区間は`date +%s`に基づく。

dfix-004 v1〜v4の空振り会計も別枠で合算する。

| Issue | Preflight / acquisition | Harness attempts | Product runs consumed | Reporting | Exact wall record |
|---|---|---:|---:|---|---:|
| v1 | full suite実施、指定workspace不在、acquisition 0 | 0 | 0 | 1 report | elapsed未保存 |
| v2 | full suite実施、source探索、schema source不在 | 0 | 0 | 1 report | elapsed未保存 |
| v3 | full suite + build/install + 4-set acquisition完了 | 6件exit 127 | 0（レビュー裁定） | 1 report | elapsed未保存 |
| v4 | guardrail failure、acquisition 0 | 0 | 0 | 1 report | 343 s |
| **v1〜v4合算** | full preflight 4発行、complete acquisition 1発行 | **6 harness failures** | **0** | **4 reports** | **既知下限343 s + v1〜v3未計時** |

v1〜v3にはepoch timing artifactがなく、commit時刻を作業開始時刻へ読み替えることもできないため、数値を捏造せず`unmeasured`として残す。従ってD-2の全既知wall下限は、dfix-001 5307 + dfix-002 4221 + dfix-003既知3527 + dfix-004空振り既知343 + v5 6075 = **19473秒**に、dfix-003 report/preflightとdfix-004 v1〜v3空振り実費を加えた値である。v3/v4の製品run消費はレビュー裁定どおり0で、本v5の6本だけがdfix-004正式runである。

### Artifact index

- `artifacts-v5/preflight.log`: clean/HEAD/full suite/build/install/version/hash/NODE_ENV。
- `artifacts-v5/provenance.md`: dfix-002 chain、主要SHA、R事前確認、調達timing。
- `artifacts-v5/run-matrix.json`: 6runのmachine-readable terminalとtiming。
- `artifacts-v5/search-audit.md`: recursive event/FIX検索コマンド、count、原文。
- `artifacts-v5/artifact-manifest.sha256`: manifest自身を除くartifact全ファイルのSHA-256。
- `artifacts-v5/<run>/.anvil/`: events、plans、repairs、probe evidence、summary。
- `artifacts-v5/<run>/evidence/fix-*.json`: F1/adjudication全12ファイル。
- `artifacts-v5/<run>/{pipeline,output,data}`、`uat-console.log`、`run-command.txt`、`outer-exit-code.txt`。

本v5で`src/`、`tests/`、`docs/`、台帳、bandは変更していない。admission、band、D-2クローズはレビュー側裁定に委ねる。
